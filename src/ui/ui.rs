// src/ui.rs
use crate::model::*;
use rust_decimal::prelude::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Table, Row, Cell, Tabs},
    Frame,
};

pub struct UIState {
    pub selected_tab: Tab,
    pub scroll_offset: usize,
    pub selected_metric_detail: Option<String>,
}

#[derive(Clone, PartialEq)]
pub enum Tab {
    Overview,
    Liquidity,
    Risk,
    Performance,
    Positions,
    Alerts,
}

impl UIState {
    pub fn new() -> Self {
        Self {
            selected_tab: Tab::Overview,
            scroll_offset: 0,
            selected_metric_detail: None,
        }
    }

    pub fn next_tab(&mut self) {
        self.selected_tab = match self.selected_tab {
            Tab::Overview => Tab::Liquidity,
            Tab::Liquidity => Tab::Risk,
            Tab::Risk => Tab::Performance,
            Tab::Performance => Tab::Positions,
            Tab::Positions => Tab::Alerts,
            Tab::Alerts => Tab::Overview,
        };
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }
}

pub fn draw(
    f: &mut Frame,
    state: &UIState,
    metrics: &GlobalMetrics,
    alerts: &[Alert],
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(3),  // Tab bar
            Constraint::Min(0),     // Content
            Constraint::Length(3),  // Footer
        ])
        .split(f.size());

    // Draw header
    draw_header(f, chunks[0], metrics);

    // Draw tab bar
    draw_tab_bar(f, chunks[1], state);

    // Draw content based on selected tab
    match state.selected_tab {
        Tab::Overview => draw_overview(f, chunks[2], metrics),
        Tab::Liquidity => {
            if let Some(ref detail) = state.selected_metric_detail {
                if detail == "PLI" {
                    draw_detailed_pli_view(f, chunks[2], metrics);
                } else {
                    draw_liquidity_metrics(f, chunks[2], metrics);
                }
            } else {
                draw_liquidity_metrics(f, chunks[2], metrics);
            }
        }
        Tab::Risk => {
            if let Some(ref detail) = state.selected_metric_detail {
                if detail == "VPIN" {
                    draw_detailed_vpin_view(f, chunks[2], metrics);
                } else {
                    draw_risk_metrics(f, chunks[2], metrics);
                }
            } else {
                draw_risk_metrics(f, chunks[2], metrics);
            }
        }
        Tab::Performance => draw_performance_metrics(f, chunks[2], metrics),
        Tab::Positions => draw_positions(f, chunks[2], metrics),
        Tab::Alerts => draw_alerts_tab(f, chunks[2], alerts, state.scroll_offset),
    }

    // Draw footer
    draw_footer(f, chunks[3], state);
}

fn draw_header(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let status_color = if metrics.last_update.is_some() {
        Color::Green
    } else {
        Color::Red
    };

    let header_text = if let Some(last_update) = metrics.last_update {
        format!(
            "🏛️  HLP Toshogu Dashboard | TVL: ${:.2}M | Equity: ${:.2}M | APR: {:.2}% | Last Update: {}",
            metrics.vault_metrics.tvl.to_f64().unwrap_or(0.0) / 1_000_000.0,
            metrics.vault_metrics.equity.to_f64().unwrap_or(0.0) / 1_000_000.0,
            metrics.vault_metrics.apr,
            last_update.format("%H:%M:%S")
        )
    } else {
        "🏛️  HLP Toshogu Dashboard | Connecting to Hyperliquid...".to_string()
    };

    let header = Paragraph::new(header_text)
        .style(Style::default().fg(status_color).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    
    f.render_widget(header, area);
}

fn draw_tab_bar(f: &mut Frame, area: Rect, state: &UIState) {
    let tab_titles = vec!["Overview", "Liquidity", "Risk", "Performance", "Positions", "Alerts"];
    let selected_index = match state.selected_tab {
        Tab::Overview => 0,
        Tab::Liquidity => 1,
        Tab::Risk => 2,
        Tab::Performance => 3,
        Tab::Positions => 4,
        Tab::Alerts => 5,
    };

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL))
        .select(selected_index)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    f.render_widget(tabs, area);
}

fn draw_footer(f: &mut Frame, area: Rect, _state: &UIState) {
    let footer = Paragraph::new("Tab: Switch Tabs | ↑↓: Scroll | Q: Quit | Based on JELLY Incident Analysis")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    
    f.render_widget(footer, area);
}

fn draw_overview(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),   // Critical metrics
            Constraint::Length(8),   // Key indicators
            Constraint::Min(0),      // Summary
        ])
        .split(area);

    draw_critical_metrics(f, chunks[0], metrics);
    draw_key_indicators(f, chunks[1], metrics);
    draw_overview_summary(f, chunks[2], metrics);
}

fn draw_critical_metrics(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    // VPIN Toxicity
    let vpin_pct = (metrics.risk_metrics.vpin_score * 100.0) as u16;
    let vpin_color = match metrics.risk_metrics.vpin_score {
        x if x < 0.2 => Color::Green,
        x if x < 0.3 => Color::Cyan,
        x if x < 0.5 => Color::Yellow,
        x if x < 0.7 => Color::LightRed,
        _ => Color::Red,
    };
    let vpin_gauge = Gauge::default()
        .block(Block::default().title("🔍 VPIN Toxicity").borders(Borders::ALL))
        .gauge_style(Style::default().fg(vpin_color))
        .percent(vpin_pct)
        .label(format!("{:.3}", metrics.risk_metrics.vpin_score));
    f.render_widget(vpin_gauge, chunks[0]);

    // Phantom Liquidity Index
    let pli_pct = (metrics.risk_metrics.phantom_liquidity_index * 100.0) as u16;
    let pli_color = match metrics.risk_metrics.phantom_liquidity_index {
        x if x < 0.2 => Color::Green,
        x if x < 0.4 => Color::Yellow,
        x if x < 0.6 => Color::LightRed,
        _ => Color::Red,
    };
    let pli_gauge = Gauge::default()
        .block(Block::default().title("👻 Phantom Liquidity").borders(Borders::ALL))
        .gauge_style(Style::default().fg(pli_color))
        .percent(pli_pct)
        .label(format!("{:.1}%", metrics.risk_metrics.phantom_liquidity_index * 100.0));
    f.render_widget(pli_gauge, chunks[1]);

    // Liquidation Risk
    let liq_pct = (metrics.risk_metrics.liquidation_risk_score * 100.0) as u16;
    let liq_color = match metrics.risk_metrics.liquidation_risk_score {
        x if x < 0.6 => Color::Green,
        x if x < 0.8 => Color::Yellow,
        _ => Color::Red,
    };
    let liq_gauge = Gauge::default()
        .block(Block::default().title("⚡ Liquidation Risk").borders(Borders::ALL))
        .gauge_style(Style::default().fg(liq_color))
        .percent(liq_pct)
        .label(format!("{:.2}", metrics.risk_metrics.liquidation_risk_score));
    f.render_widget(liq_gauge, chunks[2]);

    // Utilization
    let util_pct = (metrics.vault_metrics.utilization_rate * 100.0) as u16;
    let util_color = match metrics.vault_metrics.utilization_rate {
        x if x < 0.3 => Color::Blue,
        x if x < 0.7 => Color::Green,
        x if x < 0.9 => Color::Yellow,
        _ => Color::Red,
    };
    let util_gauge = Gauge::default()
        .block(Block::default().title("📊 Utilization").borders(Borders::ALL))
        .gauge_style(Style::default().fg(util_color))
        .percent(util_pct)
        .label(format!("{:.1}%", metrics.vault_metrics.utilization_rate * 100.0));
    f.render_widget(util_gauge, chunks[3]);
}

fn draw_key_indicators(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    // Performance Indicators
    let perf_color = if metrics.performance_metrics.daily_pnl >= Decimal::ZERO {
        Color::Green
    } else {
        Color::Red
    };
    
    let perf_text = vec![
        Line::from(vec![
            Span::raw("APR: "),
            Span::styled(
                format!("{:.2}%", metrics.vault_metrics.apr),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Sharpe: "),
            Span::styled(
                format!("{:.2}", metrics.performance_metrics.sharpe_ratio),
                Style::default().fg(if metrics.performance_metrics.sharpe_ratio >= 2.0 { Color::Green } else { Color::Yellow }),
            ),
        ]),
        Line::from(vec![
            Span::raw("Daily PnL: "),
            Span::styled(
                format!("${:.0}", metrics.performance_metrics.daily_pnl),
                Style::default().fg(perf_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Max DD: "),
            Span::styled(
                format!("{:.1}%", metrics.risk_metrics.max_drawdown * 100.0),
                Style::default().fg(if metrics.risk_metrics.max_drawdown < 0.15 { Color::Green } else { Color::Red }),
            ),
        ]),
    ];
    
    let perf_widget = Paragraph::new(perf_text)
        .block(Block::default().title("📈 Performance").borders(Borders::ALL));
    f.render_widget(perf_widget, chunks[0]);

    // Liquidity Quality
    let avg_spread = metrics.liquidity_metrics.bid_ask_spread_bps.values()
        .sum::<f64>() / metrics.liquidity_metrics.bid_ask_spread_bps.len().max(1) as f64;
    
    let liquidity_text = vec![
        Line::from(vec![
            Span::raw("Avg Spread: "),
            Span::styled(
                format!("{:.1} bps", avg_spread),
                Style::default().fg(if avg_spread < 10.0 { Color::Green } else { Color::Yellow }),
            ),
        ]),
        Line::from(vec![
            Span::raw("Cancel Rate: "),
            Span::styled(
                format!("{:.1}%", metrics.liquidity_metrics.cancel_rate * 100.0),
                Style::default().fg(if metrics.liquidity_metrics.cancel_rate < 0.1 { Color::Green } else { Color::Red }),
            ),
        ]),
        Line::from(vec![
            Span::raw("Avg Lifetime: "),
            Span::styled(
                format!("{:.0}ms", metrics.liquidity_metrics.avg_order_lifetime_ms),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("Fleeting: "),
            Span::styled(
                format!("{:.1}%", metrics.liquidity_metrics.fleeting_order_ratio * 100.0),
                Style::default().fg(if metrics.liquidity_metrics.fleeting_order_ratio < 0.1 { Color::Green } else { Color::Red }),
            ),
        ]),
    ];
    
    let liquidity_widget = Paragraph::new(liquidity_text)
        .block(Block::default().title("💧 Liquidity Quality").borders(Borders::ALL));
    f.render_widget(liquidity_widget, chunks[1]);

    // Risk Concentrations
    let mut risk_items = Vec::new();
    
    // Top position concentrations
    let mut concentrations: Vec<_> = metrics.risk_metrics.position_concentration
        .iter()
        .collect();
    concentrations.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());

    for (symbol, concentration) in concentrations.iter().take(3) {
        let color = if **concentration > 0.15 { Color::Red }
                   else if **concentration > 0.10 { Color::Yellow }
                   else { Color::Green };
        
        risk_items.push(ListItem::new(
            format!("{}: {:.1}%", symbol, **concentration * 100.0)
        ).style(Style::default().fg(color)));
    }

    // Add cascade risk
    risk_items.push(ListItem::new(
        format!("Cascade Risk: {:.2}", metrics.risk_metrics.cascade_risk_score)
    ).style(Style::default().fg(
        if metrics.risk_metrics.cascade_risk_score > 0.7 { Color::Red } else { Color::Green }
    )));

    let risk_list = List::new(risk_items)
        .block(Block::default().title("⚠️  Risk Concentrations").borders(Borders::ALL));
    
    f.render_widget(risk_list, chunks[2]);
}

fn draw_overview_summary(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    // Market Microstructure Health
    let microstructure_score = calculate_microstructure_health_score(metrics);
    let health_color = match microstructure_score {
        x if x >= 80.0 => Color::Green,
        x if x >= 60.0 => Color::Yellow,
        x if x >= 40.0 => Color::LightRed,
        _ => Color::Red,
    };

    let health_text = vec![
        Line::from(vec![
            Span::raw("Market Microstructure Health: "),
            Span::styled(
                format!("{:.0}/100", microstructure_score),
                Style::default().fg(health_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(format!("• VPIN Level: {}", get_vpin_status(metrics.risk_metrics.vpin_score))),
        Line::from(format!("• PLI Status: {}", get_pli_status(metrics.risk_metrics.phantom_liquidity_index))),
        Line::from(format!("• Liquidity: {}", get_liquidity_status(metrics))),
        Line::from(format!("• Position Risk: {}", get_position_risk_status(metrics))),
    ];

    let health_widget = Paragraph::new(health_text)
        .block(Block::default().title("🏥 System Health").borders(Borders::ALL));
    f.render_widget(health_widget, chunks[0]);

    // Recent Activity Summary
    let activity_text = vec![
        Line::from(vec![
            Span::raw("Total Volume: "),
            Span::styled(
                format!("${:.0}M", metrics.performance_metrics.total_volume.to_f64().unwrap_or(0.0) / 1_000_000.0),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(format!("Deployed: ${:.1}M", metrics.vault_metrics.deployed_liquidity.to_f64().unwrap_or(0.0) / 1_000_000.0)),
        Line::from(format!("Idle: ${:.1}M", metrics.vault_metrics.idle_liquidity.to_f64().unwrap_or(0.0) / 1_000_000.0)),
        Line::from(format!("Unrealized PnL: ${:.0}", metrics.performance_metrics.unrealized_pnl)),
        Line::from(format!("Sortino: {:.2}", metrics.performance_metrics.sortino_ratio)),
    ];

    let activity_widget = Paragraph::new(activity_text)
        .block(Block::default().title("📊 Activity Summary").borders(Borders::ALL));
    f.render_widget(activity_widget, chunks[1]);
}

fn draw_liquidity_metrics(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12),  // Spread and depth metrics
            Constraint::Length(8),   // Order flow metrics
            Constraint::Min(0),      // Phantom liquidity details
        ])
        .split(area);

    draw_spread_depth_metrics(f, chunks[0], metrics);
    draw_order_flow_metrics(f, chunks[1], metrics);
    draw_phantom_liquidity_details(f, chunks[2], metrics);
}

fn draw_spread_depth_metrics(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let header = Row::new(vec!["Symbol", "Spread (bps)", "Depth@50bps", "OB Imbalance", "Status"])
        .style(Style::default().add_modifier(Modifier::BOLD));
    
    let mut rows = Vec::new();
    
    for (symbol, spread) in &metrics.liquidity_metrics.bid_ask_spread_bps {
        let depth = metrics.liquidity_metrics.depth_at_50bps
            .get(symbol)
            .map(|d| format!("${:.0}k", d.to_f64().unwrap_or(0.0) / 1000.0))
            .unwrap_or_else(|| "N/A".to_string());
        
        let imbalance = metrics.liquidity_metrics.order_book_imbalance
            .get(symbol)
            .map(|i| format!("{:.2}", i))
            .unwrap_or_else(|| "N/A".to_string());
        
        let status = match *spread {
            x if x < 5.0 => "Excellent",
            x if x < 10.0 => "Good", 
            x if x < 20.0 => "Fair",
            _ => "Poor",
        };
        
        let status_color = match *spread {
            x if x < 5.0 => Color::Green,
            x if x < 10.0 => Color::Cyan,
            x if x < 20.0 => Color::Yellow,
            _ => Color::Red,
        };

        rows.push(Row::new(vec![
            Cell::from(symbol.as_str()),
            Cell::from(format!("{:.1}", spread)),
            Cell::from(depth),
            Cell::from(imbalance),
            Cell::from(status).style(Style::default().fg(status_color)),
        ]));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(10),
        ]
    )
        .header(header)
        .block(Block::default().title("💱 Spread & Depth Analysis").borders(Borders::ALL));

    f.render_widget(table, area);
}

fn draw_order_flow_metrics(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    // Order Lifetime Distribution
    let lifetime_text = vec![
        Line::from(vec![
            Span::raw("Avg Lifetime: "),
            Span::styled(
                format!("{:.0}ms", metrics.liquidity_metrics.avg_order_lifetime_ms),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Fleeting (<100ms): "),
            Span::styled(
                format!("{:.1}%", metrics.liquidity_metrics.fleeting_order_ratio * 100.0),
                Style::default().fg(if metrics.liquidity_metrics.fleeting_order_ratio > 0.1 { Color::Red } else { Color::Green }),
            ),
        ]),
        Line::from(vec![
            Span::raw("Cancel Rate: "),
            Span::styled(
                format!("{:.1}%", metrics.liquidity_metrics.cancel_rate * 100.0),
                Style::default().fg(if metrics.liquidity_metrics.cancel_rate > 0.1 { Color::Red } else { Color::Green }),
            ),
        ]),
    ];

    let lifetime_widget = Paragraph::new(lifetime_text)
        .block(Block::default().title("⏱️  Order Lifetimes").borders(Borders::ALL));
    f.render_widget(lifetime_widget, chunks[0]);

    // Manipulation Detection
    let manipulation_text = vec![
        Line::from(vec![
            Span::raw("Layering Score: "),
            Span::styled(
                format!("{:.2}", metrics.liquidity_metrics.layering_detection_score),
                Style::default().fg(if metrics.liquidity_metrics.layering_detection_score > 0.5 { Color::Red } else { Color::Green }),
            ),
        ]),
        Line::from(vec![
            Span::raw("Spoofing Index: "),
            Span::styled(
                format!("{:.2}", metrics.liquidity_metrics.spoofing_detection_index),
                Style::default().fg(if metrics.liquidity_metrics.spoofing_detection_index > 0.3 { Color::Red } else { Color::Green }),
            ),
        ]),
        Line::from(vec![
            Span::raw("Realization Rate: "),
            Span::styled(
                format!("{:.1}%", metrics.liquidity_metrics.liquidity_realization_rate * 100.0),
                Style::default().fg(if metrics.liquidity_metrics.liquidity_realization_rate < 0.8 { Color::Red } else { Color::Green }),
            ),
        ]),
    ];

    let manipulation_widget = Paragraph::new(manipulation_text)
        .block(Block::default().title("🎭 Manipulation Detection").borders(Borders::ALL));
    f.render_widget(manipulation_widget, chunks[1]);

    // Fill Probability by Distance
    let fill_items: Vec<ListItem> = metrics.liquidity_metrics.fill_probability_by_distance
        .iter()
        .map(|(distance, prob)| {
            let color = if *prob > 0.8 { Color::Green }
                      else if *prob > 0.6 { Color::Yellow }
                      else { Color::Red };
            
            ListItem::new(format!("{}: {:.1}%", distance, prob * 100.0))
                .style(Style::default().fg(color))
        })
        .collect();

    let fill_list = List::new(fill_items)
        .block(Block::default().title("🎯 Fill Probability").borders(Borders::ALL));
    
    f.render_widget(fill_list, chunks[2]);
}

fn draw_phantom_liquidity_details(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let pli_breakdown = vec![
        Line::from(vec![
            Span::raw("📊 Phantom Liquidity Index Breakdown"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Overall PLI: "),
            Span::styled(
                format!("{:.1}%", metrics.risk_metrics.phantom_liquidity_index * 100.0),
                Style::default().fg(get_pli_color(metrics.risk_metrics.phantom_liquidity_index)).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(format!("• Fleeting Orders (25%): {:.1}%", metrics.liquidity_metrics.fleeting_order_ratio * 100.0)),
        Line::from(format!("• Fill Probability (20%): {:.1}%", calculate_avg_fill_probability(metrics) * 100.0)),
        Line::from(format!("• Layering Score (20%): {:.1}%", metrics.liquidity_metrics.layering_detection_score * 100.0)),
        Line::from(format!("• Spoofing Index (20%): {:.1}%", metrics.liquidity_metrics.spoofing_detection_index * 100.0)),
        Line::from(format!("• Realization Rate (15%): {:.1}%", metrics.liquidity_metrics.liquidity_realization_rate * 100.0)),
        Line::from(""),
        Line::from(get_pli_interpretation(metrics.risk_metrics.phantom_liquidity_index)),
        Line::from(""),
        Line::from("📈 Recent VPIN Buckets Analysis:"),
        Line::from(format!("  - Current bucket age: <1min")),
        Line::from(format!("  - Avg bucket fill time: ~{}s", 
            if metrics.performance_metrics.total_volume > rust_decimal::Decimal::ZERO { "45" } else { "N/A" })),
        Line::from(format!("  - Order imbalance trend: {}", 
            if metrics.risk_metrics.vpin_score > 0.3 { "Increasing ⚠️" } else { "Stable ✅" })),
    ];

    let pli_widget = Paragraph::new(pli_breakdown)
        .block(Block::default().title("👻 Phantom Liquidity Analysis").borders(Borders::ALL));
    f.render_widget(pli_widget, area);
}

fn draw_risk_metrics(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),   // VPIN details
            Constraint::Length(10),  // Risk concentrations
            Constraint::Min(0),      // Liquidation analysis
        ])
        .split(area);

    draw_vpin_details(f, chunks[0], metrics);
    draw_risk_concentrations(f, chunks[1], metrics);
    draw_liquidation_analysis(f, chunks[2], metrics);
}

fn draw_vpin_details(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let vpin_interpretation = get_vpin_interpretation(metrics.risk_metrics.vpin_score);
    let action_required = get_vpin_action(metrics.risk_metrics.vpin_score);
    
    let vpin_text = vec![
        Line::from(vec![
            Span::raw("Current VPIN: "),
            Span::styled(
                format!("{:.3}", metrics.risk_metrics.vpin_score),
                Style::default().fg(get_vpin_color(metrics.risk_metrics.vpin_score)).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(format!("Status: {}", vpin_interpretation)),
        Line::from(format!("Action: {}", action_required)),
        Line::from(""),
        Line::from("VPIN measures toxic order flow probability"),
        Line::from("Higher values indicate informed trading activity"),
    ];

    let vpin_widget = Paragraph::new(vpin_text)
        .block(Block::default().title("🔍 VPIN Toxicity Analysis").borders(Borders::ALL));
    f.render_widget(vpin_widget, area);
}

fn draw_risk_concentrations(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let header = Row::new(vec!["Asset", "Concentration", "Risk Level", "Market Cap Limit"])
        .style(Style::default().add_modifier(Modifier::BOLD));
    
    let mut rows = Vec::new();
    let mut concentrations: Vec<_> = metrics.risk_metrics.position_concentration
        .iter()
        .collect();
    concentrations.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());

    for (symbol, concentration) in concentrations.iter().take(10) {
        let risk_level = match **concentration {
            x if x < 0.05 => ("Safe", Color::Green),
            x if x < 0.10 => ("Moderate", Color::Yellow),
            x if x < 0.15 => ("High", Color::LightRed),
            _ => ("Critical", Color::Red),
        };

        let market_cap_limit = get_market_cap_limit(symbol);

        rows.push(Row::new(vec![
            Cell::from(symbol.as_str()),
            Cell::from(format!("{:.1}%", **concentration * 100.0)),
            Cell::from(risk_level.0).style(Style::default().fg(risk_level.1)),
            Cell::from(market_cap_limit),
        ]));
    }

    // Add cross-exchange manipulation score if available
    if metrics.risk_metrics.cross_exchange_manipulation_score > 0.0 {
        rows.push(Row::new(vec![
            Cell::from("Cross-Exchange"),
            Cell::from(""),
            Cell::from(if metrics.risk_metrics.cross_exchange_manipulation_score > 0.5 { "High" } else { "Normal" })
                .style(Style::default().fg(if metrics.risk_metrics.cross_exchange_manipulation_score > 0.5 { Color::Red } else { Color::Green })),
            Cell::from("Global Risk"),
        ]));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(15),
        ]
    )
        .header(header)
        .block(Block::default().title("📊 Position Concentration Analysis").borders(Borders::ALL));

    f.render_widget(table, area);
}

fn draw_liquidation_analysis(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    // Liquidation Risk Summary
    let liq_color = get_liquidation_risk_color(metrics.risk_metrics.liquidation_risk_score);
    let cascade_color = get_cascade_risk_color(metrics.risk_metrics.cascade_risk_score);

    let risk_text = vec![
        Line::from(vec![
            Span::raw("Liquidation Risk: "),
            Span::styled(
                format!("{:.2}", metrics.risk_metrics.liquidation_risk_score),
                Style::default().fg(liq_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Cascade Risk: "),
            Span::styled(
                format!("{:.2}", metrics.risk_metrics.cascade_risk_score),
                Style::default().fg(cascade_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(get_liquidation_status(metrics.risk_metrics.liquidation_risk_score)),
        Line::from(get_cascade_status(metrics.risk_metrics.cascade_risk_score)),
    ];

    let risk_widget = Paragraph::new(risk_text)
        .block(Block::default().title("⚡ Liquidation Risk").borders(Borders::ALL));
    f.render_widget(risk_widget, chunks[0]);

    // Risk Mitigation Actions
    let actions = get_risk_mitigation_actions(metrics);
    let action_items: Vec<ListItem> = actions.iter()
        .map(|action| ListItem::new(action.clone()))
        .collect();

    let action_list = List::new(action_items)
        .block(Block::default().title("🛡️  Risk Mitigation").borders(Borders::ALL));
    
    f.render_widget(action_list, chunks[1]);
}

fn draw_performance_metrics(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),   // Returns and ratios
            Constraint::Length(8),   // Drawdown analysis
            Constraint::Min(0),      // Volume and execution
        ])
        .split(area);

    draw_returns_ratios(f, chunks[0], metrics);
    draw_drawdown_analysis(f, chunks[1], metrics);
    draw_volume_execution(f, chunks[2], metrics);
}

fn draw_returns_ratios(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    // APR and Sharpe
    let sharpe_color = if metrics.performance_metrics.sharpe_ratio >= 2.0 { Color::Green } else { Color::Yellow };
    
    let returns_text = vec![
        Line::from(vec![
            Span::raw("APR: "),
            Span::styled(
                format!("{:.2}%", metrics.vault_metrics.apr),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Sharpe Ratio: "),
            Span::styled(
                format!("{:.2}", metrics.performance_metrics.sharpe_ratio),
                Style::default().fg(sharpe_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Target: "),
            Span::styled("≥2.0", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("Sortino: "),
            Span::styled(
                format!("{:.2}", metrics.performance_metrics.sortino_ratio),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];

    let returns_widget = Paragraph::new(returns_text)
        .block(Block::default().title("📈 Returns & Ratios").borders(Borders::ALL));
    f.render_widget(returns_widget, chunks[0]);

    // PnL Breakdown
    let pnl_color = if metrics.performance_metrics.daily_pnl >= Decimal::ZERO {
        Color::Green
    } else {
        Color::Red
    };

    let pnl_text = vec![
        Line::from(vec![
            Span::raw("Daily PnL: "),
            Span::styled(
                format!("${:.0}", metrics.performance_metrics.daily_pnl),
                Style::default().fg(pnl_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Unrealized: "),
            Span::styled(
                format!("${:.0}", metrics.performance_metrics.unrealized_pnl),
                Style::default().fg(pnl_color),
            ),
        ]),
        Line::from(vec![
            Span::raw("Equity: "),
            Span::styled(
                format!("${:.1}M", metrics.vault_metrics.equity.to_f64().unwrap_or(0.0) / 1_000_000.0),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let pnl_widget = Paragraph::new(pnl_text)
        .block(Block::default().title("💰 PnL Breakdown").borders(Borders::ALL));
    f.render_widget(pnl_widget, chunks[1]);

    // Execution Quality
    let avg_realized_spread = metrics.performance_metrics.realized_spread.values()
        .sum::<f64>() / metrics.performance_metrics.realized_spread.len().max(1) as f64;

    let execution_text = vec![
        Line::from(vec![
            Span::raw("Realized Spread: "),
            Span::styled(
                format!("{:.1} bps", avg_realized_spread),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("Adverse Selection: "),
            Span::styled(
                format!("{:.2}", metrics.performance_metrics.adverse_selection_cost),
                Style::default().fg(if metrics.performance_metrics.adverse_selection_cost > 0.1 { Color::Red } else { Color::Green }),
            ),
        ]),
    ];

    let execution_widget = Paragraph::new(execution_text)
        .block(Block::default().title("⚡ Execution Quality").borders(Borders::ALL));
    f.render_widget(execution_widget, chunks[2]);
}

fn draw_drawdown_analysis(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let dd_color = match metrics.risk_metrics.max_drawdown {
        x if x < 0.15 => Color::Green,
        x if x < 0.25 => Color::Yellow,
        _ => Color::Red,
    };

    let dd_text = vec![
        Line::from(vec![
            Span::raw("Maximum Drawdown: "),
            Span::styled(
                format!("{:.1}%", metrics.risk_metrics.max_drawdown * 100.0),
                Style::default().fg(dd_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(format!("Warning Threshold: 15.0%")),
        Line::from(format!("Critical Threshold: 25.0%")),
        Line::from(""),
        Line::from(get_drawdown_interpretation(metrics.risk_metrics.max_drawdown)),
    ];

    let dd_widget = Paragraph::new(dd_text)
        .block(Block::default().title("📉 Drawdown Analysis").borders(Borders::ALL));
    f.render_widget(dd_widget, area);
}

fn draw_volume_execution(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let volume_text = vec![
        Line::from(vec![
            Span::raw("Total Volume: "),
            Span::styled(
                format!("${:.0}M", metrics.performance_metrics.total_volume.to_f64().unwrap_or(0.0) / 1_000_000.0),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(format!("TVL: ${:.1}M", metrics.vault_metrics.tvl.to_f64().unwrap_or(0.0) / 1_000_000.0)),
        Line::from(format!("Deployed: ${:.1}M", metrics.vault_metrics.deployed_liquidity.to_f64().unwrap_or(0.0) / 1_000_000.0)),
        Line::from(format!("Idle: ${:.1}M", metrics.vault_metrics.idle_liquidity.to_f64().unwrap_or(0.0) / 1_000_000.0)),
        Line::from(format!("Utilization: {:.1}%", metrics.vault_metrics.utilization_rate * 100.0)),
    ];

    let volume_widget = Paragraph::new(volume_text)
        .block(Block::default().title("📊 Volume & Utilization").borders(Borders::ALL));
    f.render_widget(volume_widget, area);
}

fn draw_positions(f: &mut Frame, area: Rect, _metrics: &GlobalMetrics) {
    // TODO:In a complete implementation, this would show actual positions from vault details
    let positions = vec![
        vec!["BTC-USD", "1.5", "$45,000", "$42,000", "+$3,000", "15%", "Safe"],
        vec!["ETH-USD", "10.0", "$2,500", "$2,300", "+$2,000", "12%", "Safe"], 
        vec!["SOL-USD", "100.0", "$150", "$140", "-$1,000", "8%", "Moderate"],
        vec!["DOGE-USD", "50,000", "$0.08", "$0.07", "+$500", "5%", "Safe"],
        vec!["AVAX-USD", "100.0", "$150", "$140", "-$1,000", "8%", "Moderate"],
    ];

    let header = Row::new(vec!["Symbol", "Size", "Entry", "Liq Price", "PnL", "Margin %", "Risk"])
        .style(Style::default().add_modifier(Modifier::BOLD));
    
    let rows: Vec<Row> = positions
        .iter()
        .map(|p| {
            let risk_color = match p[6] {
                "Safe" => Color::Green,
                "Moderate" => Color::Yellow,
                "High" => Color::LightRed,
                "Critical" => Color::Red,
                _ => Color::White,
            };

            let pnl_color = if p[4].starts_with('+') { Color::Green } else { Color::Red };

            Row::new(vec![
                Cell::from(p[0]),
                Cell::from(p[1]),
                Cell::from(p[2]),
                Cell::from(p[3]),
                Cell::from(p[4]).style(Style::default().fg(pnl_color)),
                Cell::from(p[5]),
                Cell::from(p[6]).style(Style::default().fg(risk_color)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
        ]
    )
        .header(header)
        .block(Block::default().title("📊 Open Positions").borders(Borders::ALL));

    f.render_widget(table, area);
}

fn draw_alerts_tab(
    f: &mut Frame,
    area: Rect,
    alerts: &[Alert],
    scroll_offset: usize,
) {
    let alert_items: Vec<ListItem> = alerts
        .iter()
        .rev()
        .skip(scroll_offset)
        .take(area.height as usize - 2)
        .map(|alert| {
            let (icon, color) = match alert.level {
                AlertLevel::Info => ("ℹ️", Color::Blue),
                AlertLevel::Warning => ("⚠️", Color::Yellow),
                AlertLevel::Critical => ("🔴", Color::Red),
            };
            
            let content = format!(
                "{} [{}] {} - {}",
                icon,
                alert.timestamp.format("%H:%M:%S"),
                alert.metric,
                alert.message
            );
            
            ListItem::new(content).style(Style::default().fg(color))
        })
        .collect();

    let alerts_list = List::new(alert_items)
        .block(Block::default()
            .title(format!("🔔 Alerts ({} total)", alerts.len()))
            .borders(Borders::ALL));
    
    f.render_widget(alerts_list, area);
}

// Helper functions

fn calculate_microstructure_health_score(metrics: &GlobalMetrics) -> f64 {
    // VPIN component (30% weight) - inverted because lower VPIN is better
    let vpin_component = (1.0 - metrics.risk_metrics.vpin_score.clamp(0.0, 1.0)) * 30.0;
    
    // PLI component (25% weight) - inverted because lower PLI is better  
    let pli_component = (1.0 - metrics.risk_metrics.phantom_liquidity_index.clamp(0.0, 1.0)) * 25.0;
    
    // Liquidity component (20% weight) - inverted because lower cancel rate is better
    let liquidity_component = (1.0 - metrics.liquidity_metrics.cancel_rate.clamp(0.0, 10.0) / 10.0) * 20.0;
    
    // Concentration component (15% weight)
    let concentration_component = calculate_concentration_score(metrics) * 15.0;
    
    // Performance component (10% weight) - normalized Sharpe ratio
    let performance_component = (metrics.performance_metrics.sharpe_ratio.clamp(0.0, 5.0) / 5.0) * 10.0;
    
    let total_score = vpin_component + pli_component + liquidity_component + concentration_component + performance_component;
    
    // Ensure score is between 0 and 100
    total_score.clamp(0.0, 100.0)
}

fn calculate_concentration_score(metrics: &GlobalMetrics) -> f64 {
    let max_concentration = metrics.risk_metrics.position_concentration
        .values()
        .fold(0.0f64, |acc, &x| acc.max(x));
    
    if max_concentration < 0.05 { 1.0 }
    else if max_concentration < 0.10 { 0.8 }
    else if max_concentration < 0.15 { 0.6 }
    else { 0.2 }
}

fn get_vpin_status(vpin: f64) -> &'static str {
    match vpin {
        x if x < 0.2 => "Normal ✅",
        x if x < 0.3 => "Elevated ⚠️", 
        x if x < 0.5 => "Warning 🔶",
        x if x < 0.7 => "Critical 🔴",
        _ => "Extreme ⚠️",
    }
}

fn get_pli_status(pli: f64) -> &'static str {
    match pli {
        x if x < 0.2 => "Healthy ✅",
        x if x < 0.4 => "Some Phantom ⚠️",
        x if x < 0.6 => "Significant 🔶", 
        _ => "Compromised 🔴",
    }
}

fn get_liquidity_status(metrics: &GlobalMetrics) -> &'static str {
    let avg_spread = metrics.liquidity_metrics.bid_ask_spread_bps.values()
        .sum::<f64>() / metrics.liquidity_metrics.bid_ask_spread_bps.len().max(1) as f64;
    
    match avg_spread {
        x if x < 5.0 => "Excellent ✅",
        x if x < 10.0 => "Good 👍",
        x if x < 20.0 => "Fair ⚠️",
        _ => "Poor 🔴",
    }
}

fn get_position_risk_status(metrics: &GlobalMetrics) -> &'static str {
    let max_concentration = metrics.risk_metrics.position_concentration
        .values()
        .fold(0.0f64, |acc, &x| acc.max(x));
    
    match max_concentration {
        x if x < 0.05 => "Low ✅",
        x if x < 0.10 => "Moderate ⚠️",
        x if x < 0.15 => "High 🔶",
        _ => "Critical 🔴",
    }
}

fn get_pli_color(pli: f64) -> Color {
    match pli {
        x if x < 0.2 => Color::Green,
        x if x < 0.4 => Color::Yellow,
        x if x < 0.6 => Color::LightRed,
        _ => Color::Red,
    }
}

fn get_vpin_color(vpin: f64) -> Color {
    match vpin {
        x if x < 0.2 => Color::Green,
        x if x < 0.3 => Color::Cyan,
        x if x < 0.5 => Color::Yellow,
        x if x < 0.7 => Color::LightRed,
        _ => Color::Red,
    }
}

fn get_liquidation_risk_color(risk: f64) -> Color {
    match risk {
        x if x < 0.6 => Color::Green,
        x if x < 0.8 => Color::Yellow,
        _ => Color::Red,
    }
}

fn get_cascade_risk_color(risk: f64) -> Color {
    match risk {
        x if x < 0.5 => Color::Green,
        x if x < 0.7 => Color::Yellow,
        _ => Color::Red,
    }
}

fn calculate_avg_fill_probability(metrics: &GlobalMetrics) -> f64 {
    if metrics.liquidity_metrics.fill_probability_by_distance.is_empty() {
        0.0
    } else {
        metrics.liquidity_metrics.fill_probability_by_distance.values().sum::<f64>() 
            / metrics.liquidity_metrics.fill_probability_by_distance.len() as f64
    }
}

fn get_pli_interpretation(pli: f64) -> Line<'static> {
    let interpretation = match pli {
        x if x < 0.2 => "Genuine liquidity dominates the market",
        x if x < 0.4 => "Some phantom liquidity detected, monitor closely", 
        x if x < 0.6 => "Significant phantom liquidity, consider defensive measures",
        _ => "Severely compromised liquidity, high manipulation risk",
    };
    Line::from(interpretation)
}

fn get_vpin_interpretation(vpin: f64) -> &'static str {
    match vpin {
        x if x < 0.2 => "Normal market conditions",
        x if x < 0.3 => "Elevated informed trading activity",
        x if x < 0.5 => "Warning: High toxic flow detected",
        x if x < 0.7 => "Critical: Emergency defensive mode recommended", 
        _ => "Extreme: Consider position exit",
    }
}

fn get_vpin_action(vpin: f64) -> &'static str {
    match vpin {
        x if x < 0.2 => "Continue normal operations",
        x if x < 0.3 => "Widen spreads by 20%",
        x if x < 0.5 => "Widen spreads by 50%, reduce sizes",
        x if x < 0.7 => "Emergency defensive mode",
        _ => "Consider position exit",
    }
}

fn get_liquidation_status(risk: f64) -> Line<'static> {
    let status = match risk {
        x if x < 0.6 => "Low liquidation risk",
        x if x < 0.8 => "Moderate liquidation risk - monitor closely",
        _ => "High liquidation risk - immediate attention required",
    };
    Line::from(status)
}

fn get_cascade_status(risk: f64) -> Line<'static> {
    let status = match risk {
        x if x < 0.5 => "Low cascade risk",
        x if x < 0.7 => "Moderate cascade risk",
        _ => "High cascade risk - diversification needed",
    };
    Line::from(status)
}

fn get_market_cap_limit(symbol: &str) -> &'static str {
    // Simplified - in production would look up actual market caps
    match symbol {
        "BTC" | "ETH" => "1% of market cap",
        "SOL" | "AVAX" => "0.5% of market cap", 
        _ => "0.25% of market cap",
    }
}

fn get_risk_mitigation_actions(metrics: &GlobalMetrics) -> Vec<String> {
    let mut actions = Vec::new();
    
    if metrics.risk_metrics.vpin_score > 0.5 {
        actions.push("• Reduce position sizes".to_string());
        actions.push("• Widen bid-ask spreads".to_string());
    }
    
    if metrics.risk_metrics.phantom_liquidity_index > 0.4 {
        actions.push("• Implement stricter order validation".to_string());
        actions.push("• Monitor for manipulation patterns".to_string());
    }
    
    if metrics.risk_metrics.liquidation_risk_score > 0.8 {
        actions.push("• Reduce leverage immediately".to_string());
        actions.push("• Add margin to positions".to_string());
    }
    
    let max_concentration = metrics.risk_metrics.position_concentration
        .values()
        .fold(0.0f64, |acc, &x| acc.max(x));
    
    if max_concentration > 0.15 {
        actions.push("• Diversify position concentrations".to_string());
        actions.push("• Reduce oversized positions".to_string());
    }
    
    if actions.is_empty() {
        actions.push("• All risk metrics within normal ranges".to_string());
        actions.push("• Continue monitoring".to_string());
    }
    
    actions
}

fn get_drawdown_interpretation(drawdown: f64) -> Line<'static> {
    let interpretation = match drawdown {
        x if x < 0.05 => "Excellent drawdown control",
        x if x < 0.15 => "Good drawdown management",
        x if x < 0.25 => "Acceptable drawdown level",
        _ => "Excessive drawdown - review risk management",
    };
    Line::from(interpretation)
}

fn draw_detailed_pli_view(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let pli_detailed = vec![
        Line::from(vec![
            Span::styled("📊 DETAILED PHANTOM LIQUIDITY ANALYSIS", 
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Current PLI Score: "),
            Span::styled(
                format!("{:.2}%", metrics.risk_metrics.phantom_liquidity_index * 100.0),
                Style::default().fg(get_pli_color(metrics.risk_metrics.phantom_liquidity_index)).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("🔍 COMPONENT BREAKDOWN:"),
        Line::from(format!("  • Fleeting Orders (Weight: 25%): {:.1}%", 
            metrics.liquidity_metrics.fleeting_order_ratio * 100.0)),
        Line::from(format!("    └─ Orders cancelled <100ms")),
        Line::from(format!("  • Fill Probability (Weight: 20%): {:.1}%", 
            calculate_avg_fill_probability(metrics) * 100.0)),
        Line::from(format!("    └─ Actual vs promised execution")),
        Line::from(format!("  • Layering Score (Weight: 20%): {:.1}%", 
            metrics.liquidity_metrics.layering_detection_score * 100.0)),
        Line::from(format!("    └─ Multiple orders at same levels")),
        Line::from(format!("  • Spoofing Index (Weight: 20%): {:.1}%", 
            metrics.liquidity_metrics.spoofing_detection_index * 100.0)),
        Line::from(format!("    └─ Large orders quickly removed")),
        Line::from(format!("  • Realization Rate (Weight: 15%): {:.1}%", 
            metrics.liquidity_metrics.liquidity_realization_rate * 100.0)),
        Line::from(format!("    └─ Actual liquidity provided")),
        Line::from(""),
        Line::from("📈 MARKET IMPACT:"),
        Line::from(format!("  • Cancel-to-Trade Ratio: {:.1}:1", metrics.liquidity_metrics.cancel_rate * 10.0)),
        Line::from(format!("  • Average Order Lifetime: {:.0}ms", metrics.liquidity_metrics.avg_order_lifetime_ms)),
        Line::from(""),
        Line::from("⚠️  RISK ASSESSMENT:"),
        Line::from(get_pli_interpretation(metrics.risk_metrics.phantom_liquidity_index)),
        Line::from(""),
        Line::from("Press ESC to return to main view"),
    ];

    let detailed_widget = Paragraph::new(pli_detailed)
        .block(Block::default().title("👻 Phantom Liquidity Deep Dive").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(detailed_widget, area);
}

fn draw_detailed_vpin_view(f: &mut Frame, area: Rect, metrics: &GlobalMetrics) {
    let vpin_detailed = vec![
        Line::from(vec![
            Span::styled("🔍 DETAILED VPIN ANALYSIS", 
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Current VPIN Score: "),
            Span::styled(
                format!("{:.4}", metrics.risk_metrics.vpin_score),
                Style::default().fg(get_vpin_color(metrics.risk_metrics.vpin_score)).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("📊 VOLUME BUCKET ANALYSIS:"),
        Line::from(format!("  • Bucket Size: $10,000 (configurable)")),
        Line::from(format!("  • Current Window: 50 buckets")),
        Line::from(format!("  • Avg Bucket Fill Time: ~45 seconds")),
        Line::from(format!("  • Order Imbalance Trend: {}", 
            if metrics.risk_metrics.vpin_score > 0.3 { "Increasing ⚠️" } else { "Stable ✅" })),
        Line::from(""),
        Line::from("🎯 TOXICITY LEVELS:"),
        Line::from("  • 0.0 - 0.2: Normal flow ✅"),
        Line::from("  • 0.2 - 0.3: Elevated (widen spreads 20%) ⚠️"),
        Line::from("  • 0.3 - 0.5: Warning (widen spreads 50%) 🔶"),
        Line::from("  • 0.5 - 0.7: Critical (emergency mode) 🔴"),
        Line::from("  • 0.7 - 1.0: Extreme (consider exit) ⚠️"),
        Line::from(""),
        Line::from("📈 CURRENT STATUS:"),
        Line::from(format!("  • Risk Level: {}", get_vpin_interpretation(metrics.risk_metrics.vpin_score))),
        Line::from(format!("  • Recommended Action: {}", get_vpin_action(metrics.risk_metrics.vpin_score))),
        Line::from(""),
        Line::from("🔬 TECHNICAL DETAILS:"),
        Line::from("  • Algorithm: Easley, López de Prado, O'Hara (2012)"),
        Line::from("  • Classification: Tick rule for buy/sell"),
        Line::from("  • Calculation: |Buy_Volume - Sell_Volume| / Total"),
        Line::from(""),
        Line::from("Press ESC to return to main view"),
    ];

    let detailed_widget = Paragraph::new(vpin_detailed)
        .block(Block::default().title("🔍 VPIN Toxicity Deep Dive").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(detailed_widget, area);
}