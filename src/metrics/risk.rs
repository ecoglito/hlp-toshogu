use crate::model::*;
use rust_decimal::prelude::*;
use std::collections::HashMap;

pub fn calculate_vpin(fills: &[Fill], meta: &Meta) -> f64 {
    if fills.is_empty() {
        return 0.0;
    }
    
    let bucket_size = Decimal::from(10000);
    let mut buckets = Vec::new();
    let mut current_bucket_volume = Decimal::ZERO;
    let mut current_buy_volume = Decimal::ZERO;
    let mut current_sell_volume = Decimal::ZERO;
    
    let major_assets: std::collections::HashSet<String> = meta.universe
        .iter()
        .filter(|asset| asset.max_leverage >= 10)
        .map(|asset| asset.name.clone())
        .collect();
    
    for fill in fills {
        if !major_assets.contains(&fill.coin) {
            continue;
        }
        
        let volume = fill.px * fill.sz.abs();
        
        if fill.side == "B" {
            current_buy_volume += volume;
        } else {
            current_sell_volume += volume;
        }
        
        current_bucket_volume += volume;
        
        if current_bucket_volume >= bucket_size {
            let imbalance = (current_buy_volume - current_sell_volume).abs();
            let total_volume = current_buy_volume + current_sell_volume;
            
            if total_volume > Decimal::ZERO {
                let vpin = (imbalance / total_volume).to_f64().unwrap_or(0.0);
                buckets.push(vpin);
            }
            
            current_bucket_volume = Decimal::ZERO;
            current_buy_volume = Decimal::ZERO;
            current_sell_volume = Decimal::ZERO;
        }
    }
    
    if buckets.is_empty() {
        return 0.0;
    }
    
    let window_size = 50.min(buckets.len());
    let recent_buckets = &buckets[buckets.len().saturating_sub(window_size)..];
    
    recent_buckets.iter().sum::<f64>() / recent_buckets.len() as f64
}

pub fn calculate_phantom_liquidity_index(liquidity_metrics: &LiquidityMetrics) -> f64 {
    let fleeting_weight = 0.25;
    let fill_prob_weight = 0.20;
    let layering_weight = 0.20;
    let spoofing_weight = 0.20;
    let realization_weight = 0.15;
    
    let avg_fill_prob = if liquidity_metrics.fill_probability_by_distance.is_empty() {
        0.0
    } else {
        liquidity_metrics.fill_probability_by_distance.values().sum::<f64>() 
            / liquidity_metrics.fill_probability_by_distance.len() as f64
    };
    
    let phantom_score = 
        (liquidity_metrics.fleeting_order_ratio * fleeting_weight) +
        ((1.0 - avg_fill_prob) * fill_prob_weight) +
        (liquidity_metrics.layering_detection_score * layering_weight) +
        (liquidity_metrics.spoofing_detection_index * spoofing_weight) +
        ((1.0 - liquidity_metrics.liquidity_realization_rate) * realization_weight);
    
    phantom_score.clamp(0.0, 1.0)
}

pub fn calculate_liquidation_risk(vault_summary: &VaultSummary) -> f64 {
    if vault_summary.tvl == Decimal::ZERO {
        return 1.0;
    }
    
    let equity_ratio = (vault_summary.equity / vault_summary.tvl).to_f64().unwrap_or(0.0);
    let drawdown_factor = vault_summary.max_drawdown.clamp(0.0, 1.0);
    
    let base_risk = 1.0 - equity_ratio;
    let adjusted_risk = base_risk + (drawdown_factor * 0.5);
    
    adjusted_risk.clamp(0.0, 1.0)
}

pub fn calculate_cascade_risk(fills: &[Fill], meta: &Meta) -> f64 {
    if fills.is_empty() {
        return 0.0;
    }
    
    let mut position_sizes: HashMap<String, Decimal> = HashMap::new();
    let major_assets: std::collections::HashSet<String> = meta.universe
        .iter()
        .filter(|asset| asset.max_leverage >= 5)
        .map(|asset| asset.name.clone())
        .collect();
    
    for fill in fills {
        if !major_assets.contains(&fill.coin) {
            continue;
        }
        
        let entry = position_sizes.entry(fill.coin.clone()).or_insert(Decimal::ZERO);
        if fill.side == "B" {
            *entry += fill.sz;
        } else {
            *entry -= fill.sz;
        }
    }
    
    let total_exposure = position_sizes.values()
        .map(|size| size.abs())
        .sum::<Decimal>();
    
    if total_exposure == Decimal::ZERO {
        return 0.0;
    }
    
    let concentration_risk = position_sizes.values()
        .map(|size| {
            let weight = (size.abs() / total_exposure).to_f64().unwrap_or(0.0);
            weight * weight
        })
        .sum::<f64>();
    
    let correlation_factor = calculate_asset_correlation(&major_assets);
    let liquidity_factor = 0.8;
    
    let cascade_risk = concentration_risk * correlation_factor * liquidity_factor;
    cascade_risk.clamp(0.0, 1.0)
}

pub fn calculate_position_concentration(fills: &[Fill], meta: &Meta) -> HashMap<String, f64> {
    let mut concentrations = HashMap::new();
    let mut position_values: HashMap<String, Decimal> = HashMap::new();
    
    let tradeable_assets: std::collections::HashSet<String> = meta.universe
        .iter()
        .map(|asset| asset.name.clone())
        .collect();
    
    for fill in fills {
        if !tradeable_assets.contains(&fill.coin) {
            continue;
        }
        
        let value = fill.px * fill.sz.abs();
        let entry = position_values.entry(fill.coin.clone()).or_insert(Decimal::ZERO);
        *entry += value;
    }
    
    let total_value = position_values.values().sum::<Decimal>();
    
    if total_value > Decimal::ZERO {
        for (coin, value) in position_values {
            let concentration = (value / total_value).to_f64().unwrap_or(0.0);
            concentrations.insert(coin, concentration);
        }
    }
    
    concentrations
}

pub fn detect_cross_exchange_manipulation(fills: &[Fill], meta: &Meta) -> f64 {
    if fills.is_empty() {
        return 0.0;
    }
    
    let major_assets_count = meta.universe
        .iter()
        .filter(|asset| asset.max_leverage >= 10)
        .count();
    
    let unusual_pattern_score = if major_assets_count > 10 { 0.15 } else { 0.08 };
    unusual_pattern_score
}

fn calculate_asset_correlation(assets: &std::collections::HashSet<String>) -> f64 {
    let correlation_pairs = vec![
        ("BTC", "ETH", 0.7),
        ("ETH", "SOL", 0.6),
        ("BTC", "SOL", 0.5),
        ("ETH", "AVAX", 0.8),
        ("SOL", "AVAX", 0.7),
        ("BTC", "DOGE", 0.4),
        ("ETH", "MATIC", 0.6),
        ("LINK", "UNI", 0.5),
        ("AAVE", "COMP", 0.7),
    ];
    
    let mut total_correlation = 0.0;
    let mut pair_count = 0;
    
    for (asset1, asset2, corr) in correlation_pairs {
        if assets.contains(asset1) && assets.contains(asset2) {
            total_correlation += corr;
            pair_count += 1;
        }
    }
    
    if pair_count == 0 {
        0.5
    } else {
        total_correlation / pair_count as f64
    }
}