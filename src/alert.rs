use crate::model::{Alert, AlertLevel, GlobalMetrics};
use chrono::Utc;
use uuid::Uuid;

pub fn check_alerts(metrics: &GlobalMetrics) -> Vec<Alert> {
    let mut alerts = Vec::new();
    
    if metrics.risk_metrics.vpin_score > 0.7 {
        alerts.push(create_alert(
            AlertLevel::Critical,
            "VPIN".to_string(),
            format!("Extreme toxic flow detected: {:.3}", metrics.risk_metrics.vpin_score),
            metrics.risk_metrics.vpin_score,
            0.7,
        ));
    } else if metrics.risk_metrics.vpin_score > 0.5 {
        alerts.push(create_alert(
            AlertLevel::Warning,
            "VPIN".to_string(),
            format!("High toxic flow detected: {:.3}", metrics.risk_metrics.vpin_score),
            metrics.risk_metrics.vpin_score,
            0.5,
        ));
    }
    
    if metrics.risk_metrics.phantom_liquidity_index > 0.6 {
        alerts.push(create_alert(
            AlertLevel::Critical,
            "Phantom Liquidity".to_string(),
            format!("Severely compromised liquidity: {:.1}%", metrics.risk_metrics.phantom_liquidity_index * 100.0),
            metrics.risk_metrics.phantom_liquidity_index,
            0.6,
        ));
    } else if metrics.risk_metrics.phantom_liquidity_index > 0.4 {
        alerts.push(create_alert(
            AlertLevel::Warning,
            "Phantom Liquidity".to_string(),
            format!("Significant phantom liquidity: {:.1}%", metrics.risk_metrics.phantom_liquidity_index * 100.0),
            metrics.risk_metrics.phantom_liquidity_index,
            0.4,
        ));
    }
    
    if metrics.risk_metrics.liquidation_risk_score > 0.85 {
        alerts.push(create_alert(
            AlertLevel::Critical,
            "Liquidation Risk".to_string(),
            format!("Critical liquidation risk: {:.2}", metrics.risk_metrics.liquidation_risk_score),
            metrics.risk_metrics.liquidation_risk_score,
            0.85,
        ));
    } else if metrics.risk_metrics.liquidation_risk_score > 0.7 {
        alerts.push(create_alert(
            AlertLevel::Warning,
            "Liquidation Risk".to_string(),
            format!("Elevated liquidation risk: {:.2}", metrics.risk_metrics.liquidation_risk_score),
            metrics.risk_metrics.liquidation_risk_score,
            0.7,
        ));
    }
    
    if metrics.risk_metrics.max_drawdown > 0.25 {
        alerts.push(create_alert(
            AlertLevel::Critical,
            "Max Drawdown".to_string(),
            format!("Excessive drawdown: {:.1}%", metrics.risk_metrics.max_drawdown * 100.0),
            metrics.risk_metrics.max_drawdown,
            0.25,
        ));
    } else if metrics.risk_metrics.max_drawdown > 0.15 {
        alerts.push(create_alert(
            AlertLevel::Warning,
            "Max Drawdown".to_string(),
            format!("High drawdown: {:.1}%", metrics.risk_metrics.max_drawdown * 100.0),
            metrics.risk_metrics.max_drawdown,
            0.15,
        ));
    }
    
    if metrics.vault_metrics.utilization_rate > 0.9 {
        alerts.push(create_alert(
            AlertLevel::Warning,
            "Utilization".to_string(),
            format!("High capital utilization: {:.1}%", metrics.vault_metrics.utilization_rate * 100.0),
            metrics.vault_metrics.utilization_rate,
            0.9,
        ));
    }
    
    let max_concentration = metrics.risk_metrics.position_concentration
        .values()
        .fold(0.0f64, |acc, &x| acc.max(x));
    
    if max_concentration > 0.15 {
        alerts.push(create_alert(
            AlertLevel::Warning,
            "Position Concentration".to_string(),
            format!("High position concentration: {:.1}%", max_concentration * 100.0),
            max_concentration,
            0.15,
        ));
    }
    
    if metrics.liquidity_metrics.cancel_rate > 0.5 {
        alerts.push(create_alert(
            AlertLevel::Warning,
            "Cancel Rate".to_string(),
            format!("High order cancel rate: {:.1}%", metrics.liquidity_metrics.cancel_rate * 100.0),
            metrics.liquidity_metrics.cancel_rate,
            0.5,
        ));
    }
    
    if metrics.liquidity_metrics.fleeting_order_ratio > 0.2 {
        alerts.push(create_alert(
            AlertLevel::Warning,
            "Fleeting Orders".to_string(),
            format!("High fleeting order ratio: {:.1}%", metrics.liquidity_metrics.fleeting_order_ratio * 100.0),
            metrics.liquidity_metrics.fleeting_order_ratio,
            0.2,
        ));
    }
    
    if metrics.performance_metrics.sharpe_ratio < 1.0 {
        alerts.push(create_alert(
            AlertLevel::Info,
            "Sharpe Ratio".to_string(),
            format!("Low Sharpe ratio: {:.2}", metrics.performance_metrics.sharpe_ratio),
            metrics.performance_metrics.sharpe_ratio,
            1.0,
        ));
    }
    
    alerts
}

fn create_alert(
    level: AlertLevel,
    metric: String,
    message: String,
    value: f64,
    threshold: f64,
) -> Alert {
    Alert {
        id: Uuid::new_v4().to_string(),
        level,
        metric,
        message,
        timestamp: Utc::now(),
        value,
        threshold,
    }
}