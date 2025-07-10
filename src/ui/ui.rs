use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use reqwest::Client;
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use log::{info, warn, error, debug};
use rust_decimal::prelude::*;

use crate::api::provider::{DataProvider, DataSourceStatus, parse_decimal};
use crate::config::Config;
use crate::model::*;

pub struct HyperliquidProvider {
    info_client: InfoClient,
    ws_manager: Option<WsManager>,
    user_address: String,
    monitored_assets: Vec<String>,
}

pub struct InfoClient {
    client: Client,
    base_url: String,
}

pub struct WsManager {
    url: String,
    trade_sender: broadcast::Sender<Fill>,
    l2_sender: broadcast::Sender<L2Snapshot>,
    order_sender: broadcast::Sender<OrderEvent>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl InfoClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }
    
    pub async fn post_request(&self, endpoint: &str, payload: Value) -> Result<Value> {
        let url = format!("{}/{}", self.base_url, endpoint);
        debug!("📡 Making request to: {} with payload: {}", url, payload);
        
        let response = self.client
            .post(&url)
            .json(&payload)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| {
                error!("❌ HTTP request failed: {}", e);
                anyhow::anyhow!("HTTP request failed: {}", e)
            })?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("❌ Request failed with status {}: {}", status, error_body);
            return Err(anyhow::anyhow!("Request failed: {} - {}", status, error_body));
        }
        
        let result = response.json().await
            .map_err(|e| {
                error!("❌ Failed to parse JSON response: {}", e);
                anyhow::anyhow!("Failed to parse JSON response: {}", e)
            })?;
        
        debug!("✅ Request successful, response: {}", result);
        Ok(result)
    }
    
    pub async fn get_clearinghouse_state(&self, user_address: &str) -> Result<Value> {
        let payload = serde_json::json!({
            "type": "clearinghouseState", 
            "user": user_address
        });
        
        info!("📊 Fetching clearinghouse state for user: {}", user_address);
        self.post_request("info", payload).await
    }
    
    pub async fn get_meta(&self) -> Result<Value> {
        let payload = serde_json::json!({
            "type": "meta"
        });
        
        info!("📊 Fetching meta information");
        self.post_request("info", payload).await
    }
    
    pub async fn get_user_fills(&self, user_address: &str) -> Result<Value> {
        let payload = serde_json::json!({
            "type": "userFills",
            "user": user_address
        });
        
        info!("📊 Fetching user fills for: {}", user_address);
        self.post_request("info", payload).await
    }
    
    pub async fn get_l2_book(&self, coin: &str) -> Result<Value> {
        let payload = serde_json::json!({
            "type": "l2Book",
            "coin": coin
        });
        
        debug!("📊 Fetching L2 book for: {}", coin);
        self.post_request("info", payload).await
    }

    #[allow(dead_code)]
    pub async fn get_all_mids(&self) -> Result<Value> {
        let payload = serde_json::json!({
            "type": "allMids"
        });
        
        debug!("📊 Fetching all mids");
        self.post_request("info", payload).await
    }
}

impl WsManager {
    pub fn new(url: String) -> Self {
        let (trade_sender, _) = broadcast::channel(1000);
        let (l2_sender, _) = broadcast::channel(1000);
        let (order_sender, _) = broadcast::channel(1000);
        let connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        
        Self { 
            url,
            trade_sender,
            l2_sender,
            order_sender,
            connected,
        }
    }
    
    pub async fn connect_and_subscribe(&self, assets: &[String]) -> Result<()> {
        let ws_url = &self.url;
        info!("🔌 Connecting to WebSocket: {}", ws_url);
        
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut ws_sink, mut ws_stream) = ws_stream.split();
        
        self.connected.store(true, std::sync::atomic::Ordering::Relaxed);
        info!("✅ WebSocket connected successfully");
        
        let order_subscribe_msg = serde_json::json!({
            "method": "subscribe",
            "subscription": {
                "type": "orders"
            }
        });
        
        ws_sink.send(Message::Text(order_subscribe_msg.to_string())).await?;
        info!("📊 Subscribed to orders");

        for asset in assets {
            let subscribe_msg = serde_json::json!({
                "method": "subscribe",
                "subscription": {
                    "type": "trades",
                    "coin": asset
                }
            });
            
            ws_sink.send(Message::Text(subscribe_msg.to_string())).await?;
            info!("📡 Subscribed to trades for {}", asset);
            
            let l2_subscribe_msg = serde_json::json!({
                "method": "subscribe", 
                "subscription": {
                    "type": "l2Book",
                    "coin": asset
                }
            });
            
            ws_sink.send(Message::Text(l2_subscribe_msg.to_string())).await?;
            info!("📊 Subscribed to L2 book for {}", asset);

        }
        
        let trade_sender = self.trade_sender.clone();
        let l2_sender = self.l2_sender.clone();
        let order_sender = self.order_sender.clone();
        let connected = self.connected.clone();
        
        tokio::spawn(async move {
            while let Some(msg_result) = ws_stream.next().await {
                match msg_result {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = Self::handle_message(&text, &trade_sender, &l2_sender, &order_sender).await {
                            warn!("⚠️ Failed to handle WebSocket message: {}", e);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        warn!("🔌 WebSocket connection closed");
                        connected.store(false, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    Err(e) => {
                        error!("❌ WebSocket error: {}", e);
                        connected.store(false, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    _ => {}
                }
            }
        });
        
        Ok(())
    }
    
    async fn handle_message(
        text: &str,
        trade_sender: &broadcast::Sender<Fill>,
        l2_sender: &broadcast::Sender<L2Snapshot>,
        order_sender: &broadcast::Sender<OrderEvent>,
    ) -> Result<()> {
        let msg: Value = serde_json::from_str(text)?;
        
        if let Some(channel) = msg.get("channel").and_then(|v| v.as_str()) {
            match channel {
                "trades" => {
                    if let Some(data) = msg.get("data") {
                        for trade_data in data.as_array().unwrap_or(&vec![]) {
                            let fill = Self::parse_trade(trade_data)?;
                            if let Err(_) = trade_sender.send(fill) {
                                debug!("No trade receivers active");
                            }
                        }
                    }
                }
                "l2Book" => {
                    if let Some(data) = msg.get("data") {
                        let snapshot = Self::parse_l2_snapshot(data)?;
                        if let Err(_) = l2_sender.send(snapshot) {
                            debug!("No L2 receivers active");
                        }
                    }
                }
                "orders" => {
                    if let Some(data) = msg.get("data") {
                        for order in data.as_array().unwrap_or(&vec![]) {
                            let evt = Self::parse_order_event(order)?;
                            if let Err(_) = order_sender.send(evt) {
                                debug!("No order receivers active");
                            }
                        }

                    }
                }
                _ => {
                    debug!("📨 Unhandled channel: {}", channel);
                }
            }
        }
        
        Ok(())
    }
    
    fn parse_order_event(data: &Value) -> Result<OrderEvent> {
        use OrderAction::*;
        let action = match data["status"].as_str().unwrap_or("") {
            "open"      => New,
            "filled"    => Filled,
            "cancelled" => Cancelled,
            other       => anyhow::bail!("unknown status {}", other),
        };
        Ok(OrderEvent {
            id:        data["oid"].as_u64().unwrap_or(0),
            action,
            coin:      data["coin"].as_str().unwrap_or("").to_string(),
            side:      data["side"].as_str().unwrap_or("").to_string(),
            px:        parse_decimal(data["limitPx"].as_str().unwrap_or("0")),
            sz:        parse_decimal(data["sz"].as_str().unwrap_or("0")),
            timestamp: data["statusTimestamp"].as_u64().unwrap_or(0),
        })
    }
    
    fn parse_trade(data: &Value) -> Result<Fill> {
        Ok(Fill {
            coin: data["coin"].as_str().unwrap_or("").to_string(),
            px: parse_decimal(data["px"].as_str().unwrap_or("0")),
            sz: parse_decimal(data["sz"].as_str().unwrap_or("0")),
            side: data["side"].as_str().unwrap_or("").to_string(),
            time: data["time"].as_u64().unwrap_or(0),
            start_position: parse_decimal("0"),
            dir: data["side"].as_str().unwrap_or("").to_string(),
            closed_pnl: parse_decimal("0"),
            hash: data["tid"].as_str().unwrap_or("").to_string(),
            oid: 0,
            crossed: false,
            fee: parse_decimal("0"),
        })
    }
    
    fn parse_l2_snapshot(data: &Value) -> Result<L2Snapshot> {
        let coin = data["coin"].as_str().unwrap_or("").to_string();
        let time = data["time"].as_u64().unwrap_or(0);
        
        let levels = &data["levels"];
        let bids = levels[0]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|level| OrderBookLevel {
                px: parse_decimal(level["px"].as_str().unwrap_or("0")),
                sz: parse_decimal(level["sz"].as_str().unwrap_or("0")),
                n: level["n"].as_u64().unwrap_or(0) as u32,
            })
            .collect();
            
        let asks = levels[1]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|level| OrderBookLevel {
                px: parse_decimal(level["px"].as_str().unwrap_or("0")),
                sz: parse_decimal(level["sz"].as_str().unwrap_or("0")),
                n: level["n"].as_u64().unwrap_or(0) as u32,
            })
            .collect();
            
        Ok(L2Snapshot {
            coin,
            time,
            bids,
            asks,
        })
    }
    
    pub fn get_trade_receiver(&self) -> broadcast::Receiver<Fill> {
        self.trade_sender.subscribe()
    }
    
    pub fn get_l2_receiver(&self) -> broadcast::Receiver<L2Snapshot> {
        self.l2_sender.subscribe()
    }

    pub fn get_order_receiver(&self) -> broadcast::Receiver<OrderEvent> {
        self.order_sender.subscribe()
    }
    
    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl HyperliquidProvider {
    pub async fn new(config: &Config) -> Result<Self> {
        info!("🚀 Initializing HyperliquidProvider with API: {}", config.hyperliquid_api_url);
        
        let info_client = InfoClient::new(config.hyperliquid_api_url.clone());
        
        let ws_manager = if config.enable_websocket {
            let ws_url = config.hyperliquid_api_url
                .replace("https://", "wss://")
                .replace("http://", "ws://") + "/ws";
            info!("🔌 WebSocket URL: {}", ws_url);
            Some(WsManager::new(ws_url))
        } else {
            info!("🔌 WebSocket disabled in config");
            None
        };
        
        // Start with defaults
        let mut monitored_assets = Self::get_default_monitored_assets();
        
        let provider = Self {
            info_client,
            ws_manager,
            user_address: config.user_address.clone(),
            monitored_assets: monitored_assets.clone(),
        };
        
        info!("✅ Testing API connectivity...");
        match provider.info_client.get_meta().await {
            Ok(_) => {
                info!("✅ API connectivity test successful");
            }
            Err(e) => {
                error!("❌ API connectivity test failed: {}", e);
                return Err(anyhow::anyhow!("Failed to connect to Hyperliquid API: {}", e));
            }
        }
        
        // Get user's actual positions
        info!("📊 Fetching user's actual positions...");
        match provider.info_client.get_clearinghouse_state(&config.user_address).await {
            Ok(state_data) => {
                let mut user_assets = Vec::new();
                
                // Extract assets from current positions
                if let Some(positions) = state_data["assetPositions"].as_array() {
                    for pos in positions {
                        if let Some(position) = pos.get("position") {
                            if let Some(coin) = position["coin"].as_str() {
                                // Only add if position size is non-zero
                                let size = position["szi"].as_str().unwrap_or("0");
                                if size != "0" && !user_assets.contains(&coin.to_string()) {
                                    user_assets.push(coin.to_string());
                                }
                            }
                        }
                    }
                }
                
                // If user has positions, use those; otherwise keep defaults
                if !user_assets.is_empty() {
                    monitored_assets = user_assets;
                    info!("📊 Found {} positions for user: {:?}", monitored_assets.len(), monitored_assets);
                } else {
                    info!("📊 No positions found, using default assets");
                }
            }
            Err(e) => {
                warn!("⚠️ Failed to fetch user positions: {}, using defaults", e);
            }
        }
        
        // Create the final provider with user's assets
        let provider = Self {
            info_client: provider.info_client,
            ws_manager: provider.ws_manager,
            user_address: provider.user_address,
            monitored_assets,
        };
        
        if let Some(ref ws_manager) = provider.ws_manager {
            if let Err(e) = ws_manager.connect_and_subscribe(&provider.monitored_assets).await {
                warn!("⚠️ Failed to connect WebSocket, falling back to HTTP only: {}", e);
            }
        }
        
        info!("✅ HyperliquidProvider initialized successfully");
        Ok(provider)
    }
    
    fn get_default_monitored_assets() -> Vec<String> {
        vec![
            "BTC".to_string(),
            "ETH".to_string(), 
            "SOL".to_string(),
            "DOGE".to_string(),
            "AVAX".to_string(),
            "ARB".to_string(),
            "MATIC".to_string(),
            "OP".to_string(),
            "LINK".to_string(),
            "ATOM".to_string(),
            "DOT".to_string(),
            "UNI".to_string(),
            "CRV".to_string(),
            "AAVE".to_string(),
            "SNX".to_string(),
            "MKR".to_string(),
            "COMP".to_string(),
            "YFI".to_string(),
            "SUSHI".to_string(),
            "1INCH".to_string(),
            "ENS".to_string(),
            "GMX".to_string(),
            "BLUR".to_string(),
            "LDO".to_string(),
            "RPL".to_string(),
            "RNDR".to_string(),
            "IMX".to_string(),
            "SAND".to_string(),
            "MANA".to_string(),
            "AXS".to_string(),
            "APE".to_string(),
            "GALA".to_string(),
            "FTM".to_string(),
            "NEAR".to_string(),
            "FIL".to_string(),
            "APT".to_string(),
            "SUI".to_string(),
            "SEI".to_string(),
            "INJ".to_string(),
            "TIA".to_string(),
            "PYTH".to_string(),
            "JUP".to_string(),
            "WIF".to_string(),
            "BONK".to_string(),
            "PEPE".to_string(),
            "SHIB".to_string(),
            "FLOKI".to_string(),
            "MEME".to_string(),
            "ORDI".to_string(),
            "STX".to_string(),
        ]
    }
    
    #[allow(dead_code)]
    pub async fn update_monitored_assets_from_meta(&mut self, meta: &Meta) {
        let major_assets: Vec<String> = meta.universe
            .iter()
            .filter(|asset| {
                let name = &asset.name;
                name == "BTC" || name == "ETH" || name == "SOL" || 
                name == "DOGE" || name == "AVAX" || asset.max_leverage >= 10
            })
            .take(10)
            .map(|asset| asset.name.clone())
            .collect();
            
        if major_assets != self.monitored_assets {
            info!("📊 Updating monitored assets: {:?}", major_assets);
            self.monitored_assets = major_assets;
            
            if let Some(ref ws_manager) = self.ws_manager {
                if ws_manager.is_connected() {
                    if let Err(e) = ws_manager.connect_and_subscribe(&self.monitored_assets).await {
                        warn!("⚠️ Failed to resubscribe to new assets: {}", e);
                    }
                }
            }
        }
    }
    
    pub fn get_live_trades(&self) -> Option<broadcast::Receiver<Fill>> {
        self.ws_manager.as_ref().map(|ws| ws.get_trade_receiver())
    }
    
    pub fn get_live_l2_updates(&self) -> Option<broadcast::Receiver<L2Snapshot>> {
        self.ws_manager.as_ref().map(|ws| ws.get_l2_receiver())
    }

    pub fn get_live_orders(&self) -> Option<broadcast::Receiver<OrderEvent>> {
        self.ws_manager.as_ref().map(|ws| ws.get_order_receiver())
    }
    
    #[allow(dead_code)]
    pub fn get_monitored_assets(&self) -> &[String] {
        &self.monitored_assets
    }
    
    async fn convert_user_state(&self, data: Value) -> Result<UserState> {
        debug!("📊 Converting user state data: {}", data);
        
        let margin_summary = data.get("marginSummary")
            .ok_or_else(|| anyhow::anyhow!("Missing marginSummary in response"))?;
        let cross_margin_summary = data.get("crossMarginSummary")
            .ok_or_else(|| anyhow::anyhow!("Missing crossMarginSummary in response"))?;
        
        let account_value = parse_decimal(margin_summary["accountValue"].as_str().unwrap_or("0"));
        let total_margin_used = parse_decimal(cross_margin_summary["totalMarginUsed"].as_str().unwrap_or("0"));
        let total_ntl_pos = parse_decimal(cross_margin_summary["totalNtlPos"].as_str().unwrap_or("0"));
        let total_raw_usd = parse_decimal(cross_margin_summary["totalRawUsd"].as_str().unwrap_or("0"));
        
        let positions: Vec<_> = data["assetPositions"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|pos| {
                let position = pos.get("position")?;
                let symbol = position["coin"].as_str().unwrap_or("").to_string();
                let size = parse_decimal(position["szi"].as_str().unwrap_or("0"));
                
                // Only include non-zero positions
                if size == rust_decimal::Decimal::ZERO {
                    return None;
                }
                
                Some(Position {
                    symbol: symbol.clone(),
                    size,
                    entry_px: position.get("entryPx").and_then(|v| v.as_str()).map(parse_decimal),
                    position_value: parse_decimal(position["positionValue"].as_str().unwrap_or("0")),
                    unrealized_pnl: parse_decimal(position["unrealizedPnl"].as_str().unwrap_or("0")),
                    margin_used: parse_decimal(position["marginUsed"].as_str().unwrap_or("0")),
                })
            })
            .collect();
            
        info!("✅ Converted user state - Account Value: ${:.2}, Margin Used: ${:.2}, Active Positions: {}", 
              account_value.to_f64().unwrap_or(0.0),
              total_margin_used.to_f64().unwrap_or(0.0),
              positions.len());
        
        // Log position details for debugging
        for pos in &positions {
            debug!("  Position: {} Size: {} Value: ${:.2} PnL: ${:.2}", 
                pos.symbol, 
                pos.size,
                pos.position_value.to_f64().unwrap_or(0.0),
                pos.unrealized_pnl.to_f64().unwrap_or(0.0)
            );
        }
        
        Ok(UserState {
            account_value,
            total_margin_used,
            total_ntl_pos,
            total_raw_usd,
            positions,
        })
    }
    
    async fn convert_meta(&self, data: Value) -> Result<Meta> {
        debug!("📊 Converting meta data");
        
        let universe: Vec<_> = data["universe"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|asset| AssetInfo {
                name: asset["name"].as_str().unwrap_or("").to_string(),
                sz_decimals: asset["szDecimals"].as_u64().unwrap_or(0) as u8,
                max_leverage: asset["maxLeverage"].as_u64().unwrap_or(1) as u32,
                only_isolated: asset["onlyIsolated"].as_bool().unwrap_or(false),
            })
            .collect();
            
        info!("✅ Converted meta - {} assets in universe", universe.len());
        
        Ok(Meta {
            universe,
        })
    }
    
    async fn convert_fills(&self, data: Value) -> Result<Vec<Fill>> {
        debug!("📊 Converting fills data");
        
        let fills: Vec<_> = data
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|fill| Fill {
                coin: fill["coin"].as_str().unwrap_or("").to_string(),
                px: parse_decimal(fill["px"].as_str().unwrap_or("0")),
                sz: parse_decimal(fill["sz"].as_str().unwrap_or("0")),
                side: fill["side"].as_str().unwrap_or("").to_string(),
                time: fill["time"].as_u64().unwrap_or(0),
                start_position: parse_decimal(fill["startPosition"].as_str().unwrap_or("0")),
                dir: fill["dir"].as_str().unwrap_or("").to_string(),
                closed_pnl: parse_decimal(fill["closedPnl"].as_str().unwrap_or("0")),
                hash: fill["hash"].as_str().unwrap_or("").to_string(),
                oid: fill["oid"].as_u64().unwrap_or(0),
                crossed: fill["crossed"].as_bool().unwrap_or(false),
                fee: parse_decimal(fill["fee"].as_str().unwrap_or("0")),
            })
            .collect();
            
        info!("✅ Converted {} fills", fills.len());
        Ok(fills)
    }
    
    async fn convert_l2_snapshot(&self, coin: &str, data: Value) -> Result<L2Snapshot> {
        let levels = data.get("levels")
            .ok_or_else(|| anyhow::anyhow!("Missing levels in L2 response for {}", coin))?;
        
        let bids = levels[0]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|level| OrderBookLevel {
                px: parse_decimal(level["px"].as_str().unwrap_or("0")),
                sz: parse_decimal(level["sz"].as_str().unwrap_or("0")),
                n: level["n"].as_u64().unwrap_or(0) as u32,
            })
            .collect();
            
        let asks = levels[1]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|level| OrderBookLevel {
                px: parse_decimal(level["px"].as_str().unwrap_or("0")),
                sz: parse_decimal(level["sz"].as_str().unwrap_or("0")),
                n: level["n"].as_u64().unwrap_or(0) as u32,
            })
            .collect();
            
        Ok(L2Snapshot {
            coin: coin.to_string(),
            time: data["time"].as_u64().unwrap_or(0),
            bids,
            asks,
        })
    }
}

#[async_trait]
impl DataProvider for HyperliquidProvider {
    async fn get_vault_summary(&self) -> Result<VaultSummary> {
        info!("📊 Creating synthetic vault summary from user state");
        
        let user_state = self.get_user_state().await?;
        
        let tvl = user_state.account_value;
        let equity = user_state.account_value;
        let portfolio_value = user_state.total_raw_usd;
        let deployed_liquidity = user_state.total_margin_used;
        let idle_liquidity = user_state.total_raw_usd - user_state.total_margin_used;
        
        let all_time_pnl = user_state.positions.iter()
            .map(|pos| pos.unrealized_pnl)
            .sum::<rust_decimal::Decimal>();
        
        let max_drawdown = if all_time_pnl < rust_decimal::Decimal::ZERO {
            (all_time_pnl / equity).to_f64().unwrap_or(0.0).abs()
        } else {
            0.0
        };
        
        let apr = if equity > rust_decimal::Decimal::ZERO && all_time_pnl > rust_decimal::Decimal::ZERO {
            (all_time_pnl / equity * rust_decimal::Decimal::from(365) * rust_decimal::Decimal::from(100))
                .to_f64().unwrap_or(0.0)
        } else {
            5.76
        };
        
        info!("✅ Synthetic vault summary - TVL: ${:.2}, Equity: ${:.2}, APR: {:.2}%", 
              tvl.to_f64().unwrap_or(0.0),
              equity.to_f64().unwrap_or(0.0),
              apr);
        
        Ok(VaultSummary {
            vault_address: self.user_address.clone(),
            tvl,
            equity,
            apr,
            all_time_pnl,
            max_drawdown,
            num_depositors: 1,
            portfolio_value,
            deployed_liquidity,
            idle_liquidity,
        })
    }
    
    async fn get_user_state(&self) -> Result<UserState> {
        info!("📊 Fetching user state for: {}", self.user_address);
        let data = self.info_client.get_clearinghouse_state(&self.user_address).await?;
        self.convert_user_state(data).await
    }
    
    async fn get_meta(&self) -> Result<Meta> {
        info!("📊 Fetching meta information");
        let data = self.info_client.get_meta().await?;
        self.convert_meta(data).await
    }
    
    async fn get_recent_fills(&self) -> Result<Vec<Fill>> {
        info!("📊 Fetching recent fills for: {}", self.user_address);
        let data = self.info_client.get_user_fills(&self.user_address).await?;
        self.convert_fills(data).await
    }
    
    async fn get_l2_snapshots(&self) -> Result<HashMap<String, L2Snapshot>> {
        info!("📊 Fetching L2 snapshots for {} assets", self.monitored_assets.len());
        let mut snapshots = HashMap::new();
        let mut successful_fetches = 0;
        
        for coin in &self.monitored_assets {
            match self.info_client.get_l2_book(coin).await {
                Ok(data) => {
                    match self.convert_l2_snapshot(coin, data).await {
                        Ok(snapshot) => {
                            snapshots.insert(coin.clone(), snapshot);
                            successful_fetches += 1;
                            debug!("✅ Successfully fetched L2 for {}", coin);
                        }
                        Err(e) => {
                            warn!("⚠️ Failed to convert L2 snapshot for {}: {}", coin, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("⚠️ Failed to get L2 book for {}: {}", coin, e);
                }
            }
        }
        
        info!("📊 Successfully fetched L2 snapshots for {}/{} assets", 
              successful_fetches, self.monitored_assets.len());
        
        if snapshots.is_empty() {
            warn!("⚠️ No L2 snapshots were successfully fetched!");
        }
        
        Ok(snapshots)
    }
    
    async fn get_status(&self) -> DataSourceStatus {
        let http_status = match self.info_client.get_meta().await {
            Ok(_) => {
                debug!("✅ HTTP API status: Connected");
                true
            }
            Err(e) => {
                debug!("❌ HTTP API status: Error - {}", e);
                false
            }
        };
        
        let ws_status = self.ws_manager
            .as_ref()
            .map(|ws| {
                let connected = ws.is_connected();
                debug!("🔌 WebSocket status: {}", if connected { "Connected" } else { "Disconnected" });
                connected
            })
            .unwrap_or_else(|| {
                debug!("🔌 WebSocket status: Disabled");
                false
            });
        
        match (http_status, ws_status) {
            (true, true) => DataSourceStatus::Connected,
            (true, false) => DataSourceStatus::Error("WebSocket disconnected, HTTP only".to_string()),
            (false, _) => DataSourceStatus::Disconnected,
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}