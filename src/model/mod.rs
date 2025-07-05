pub mod vault;


use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSummary {
    pub vault_address: String,
    pub tvl: Decimal,
    pub equity: Decimal,
    pub apr: f64,
    pub all_time_pnl: Decimal,
    pub max_drawdown: f64,
    pub num_depositors: u64,
    pub portfolio_value: Decimal,
    pub deployed_liquidity: Decimal,
    pub idle_liquidity: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserState {
    pub account_value: Decimal,
    pub total_margin_used: Decimal,
    pub total_ntl_pos: Decimal,
    pub total_raw_usd: Decimal,
    pub positions: Vec<Position>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub size: Decimal,
    pub entry_px: Option<Decimal>,
    pub position_value: Decimal,
    pub unrealized_pnl: Decimal,
    pub margin_used: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub universe: Vec<AssetInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    pub name: String,
    pub sz_decimals: u8,
    pub max_leverage: u32,
    pub only_isolated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub coin: String,
    pub px: Decimal,
    pub sz: Decimal,
    pub side: String,
    pub time: u64,
    pub start_position: Decimal,
    pub dir: String,
    pub closed_pnl: Decimal,
    pub hash: String,
    pub oid: u64,
    pub crossed: bool,
    pub fee: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Snapshot {
    pub coin: String,
    pub time: u64,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub px: Decimal,
    pub sz: Decimal,
    pub n: u32,
}

#[derive(Debug, Clone, Default)]
pub struct GlobalMetrics {
    pub vault_metrics: VaultMetrics,
    pub performance_metrics: PerformanceMetrics,
    pub liquidity_metrics: LiquidityMetrics,
    pub risk_metrics: RiskMetrics,
    pub last_update: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct VaultMetrics {
    pub tvl: Decimal,
    pub equity: Decimal,
    pub apr: f64,
    pub utilization_rate: f64,
    pub deployed_liquidity: Decimal,
    pub idle_liquidity: Decimal,
}

#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub daily_pnl: Decimal,
    pub unrealized_pnl: Decimal,
    pub total_volume: Decimal,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub realized_spread: HashMap<String, f64>,
    pub adverse_selection_cost: f64,
}

#[derive(Debug, Clone, Default)]
pub struct LiquidityMetrics {
    pub bid_ask_spread_bps: HashMap<String, f64>,
    pub depth_at_50bps: HashMap<String, Decimal>,
    pub order_book_imbalance: HashMap<String, f64>,
    pub avg_order_lifetime_ms: f64,
    pub cancel_rate: f64,
    pub fleeting_order_ratio: f64,
    pub layering_detection_score: f64,
    pub spoofing_detection_index: f64,
    pub liquidity_realization_rate: f64,
    pub fill_probability_by_distance: HashMap<String, f64>,
}

#[derive(Debug, Clone, Default)]
pub struct RiskMetrics {
    pub vpin_score: f64,
    pub phantom_liquidity_index: f64,
    pub liquidation_risk_score: f64,
    pub cascade_risk_score: f64,
    pub position_concentration: HashMap<String, f64>,
    pub max_drawdown: f64,
    pub cross_exchange_manipulation_score: f64,
}

#[derive(Clone)]
pub enum OrderAction {
    New,
    Filled,
    Cancelled,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct OrderEvent {
    pub id:        u64,
    pub action:    OrderAction,
    pub coin:      String,
    pub side:      String,
    pub px:        Decimal,
    pub sz:        Decimal,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub level: AlertLevel,
    pub metric: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub threshold: f64,
}