use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultDetails {
    pub vault_address: String,
    pub name: String,
    pub description: String,
    pub manager: String,
    pub max_capacity: Decimal,
    pub min_deposit: Decimal,
    pub management_fee: f64,
    pub performance_fee: f64,
    pub inception_date: u64,
    pub status: VaultStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VaultStatus {
    Active,
    Paused,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultPerformance {
    pub daily_returns: Vec<f64>,
    pub weekly_returns: Vec<f64>,
    pub monthly_returns: Vec<f64>,
    pub cumulative_return: f64,
    pub volatility: f64,
    pub beta: f64,
    pub alpha: f64,
    pub information_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultHoldings {
    pub cash: Decimal,
    pub positions: Vec<VaultPosition>,
    pub total_exposure: Decimal,
    pub net_exposure: Decimal,
    pub gross_exposure: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultPosition {
    pub symbol: String,
    pub quantity: Decimal,
    pub market_value: Decimal,
    pub weight: f64,
    pub unrealized_pnl: Decimal,
    pub entry_price: Decimal,
    pub current_price: Decimal,
}