use anyhow::Result;
use figment::{Figment, providers::{Format, Toml, Env}};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub operating_mode: OperatingMode,
    pub hyperliquid_api_url: String,
    pub hyperliquid_ws_url: String,
    pub user_address: String,
    pub vault_address: Option<String>,
    pub enable_websocket: bool,
    pub update_interval_ms: u64,
    pub alert_thresholds: AlertThresholds,
    pub ui_settings: UiSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperatingMode {
    Live,
    Demo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub vpin_warning: f64,
    pub vpin_critical: f64,
    pub phantom_liquidity_warning: f64,
    pub phantom_liquidity_critical: f64,
    pub liquidation_risk_warning: f64,
    pub liquidation_risk_critical: f64,
    pub max_drawdown_warning: f64,
    pub max_drawdown_critical: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    pub refresh_rate_ms: u64,
    pub theme: String,
    pub show_debug_info: bool,
    pub auto_scroll_alerts: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            operating_mode: OperatingMode::Live,
            hyperliquid_api_url: "https://api.hyperliquid.xyz".to_string(),
            hyperliquid_ws_url: "wss://api.hyperliquid.xyz/ws".to_string(),
            user_address: "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303".to_string(),
            vault_address: None,
            enable_websocket: true,
            update_interval_ms: 1000,
            alert_thresholds: AlertThresholds::default(),
            ui_settings: UiSettings::default(),
        }
    }
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            vpin_warning: 0.3,
            vpin_critical: 0.5,
            phantom_liquidity_warning: 0.4,
            phantom_liquidity_critical: 0.6,
            liquidation_risk_warning: 0.7,
            liquidation_risk_critical: 0.85,
            max_drawdown_warning: 0.15,
            max_drawdown_critical: 0.25,
        }
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            refresh_rate_ms: 100,
            theme: "dark".to_string(),
            show_debug_info: false,
            auto_scroll_alerts: true,
        }
    }
}

pub fn load_config(config_path: Option<&str>) -> Result<Config> {
    let config_file = config_path.unwrap_or("config.toml");
    
    let config: Config = Figment::new()
        .merge(Toml::file(config_file))
        .merge(Env::prefixed("HLP_"))
        .extract()?;
    
    validate_config(&config)?;
    
    Ok(config)
}

pub fn generate_sample_config() -> Result<()> {
    let config = Config::default();
    let toml_content = toml::to_string_pretty(&config)?;
    
    fs::write("config.toml", toml_content)?;
    
    Ok(())
}

fn validate_config(config: &Config) -> Result<()> {
    if config.user_address == "0x" {
        return Err(anyhow::anyhow!("Please set a valid user_address in config"));
    }
    
    if config.update_interval_ms < 100 {
        return Err(anyhow::anyhow!("update_interval_ms must be at least 100ms"));
    }
    
    if config.ui_settings.refresh_rate_ms < 50 {
        return Err(anyhow::anyhow!("UI refresh_rate_ms must be at least 50ms"));
    }
    
    Ok(())
}

pub fn save_config_to_file(config: &Config, path: &str) -> Result<()> {
    let toml_content = toml::to_string_pretty(config)?;
    std::fs::write(path, toml_content)?;
    Ok(())
}