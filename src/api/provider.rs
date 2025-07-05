use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::model::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataSourceStatus {
    Connected,
    Disconnected,
    Error(String),
}

#[async_trait]
#[allow(dead_code)]
pub trait DataProvider {
    async fn get_vault_summary(&self) -> Result<VaultSummary>;
    async fn get_user_state(&self) -> Result<UserState>;
    async fn get_meta(&self) -> Result<Meta>;
    async fn get_recent_fills(&self) -> Result<Vec<Fill>>;
    async fn get_l2_snapshots(&self) -> Result<HashMap<String, L2Snapshot>>;
    async fn get_status(&self) -> DataSourceStatus;
    
    fn as_any(&self) -> &dyn std::any::Any;
}


pub fn parse_decimal(s: &str) -> rust_decimal::Decimal {
    s.parse().unwrap_or_else(|_| rust_decimal::Decimal::ZERO)
}