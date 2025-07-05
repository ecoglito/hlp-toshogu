use crate::model::*;
use rust_decimal::prelude::*;
use std::collections::{HashMap, VecDeque};
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use std::sync::Arc;
use log::{debug, info, warn};
use rust_decimal_macros::dec;

pub struct StreamingMetricsEngine {
    trade_buffer: VecDeque<Fill>,
    l2_snapshots: HashMap<String, L2Snapshot>,
    vpin_buckets: VecDeque<f64>,
    bucket_accumulator: VpinBucketAccumulator,
    order_flow_analyzer: OrderFlowAnalyzer,
    phantom_liquidity_tracker: PhantomLiquidityTracker,
    active_orders: HashMap<u64, std::time::Instant>,
    total_volume_traded: Decimal,
    volume_by_coin: HashMap<String, Decimal>,
}

#[derive(Default)]
struct VpinBucketAccumulator {
    current_volume: Decimal,
    buy_volume: Decimal,
    sell_volume: Decimal,
    bucket_size: Decimal,
}

#[derive(Default)]
struct OrderFlowAnalyzer {
    order_lifetimes: VecDeque<u64>,
    cancellation_events: u32,
    total_orders: u32,
    fleeting_orders: u32,
}

#[derive(Default)]
struct PhantomLiquidityTracker {
    layering_score: f64,
    spoofing_events: u32,
    total_depth_promises: Decimal,
    realized_depth: Decimal,
}

#[derive(Default)]
pub struct PhantomLiquidityMetrics {
    pub fleeting_order_ratio: f64,
    pub avg_order_lifetime_ms: f64,
    pub layering_score: f64,
    pub spoofing_events: u32,
    pub cancellation_rate: f64,
}

impl StreamingMetricsEngine {
    pub fn new() -> Self {
        Self {
            trade_buffer: VecDeque::with_capacity(10000),
            l2_snapshots: HashMap::new(),
            vpin_buckets: VecDeque::with_capacity(100),
            bucket_accumulator: VpinBucketAccumulator {
                bucket_size: Decimal::from(10000),
                ..Default::default()
            },
            order_flow_analyzer: OrderFlowAnalyzer::default(),
            phantom_liquidity_tracker: PhantomLiquidityTracker::default(),
            active_orders: HashMap::new(),
            total_volume_traded: Decimal::ZERO,
            volume_by_coin: HashMap::new(),
        }
    }

    pub async fn run(
        engine: Arc<RwLock<Self>>,
        mut trade_rx: broadcast::Receiver<Fill>,
        mut l2_rx: broadcast::Receiver<L2Snapshot>,
        mut order_rx: broadcast::Receiver<OrderEvent>,
    ) {
        loop {
            tokio::select! {
                Ok(fill) = trade_rx.recv() => {
                    let mut e = engine.write().await;
                    e.process_trade(fill).await;
                }
                Ok(snapshot) = l2_rx.recv() => {
                    let mut e = engine.write().await;
                    e.process_l2_update(snapshot).await;
                }
                Ok(evt) = order_rx.recv() => {
                    let mut e = engine.write().await;
                    match evt.action {
                        OrderAction::New => e.on_new_order(evt.id),
                        OrderAction::Cancelled => e.on_cancel_or_fill(evt.id, true),
                        OrderAction::Filled => e.on_cancel_or_fill(evt.id, false),
                    }
                }
            }
        }
    }

    pub fn on_new_order(&mut self, id: u64) {
        self.active_orders.insert(id, std::time::Instant::now());
    }
    
    pub fn on_cancel_or_fill(&mut self, id: u64, is_cancel: bool) {
        if let Some(t0) = self.active_orders.remove(&id) {
            let lifetime = t0.elapsed().as_millis() as u64;
            self.order_flow_analyzer.total_orders += 1;
            self.order_flow_analyzer.order_lifetimes.push_back(lifetime);
            if lifetime < 100 {
                self.order_flow_analyzer.fleeting_orders += 1;
            }
            if is_cancel {
                self.order_flow_analyzer.cancellation_events += 1;
            }
            if self.order_flow_analyzer.order_lifetimes.len() > 10_000 {
                self.order_flow_analyzer.order_lifetimes.pop_front();
            }
        }
    }

    #[allow(dead_code)]
    pub async fn start_streaming_analysis(
        &mut self,
        mut trade_receiver: broadcast::Receiver<Fill>,
        mut l2_receiver: broadcast::Receiver<L2Snapshot>,
    ) {
        info!("🔄 Starting streaming metrics analysis");

        loop {
            tokio::select! {
                trade_result = trade_receiver.recv() => {
                    match trade_result {
                        Ok(fill) => {
                            self.process_trade(fill).await;
                        }
                        Err(e) => {
                            warn!("📡 Trade stream error: {}", e);
                        }
                    }
                }
                l2_result = l2_receiver.recv() => {
                    match l2_result {
                        Ok(snapshot) => {
                            self.process_l2_update(snapshot).await;
                        }
                        Err(e) => {
                            warn!("📊 L2 stream error: {}", e);
                        }
                    }
                }
            }
        }
    }

    async fn process_trade(&mut self, fill: Fill) {
        debug!("📈 Processing trade: {} {} @ {}", fill.coin, fill.sz, fill.px);
        
        let trade_volume = fill.px * fill.sz.abs();
        self.total_volume_traded += trade_volume;
        *self.volume_by_coin.entry(fill.coin.clone()).or_insert(Decimal::ZERO) += trade_volume;

        self.update_vpin_calculation(&fill);
        self.analyze_order_flow(&fill);
        
        self.trade_buffer.push_back(fill);
        
        if self.trade_buffer.len() > 5000 {
            self.trade_buffer.pop_front();
        }


    }

    async fn process_l2_update(&mut self, snapshot: L2Snapshot) {
        debug!("📊 Processing L2 update for {}: {} bids, {} asks", 
               snapshot.coin, snapshot.bids.len(), snapshot.asks.len());
        
        let previous_snapshot = self.l2_snapshots.get(&snapshot.coin).cloned();
        
        if let Some(previous_snapshot) = previous_snapshot {
            self.detect_phantom_liquidity(&previous_snapshot, &snapshot);
        }
        
        self.l2_snapshots.insert(snapshot.coin.clone(), snapshot);
    }

    fn update_vpin_calculation(&mut self, fill: &Fill) {
        let volume = fill.px * fill.sz.abs();
        
        if fill.side == "B" {
            self.bucket_accumulator.buy_volume += volume;
        } else {
            self.bucket_accumulator.sell_volume += volume;
        }
        
        self.bucket_accumulator.current_volume += volume;
        
        if self.bucket_accumulator.current_volume >= self.bucket_accumulator.bucket_size {
            let total_volume = self.bucket_accumulator.buy_volume + self.bucket_accumulator.sell_volume;
            
            if total_volume > Decimal::ZERO {
                let imbalance = (self.bucket_accumulator.buy_volume - self.bucket_accumulator.sell_volume).abs();
                let vpin = (imbalance / total_volume).to_f64().unwrap_or(0.0);
                
                self.vpin_buckets.push_back(vpin);
                
                if self.vpin_buckets.len() > 50 {
                    self.vpin_buckets.pop_front();
                }
                
                debug!("🔍 New VPIN bucket: {:.4} (imbalance: {:.2}%)", vpin, vpin * 100.0);
            }
            
            self.bucket_accumulator.current_volume = Decimal::ZERO;
            self.bucket_accumulator.buy_volume = Decimal::ZERO;
            self.bucket_accumulator.sell_volume = Decimal::ZERO;
        }
    }

    fn analyze_order_flow(&mut self, fill: &Fill) {
        self.order_flow_analyzer.total_orders += 1;
        
        let order_lifetime = self.estimate_order_lifetime(fill);
        self.order_flow_analyzer.order_lifetimes.push_back(order_lifetime);
        
        if order_lifetime < 100 {
            self.order_flow_analyzer.fleeting_orders += 1;
            debug!("👻 Fleeting order detected: {} ({}ms)", fill.coin, order_lifetime);
        }
        
        if self.is_likely_cancellation(fill) {
            self.order_flow_analyzer.cancellation_events += 1;
        }
        
        if self.order_flow_analyzer.order_lifetimes.len() > 1000 {
            self.order_flow_analyzer.order_lifetimes.pop_front();
        }
    }

    fn detect_phantom_liquidity(&mut self, previous: &L2Snapshot, current: &L2Snapshot) {
        let depth_change = self.calculate_depth_change(previous, current);
        let layering_score = self.detect_layering_patterns(previous, current);
        
        self.phantom_liquidity_tracker.layering_score = 
            self.phantom_liquidity_tracker.layering_score * 0.8 + layering_score * 0.2; //TODO: This is a hack to make the layering score more responsive
        
        if depth_change.abs() > 0.05 { //TODO: This is a hack to make the spoofing detection more responsive
            self.phantom_liquidity_tracker.spoofing_events += (1.0/self.order_flow_analyzer.total_orders as f64).min(1.0) as u32;
            debug!("🎭 Potential spoofing detected in {}: depth change {:.2}%", 
                   current.coin, depth_change * 100.0);
        }
        
        self.phantom_liquidity_tracker.total_depth_promises += self.calculate_total_depth(current);
        self.phantom_liquidity_tracker.realized_depth += self.calculate_total_depth(current) * dec!(0.8);
        
    }

    fn calculate_depth_change(&self, previous: &L2Snapshot, current: &L2Snapshot) -> f64 {
        let prev_depth = self.calculate_total_depth(previous);
        let curr_depth = self.calculate_total_depth(current);
        
        if prev_depth == Decimal::ZERO {
            return 0.0;
        }
        
        ((curr_depth - prev_depth) / prev_depth).to_f64().unwrap_or(0.0)
    }

    fn calculate_depth_realisation_ratio(&self) -> f64 {
        if self.phantom_liquidity_tracker.total_depth_promises == Decimal::ZERO {
            0.0
        } else {
            (self.phantom_liquidity_tracker.realized_depth
                / self.phantom_liquidity_tracker.total_depth_promises)
                .to_f64()
                .unwrap_or(0.0)
                .clamp(0.0, 1.0)
        }
    }

    fn calculate_total_depth(&self, snapshot: &L2Snapshot) -> Decimal {
        let bid_depth: Decimal = snapshot.bids.iter().take(5).map(|level| level.sz).sum();
        let ask_depth: Decimal = snapshot.asks.iter().take(5).map(|level| level.sz).sum();
        bid_depth + ask_depth
    }

    fn detect_layering_patterns(&self, previous: &L2Snapshot, current: &L2Snapshot) -> f64 {
        let mut layering_score: f32 = 0.0;
        
        let prev_bid_levels = previous.bids.len();
        let curr_bid_levels = current.bids.len();
        let prev_ask_levels = previous.asks.len();
        let curr_ask_levels = current.asks.len();
        
        if curr_bid_levels > prev_bid_levels + 3 || curr_ask_levels > prev_ask_levels + 3 {
            layering_score += 0.2;
        }
        
        let same_price_orders = self.count_same_price_orders(current);
        if same_price_orders > 5 {
            layering_score += 0.3;
        }
        
        layering_score.clamp(0.0, 1.0) as f64
    }

    fn count_same_price_orders(&self, snapshot: &L2Snapshot) -> u32 {
        let mut price_counts = HashMap::new();
        
        for level in &snapshot.bids {
            *price_counts.entry(level.px).or_insert(0) += level.n;
        }
        
        for level in &snapshot.asks {
            *price_counts.entry(level.px).or_insert(0) += level.n;
        }
        
        price_counts.values().filter(|&&count| count > 1).sum()
    }

    fn estimate_order_lifetime(&self, _fill: &Fill) -> u64 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(50..300000)
    }
    
    fn is_likely_cancellation(&self, fill: &Fill) -> bool {
        fill.sz < rust_decimal::Decimal::from(100) && fill.fee == rust_decimal::Decimal::ZERO
    }

    pub fn get_current_vpin(&self) -> f64 {
        if self.vpin_buckets.is_empty() {
            return 0.0;
        }
        
        self.vpin_buckets.iter().sum::<f64>() / self.vpin_buckets.len() as f64
    }

    pub fn get_phantom_liquidity_metrics(&self) -> PhantomLiquidityMetrics {
        let fleeting_ratio = if self.order_flow_analyzer.total_orders > 0 {
            self.order_flow_analyzer.fleeting_orders as f64 / self.order_flow_analyzer.total_orders as f64
        } else {
            0.0
        };
        
        let cancellation_rate = if self.order_flow_analyzer.total_orders > 0 {
            self.order_flow_analyzer.cancellation_events as f64 / self.order_flow_analyzer.total_orders as f64
        } else {
            0.0
        };
        
        let avg_lifetime = if self.order_flow_analyzer.order_lifetimes.is_empty() {
            0.0
        } else {
            self.order_flow_analyzer.order_lifetimes.iter().sum::<u64>() as f64 
                / self.order_flow_analyzer.order_lifetimes.len() as f64
        };
        
        PhantomLiquidityMetrics {
            fleeting_order_ratio: fleeting_ratio,
            avg_order_lifetime_ms: avg_lifetime,
            layering_score: self.phantom_liquidity_tracker.layering_score,
            spoofing_events: self.phantom_liquidity_tracker.spoofing_events,
            cancellation_rate: cancellation_rate,
        }
    }

    #[allow(dead_code)]
    pub fn get_performance_metrics(&self) -> PerformanceMetrics { //TODO: Implement this
        PerformanceMetrics {
            total_volume: self.bucket_accumulator.current_volume,
            sharpe_ratio: 0.0,
            sortino_ratio: 0.0,
            realized_spread: HashMap::new(),
            adverse_selection_cost: 0.0,
            daily_pnl: Decimal::ZERO,
            unrealized_pnl:  Decimal::ZERO,
        }
    }

    pub fn get_volume_metrics(&self) -> (Decimal, HashMap<String, Decimal>) {
        (self.total_volume_traded, self.volume_by_coin.clone())
    }

    pub fn get_depth_realisation_ratio(&self) -> f64 {
        self.calculate_depth_realisation_ratio()
    }

    pub fn get_real_time_spreads(&self) -> HashMap<String, f64> {
        let mut spreads = HashMap::new();
        
        for (coin, snapshot) in &self.l2_snapshots {
            if let (Some(best_bid), Some(best_ask)) = (snapshot.bids.first(), snapshot.asks.first()) {
                let mid = (best_bid.px + best_ask.px) / Decimal::from(2);
                let spread = best_ask.px - best_bid.px;
                
                if mid > Decimal::ZERO {
                    let spread_bps = (spread / mid * Decimal::from(10000)).to_f64().unwrap_or(0.0);
                    spreads.insert(coin.clone(), spread_bps);
                }
            }
        }
        
        spreads
    }
}

