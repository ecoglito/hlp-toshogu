use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen},
};
use log::{debug, error, info, warn};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::{
    io,
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use rust_decimal::prelude::*;
use tui_logger;

mod api;
mod config;
mod model;
mod metrics;
mod ui;
mod alert;

use config::{Config, OperatingMode};
use api::provider::DataProvider;
use model::*;
use ui::ui::UIState;
use metrics::streaming::StreamingMetricsEngine;



#[derive(Parser)]
#[command(name = "hlp-toshogu")]
#[command(about = "HLP Toshogu Terminal Dashboard for Hyperliquid")]
struct Args {
    #[arg(long)]
    generate_config: bool,
    
    #[arg(short, long)]
    config: Option<String>,
    
    #[arg(long)]
    test_mode: bool,
    
    #[arg(long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {


    let args = Args::parse();
    
    if args.debug {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        tui_logger::init_logger(log::LevelFilter::Debug).unwrap();
        tui_logger::set_default_level(log::LevelFilter::Debug);
    }
    
    print_startup_banner();
    
    if args.generate_config {
        config::generate_sample_config()?;
        println!("✅ Sample configuration generated at config.toml");
        return Ok(());
    }
    
    let config = config::load_config(args.config.as_deref())?;
    
    match config.operating_mode {
        OperatingMode::Live => run_live_mode(config, args.test_mode, args.debug).await,
        OperatingMode::Demo => run_demo_mode(config, args.test_mode, args.debug).await,
    }
}

pub fn print_startup_banner() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    HLP TOSHOGU DASHBOARD                     ║");
    println!("║                                                              ║");
    println!("║           Advanced Market Microstructure Monitoring          ║");
    println!("║              Post-JELLY Incident Risk Management             ║");
    println!("║                                                              ║");
    println!("║  Metrics: PLI | VPIN | Liquidation Risk | Phantom Liquidity  ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
}

async fn run_live_mode(config: Config, test_mode: bool, debug_mode: bool) -> Result<()> {
    info!("🚀 Starting live mode (test_mode: {}, debug: {})", test_mode, debug_mode);
    
    let provider = api::sdk::HyperliquidProvider::new(&config).await?;
    run_dashboard(provider, config, test_mode, debug_mode).await
}

async fn run_demo_mode(config: Config, test_mode: bool, debug_mode: bool) -> Result<()> {
    info!("🧪 Starting demo mode (test_mode: {}, debug: {})", test_mode, debug_mode);
    
    if test_mode {
        info!("📊 Using simulated test data for demo mode");
        run_test_dashboard(config, debug_mode).await
    } else {
        eprintln!("❌ Demo mode requires --test-mode flag");
        std::process::exit(1);
    }
}

async fn run_dashboard<P: DataProvider + Send + Sync + 'static>(
    provider: P, 
    config: Config,
    test_mode: bool,
    debug_mode: bool,
) -> Result<()> {
    let provider = Arc::new(provider);
    let metrics = Arc::new(RwLock::new(GlobalMetrics::default()));
    let alerts = Arc::new(RwLock::new(Vec::<Alert>::new()));
    
    let metrics_clone = metrics.clone();
    let alerts_clone = alerts.clone();
    let provider_clone = provider.clone();
    let config_clone = config.clone();
    
    tokio::spawn(async move {
        data_collection_loop(provider_clone, metrics_clone, alerts_clone, config_clone, test_mode).await;
    });
    
    run_ui_enhanced(metrics, alerts, config, test_mode, debug_mode).await?;
    
    Ok(())
}

async fn run_test_dashboard(config: Config, debug_mode: bool) -> Result<()> {
    let metrics = Arc::new(RwLock::new(GlobalMetrics::default()));
    let alerts = Arc::new(RwLock::new(Vec::<Alert>::new()));
    
    let metrics_clone = metrics.clone();
    let alerts_clone = alerts.clone();
    let config_clone = config.clone();
    
    tokio::spawn(async move {
        test_data_loop(metrics_clone, alerts_clone, config_clone).await;
    });
    
    run_ui_enhanced(metrics, alerts, config, true, debug_mode).await?;
    
    Ok(())
}

async fn data_collection_loop<P: DataProvider>(
    provider: Arc<P>,
    metrics: Arc<RwLock<GlobalMetrics>>,
    alerts: Arc<RwLock<Vec<Alert>>>,
    config: Config,
    test_mode: bool,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(config.update_interval_ms));
    let mut update_counter = 0;
    
    info!("📡 Starting data collection loop (interval: {}ms, test_mode: {})", 
          config.update_interval_ms, test_mode);
    
    let streaming_metrics = if config.enable_websocket {
        if let Some(hyperliquid_provider) = provider.as_any().downcast_ref::<crate::api::sdk::HyperliquidProvider>() {
            if let (Some(trade_rx), Some(l2_rx), Some(order_rx)) = (hyperliquid_provider.get_live_trades(), hyperliquid_provider.get_live_l2_updates(), hyperliquid_provider.get_live_orders()) {
                info!("🔄 Starting streaming metrics engine");            


                let streaming_engine = Arc::new(RwLock::new(crate::metrics::streaming::StreamingMetricsEngine::new()));

                let engine_arc = Arc::clone(&streaming_engine);
                tokio::spawn(async move {
                    StreamingMetricsEngine::run(engine_arc, trade_rx, l2_rx, order_rx).await;
                });
                Some(streaming_engine)
            } else {
                warn!("⚠️ Websocket streams not available, falling back to polling");
                None
            }
        } else {
            warn!("⚠️ Provider doesn't support streaming, falling back to polling");
            None
        }
    } else {
        None
    };
    
    loop {
        interval.tick().await;
        update_counter += 1;
        
        debug!("📊 Starting metrics update cycle #{}", update_counter);
        
        match update_metrics(&*provider, &streaming_metrics).await {
            Ok(new_metrics) => {
                info!("✅ Successfully updated metrics from provider");
                
                {
                    let mut metrics_guard = metrics.write().await;
                    *metrics_guard = new_metrics;
                    
                    if test_mode {
                        apply_test_modifications(&mut metrics_guard, update_counter);
                        debug!("🧪 Applied test modifications to metrics");
                    }
                    
                    info!("📊 FINAL METRICS - TVL: ${:.1}M, VPIN: {:.3}, PLI: {:.1}%, Spreads: {}", 
                           metrics_guard.vault_metrics.tvl.to_f64().unwrap_or(0.0) / 1_000_000.0,
                           metrics_guard.risk_metrics.vpin_score,
                           metrics_guard.risk_metrics.phantom_liquidity_index * 100.0,
                           metrics_guard.liquidity_metrics.bid_ask_spread_bps.len());
                }
                
                let metrics_for_alerts = metrics.read().await.clone();
                let new_alerts = alert::check_alerts(&metrics_for_alerts);
                if !new_alerts.is_empty() {
                    info!("🔔 Generated {} new alerts", new_alerts.len());
                    let mut alerts_guard = alerts.write().await;
                    alerts_guard.extend(new_alerts);
                    if alerts_guard.len() > 1000 {
                        alerts_guard.drain(0..500);
                    }
                }
                
                if update_counter % 10 == 0 {
                    let metrics_guard = metrics.read().await;
                    info!("📊 Data update #{} - VPIN: {:.3}, PLI: {:.1}%, TVL: ${:.1}M", 
                           update_counter, 
                           metrics_guard.risk_metrics.vpin_score,
                           metrics_guard.risk_metrics.phantom_liquidity_index * 100.0,
                           metrics_guard.vault_metrics.tvl.to_f64().unwrap_or(0.0) / 1_000_000.0);
                }
            }
            Err(e) => {
                error!("❌ Failed to update metrics (attempt #{}): {}", update_counter, e);
                
                if update_counter % 5 == 0 {
                    warn!("⚠️ Metrics update has been failing for {} attempts", update_counter);
                }
                
                if test_mode {
                    warn!("🧪 Test mode enabled but real data fetch failed, falling back to test data");
                    let mut test_metrics = create_test_metrics(update_counter);
                    apply_test_modifications(&mut test_metrics, update_counter);
                    
                    let mut metrics_guard = metrics.write().await;
                    *metrics_guard = test_metrics;
                }
            }
        }
    }
}

async fn test_data_loop(
    metrics: Arc<RwLock<GlobalMetrics>>,
    alerts: Arc<RwLock<Vec<Alert>>>,
    config: Config,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(config.update_interval_ms));
    let mut update_counter = 0;
    
    info!("🧪 Starting test data loop");
    
    loop {
        interval.tick().await;
        update_counter += 1;
        
        let mut test_metrics = create_test_metrics(update_counter);
        apply_test_modifications(&mut test_metrics, update_counter);
        
        {
            let mut metrics_guard = metrics.write().await;
            *metrics_guard = test_metrics;
            
            debug!("🧪 Test update #{} - Generated metrics: VPIN: {:.3}, PLI: {:.1}%", 
                   update_counter,
                   metrics_guard.risk_metrics.vpin_score,
                   metrics_guard.risk_metrics.phantom_liquidity_index * 100.0);
        }
        
        let metrics_for_alerts = metrics.read().await.clone();
        let new_alerts = alert::check_alerts(&metrics_for_alerts);
        if !new_alerts.is_empty() {
            let mut alerts_guard = alerts.write().await;
            alerts_guard.extend(new_alerts);
            if alerts_guard.len() > 100 {
                alerts_guard.drain(0..50);
            }
        }
        
        if update_counter % 30 == 0 {
            info!("🧪 Test update #{} - Simulated metrics generated", update_counter);
        }
    }
}

fn apply_test_modifications(metrics: &mut GlobalMetrics, counter: u32) {
    let time_factor = (counter as f64 * 0.1).sin();
    
    metrics.risk_metrics.vpin_score = (0.3 + time_factor * 0.4).max(0.0).min(1.0);
    metrics.risk_metrics.phantom_liquidity_index = (0.25 + time_factor * 0.2).max(0.0).min(1.0);
    metrics.risk_metrics.liquidation_risk_score = (0.2 + time_factor * 0.3).max(0.0).min(1.0);
    metrics.vault_metrics.utilization_rate = (0.5 + time_factor * 0.3).max(0.0).min(1.0);
    
    if counter % 50 == 0 {
        metrics.risk_metrics.vpin_score = 0.8; 
    }
    
    metrics.last_update = Some(chrono::Utc::now());
    
    debug!("🧪 Modified test metrics - VPIN: {:.3}, PLI: {:.1}%", 
           metrics.risk_metrics.vpin_score,
           metrics.risk_metrics.phantom_liquidity_index * 100.0);
}

fn create_test_metrics(counter: u32) -> GlobalMetrics {
    use rust_decimal::Decimal;
    use std::collections::HashMap;
    
    let mut metrics = GlobalMetrics::default();
    
    metrics.vault_metrics.tvl = Decimal::from(109530000);
    metrics.vault_metrics.equity = Decimal::from(373090000);
    metrics.vault_metrics.apr = 5.76;
    
    metrics.performance_metrics.sharpe_ratio = 2.21;
    metrics.performance_metrics.sortino_ratio = 2.65;
    metrics.performance_metrics.daily_pnl = Decimal::from(15000);
    metrics.performance_metrics.unrealized_pnl = Decimal::from(8500);
    metrics.performance_metrics.total_volume = Decimal::from(25000000);
    
    let mut spreads = HashMap::new();
    spreads.insert("BTC".to_string(), 0.5);
    spreads.insert("ETH".to_string(), 0.8);
    spreads.insert("SOL".to_string(), 1.2);
    metrics.liquidity_metrics.bid_ask_spread_bps = spreads;
    
    let mut depth_50bps = HashMap::new();
    depth_50bps.insert("BTC".to_string(), Decimal::from(500000));
    depth_50bps.insert("ETH".to_string(), Decimal::from(300000));
    depth_50bps.insert("SOL".to_string(), Decimal::from(150000));
    metrics.liquidity_metrics.depth_at_50bps = depth_50bps;
    
    let mut order_book_imbalance = HashMap::new();
    order_book_imbalance.insert("BTC".to_string(), 0.05);
    order_book_imbalance.insert("ETH".to_string(), 0.03);
    order_book_imbalance.insert("SOL".to_string(), 0.08);
    metrics.liquidity_metrics.order_book_imbalance = order_book_imbalance;
    
    let mut fill_probability = HashMap::new();
    fill_probability.insert("5bps".to_string(), 0.95);
    fill_probability.insert("10bps".to_string(), 0.88);
    fill_probability.insert("25bps".to_string(), 0.75);
    fill_probability.insert("50bps".to_string(), 0.60);
    metrics.liquidity_metrics.fill_probability_by_distance = fill_probability;
    
    metrics.liquidity_metrics.avg_order_lifetime_ms = 164170.0;
    metrics.liquidity_metrics.cancel_rate = 0.45;
    metrics.liquidity_metrics.fleeting_order_ratio = 0.093;
    metrics.liquidity_metrics.layering_detection_score = 0.35;
    metrics.liquidity_metrics.spoofing_detection_index = 0.09;
    metrics.liquidity_metrics.liquidity_realization_rate = 0.512;
    
    let mut concentrations = HashMap::new();
    concentrations.insert("BTC".to_string(), 0.08);
    concentrations.insert("ETH".to_string(), 0.06);
    concentrations.insert("SOL".to_string(), 0.04);
    metrics.risk_metrics.position_concentration = concentrations;
    
    metrics.risk_metrics.cascade_risk_score = 0.12;
    metrics.risk_metrics.max_drawdown = 0.0;
    
    metrics.vault_metrics.deployed_liquidity = Decimal::from(85000000);
    metrics.vault_metrics.idle_liquidity = Decimal::from(24530000);
    
    let mut realized_spreads = HashMap::new();
    realized_spreads.insert("BTC".to_string(), 0.3);
    realized_spreads.insert("ETH".to_string(), 0.5);
    realized_spreads.insert("SOL".to_string(), 0.8);
    metrics.performance_metrics.realized_spread = realized_spreads;
    
    metrics.performance_metrics.adverse_selection_cost = 0.05;
    
    metrics.last_update = Some(chrono::Utc::now());
    
    debug!("🧪 Created test metrics #{} - TVL: ${:.1}M", 
           counter, metrics.vault_metrics.tvl.to_f64().unwrap_or(0.0) / 1_000_000.0);
    
    metrics
}

async fn run_ui_enhanced(
    metrics: Arc<RwLock<GlobalMetrics>>,
    alerts: Arc<RwLock<Vec<Alert>>>,
    config: Config,
    test_mode: bool,
    debug_mode: bool,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut ui_state = UIState::new();
    let mut last_alert_count = 0;
    let mut update_counter = 0;

    show_loading_screen(&mut terminal, &config, test_mode)?;
    tokio::time::sleep(Duration::from_millis(1500)).await;

    info!("🎨 Starting UI loop (test_mode: {}, debug: {})", test_mode, debug_mode);

    loop {
        update_counter += 1;

        let metrics_snapshot = {
            let guard = metrics.read().await;
            guard.clone()
        };
        
        let alerts_snapshot = {
            let guard = alerts.read().await;
            guard.clone()
        };
        
        if debug_mode && update_counter % 100 == 0 {
            debug!("📊 UI Update #{} - VPIN: {:.3}, PLI: {:.1}%, Last Update: {:?}", 
                   update_counter,
                   metrics_snapshot.risk_metrics.vpin_score,
                   metrics_snapshot.risk_metrics.phantom_liquidity_index * 100.0,
                   metrics_snapshot.last_update);
        }
        
        check_critical_alerts(&alerts_snapshot, &mut last_alert_count);
        
        terminal.draw(|f| ui::ui::draw(f, &ui_state, &metrics_snapshot, &alerts_snapshot))?;

        if event::poll(Duration::from_millis(config.ui_settings.refresh_rate_ms))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.is_empty() {
                            info!("👋 User requested quit");
                            break;
                        }
                    }
                    KeyCode::Esc => {
                        info!("👋 User pressed escape");
                        break;
                    }
                    KeyCode::Tab => {
                        ui_state.next_tab();
                        debug!("📑 Switched to next tab");
                    }
                    KeyCode::Up => ui_state.scroll_up(),
                    KeyCode::Down => ui_state.scroll_down(),
                    KeyCode::PageUp => {
                        for _ in 0..10 {
                            ui_state.scroll_up();
                        }
                    }
                    KeyCode::PageDown => {
                        for _ in 0..10 {
                            ui_state.scroll_down();
                        }
                    }
                    KeyCode::Home => ui_state.scroll_offset = 0,
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        info!("🔄 User requested refresh");
                        ui_state.scroll_offset = 0;
                    }
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        info!("❓ Showing help screen");
                        show_help_screen(&mut terminal, test_mode, debug_mode)?;
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        info!("💾 Saving configuration");
                        if let Err(e) = config::save_config_to_file(&config, "config.toml") {
                            error!("❌ Failed to save configuration: {}", e);
                        } else {
                            info!("✅ Configuration saved to config.toml");
                        }
                    }
                    KeyCode::Char('t') | KeyCode::Char('T') => {
                        info!("🧪 Running manual test calculations");
                        {
                            let mut metrics_guard = metrics.write().await;
                            apply_test_modifications(&mut metrics_guard, update_counter);
                            info!("✅ Test metrics applied");
                        }
                    }
                    KeyCode::F(5) => {
                        info!("🔄 Force refresh requested");
                        ui_state.scroll_offset = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn show_loading_screen(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, 
    config: &Config,
    test_mode: bool
) -> Result<()> {
    use ratatui::{
        layout::{Alignment, Constraint, Direction, Layout},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Paragraph},
    };

    terminal.draw(|f| {
        let size = f.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Length(14),
                Constraint::Percentage(56),
            ])
            .split(size);

        let title = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("🏛️  HLP TOSHOGU DASHBOARD", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Advanced Market Microstructure Monitoring", Style::default().fg(Color::White))
            ]),
            Line::from(vec![
                Span::styled("Post-JELLY Incident Risk Management", Style::default().fg(Color::Yellow))
            ]),
        ])
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        let mode_text = if test_mode { "TEST MODE - Simulated Data" } else { "PRODUCTION MODE - Live Data" };
        let mode_color = if test_mode { Color::Yellow } else { Color::Green };
        
        let loading = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(mode_text, Style::default().fg(mode_color).add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("API Endpoint: "),
                Span::styled(config.hyperliquid_api_url.clone(), Style::default().fg(Color::Cyan))
            ]),
            Line::from(vec![
                Span::raw("User Address: "),
                Span::styled(config.user_address.clone(), Style::default().fg(Color::Yellow))
            ]),
            Line::from(vec![
                Span::raw("Update Interval: "),
                Span::styled(format!("{}ms", config.update_interval_ms), Style::default().fg(Color::Green))
            ]),
            Line::from(""),
            Line::from("Initializing metrics..."),
            Line::from("• Phantom Liquidity Index (PLI)"),
            Line::from("• VPIN Toxicity Detection"),
            Line::from("• Position Concentration Analysis"),
            Line::from("• Liquidation Risk Assessment"),
            Line::from("• Order Flow Quality Monitoring"),
        ])
        .alignment(Alignment::Center)
        .block(Block::default().title("Initializing").borders(Borders::ALL));

        f.render_widget(title, chunks[0]);
        f.render_widget(loading, chunks[1]);
    })?;

    Ok(())
}

fn show_help_screen(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    test_mode: bool,
    debug_mode: bool
) -> Result<()> {
    use ratatui::{
        layout::{Alignment, Constraint, Direction, Layout},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Paragraph},
    };

    terminal.draw(|f| {
        let size = f.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(size);

        let title = Paragraph::new(format!("Help - HLP Toshogu Dashboard ({})", 
                                         if test_mode { "TEST MODE" } else { "PRODUCTION MODE" }))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        let help_text = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("NAVIGATION", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from("Tab                 - Switch between tabs"),
            Line::from("↑/↓ Arrow Keys      - Scroll content"),
            Line::from("Page Up/Page Down   - Fast scroll"),
            Line::from("Home                - Jump to top"),
            Line::from(""),
            Line::from(vec![
                Span::styled("CONTROLS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from("Q or Ctrl+Q         - Quit application"),
            Line::from("Esc                 - Quit application"),
            Line::from("R                   - Reset scroll position"),
            Line::from("H                   - Show this help"),
            Line::from("S                   - Save configuration"),
            Line::from("T                   - Run test calculations"),
            Line::from("F5                  - Force refresh"),
            Line::from(""),
            Line::from(vec![
                Span::styled("CURRENT SESSION", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(format!("Mode: {}", if test_mode { "TEST (Simulated Data)" } else { "PRODUCTION (Live Data)" })),
            Line::from(format!("Debug: {}", if debug_mode { "ENABLED" } else { "DISABLED" })),
            Line::from(""),
            Line::from(vec![
                Span::styled("TABS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from("Overview            - Key metrics and system health"),
            Line::from("Liquidity           - Spread analysis and phantom liquidity"),
            Line::from("Risk                - VPIN, liquidation, and concentration risk"),
            Line::from("Performance         - Returns, Sharpe ratio, and drawdowns"),
            Line::from("Positions           - Open positions and margin usage"),
            Line::from("Alerts              - Real-time alert feed"),
            Line::from(""),
            Line::from(vec![
                Span::styled("METRICS LEGEND", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(vec![
                Span::styled("VPIN", Style::default().fg(Color::Cyan)), 
                Span::raw("              - Volume-Synchronized Probability of Informed Trading")
            ]),
            Line::from(vec![
                Span::styled("PLI", Style::default().fg(Color::Cyan)), 
                Span::raw("               - Phantom Liquidity Index (0-100%)")
            ]),
            Line::from(vec![
                Span::styled("Cascade Risk", Style::default().fg(Color::Cyan)), 
                Span::raw("      - Liquidation cascade probability")
            ]),
            Line::from(""),
            Line::from("Press any key to return to dashboard..."),
        ])
        .block(Block::default().borders(Borders::ALL));

        let footer = Paragraph::new("Based on JELLY Incident Analysis - Advanced Market Microstructure Monitoring")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(title, chunks[0]);
        f.render_widget(help_text, chunks[1]);
        f.render_widget(footer, chunks[2]);
    })?;

    loop {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(_) = event::read()? {
                break;
            }
        }
    }

    Ok(())
}

fn check_critical_alerts(alerts: &[Alert], last_count: &mut usize) {
    let critical_alerts: Vec<_> = alerts.iter()
        .filter(|alert| alert.level == AlertLevel::Critical)
        .collect();

    let current_critical_count = critical_alerts.len();
    
    if current_critical_count > *last_count {
        let new_alerts_count = current_critical_count - *last_count;
        warn!("🔴 {} new critical alert(s) detected!", new_alerts_count);
        
        for alert in critical_alerts.iter().rev().take(new_alerts_count) {
            error!("CRITICAL: {} - {}", alert.metric, alert.message);
        }
    }
    
    *last_count = current_critical_count;
}

async fn update_metrics<P: DataProvider>(
    provider: &P,
    streaming_metrics: &Option<Arc<RwLock<crate::metrics::streaming::StreamingMetricsEngine>>>
) -> Result<GlobalMetrics> {
    debug!("📊 Fetching data from provider...");
    
    let vault_summary = provider.get_vault_summary().await
        .map_err(|e| {
            error!("❌ Failed to get vault summary: {}", e);
            e
        })?;
    
    let user_state = provider.get_user_state().await
        .map_err(|e| {
            error!("❌ Failed to get user state: {}", e);
            e
        })?;
    
    let meta = provider.get_meta().await
        .map_err(|e| {
            error!("❌ Failed to get meta: {}", e);
            e
        })?;
    
    let recent_fills = provider.get_recent_fills().await
        .map_err(|e| {
            error!("❌ Failed to get recent fills: {}", e);
            e
        })?;
    
    let l2_snapshots = provider.get_l2_snapshots().await
        .map_err(|e| {
            error!("❌ Failed to get L2 snapshots: {}", e);
            e
        })?;
    
    debug!("📊 Successfully fetched all data, calculating metrics...");
    
    let vault_metrics = metrics::calculate_vault_metrics(&vault_summary, &user_state);
    let performance_metrics = metrics::calculate_performance_metrics(&recent_fills, &vault_summary);
    let liquidity_metrics = metrics::calculate_liquidity_metrics(&l2_snapshots, &recent_fills, &meta);
    let risk_metrics = metrics::calculate_risk_metrics(&vault_summary, &recent_fills, &liquidity_metrics, &meta);
    
    let mut global_metrics = GlobalMetrics {
        vault_metrics,
        performance_metrics,
        liquidity_metrics,
        risk_metrics,
        last_update: Some(chrono::Utc::now()),
    };
    
    if let Some(ref engine) = streaming_metrics {
        debug!("📊 Integrating streaming metrics...");
        let engine_guard = engine.read().await;
        
        let streaming_vpin = engine_guard.get_current_vpin();
        let phantom_metrics = engine_guard.get_phantom_liquidity_metrics();
        let real_time_spreads = engine_guard.get_real_time_spreads();
        let (streaming_volume, _ ) = engine_guard.get_volume_metrics();
        let liquidity_realization_rate = engine_guard.get_depth_realisation_ratio();
        
        drop(engine_guard);
        
        debug!("📊 Streaming data - VPIN: {:.3}, Fleeting: {:.1}%, Spreads: {}, Volume: {:.1}M", 
               streaming_vpin, phantom_metrics.fleeting_order_ratio * 100.0, real_time_spreads.len(), streaming_volume);
        
        global_metrics.risk_metrics.vpin_score = streaming_vpin;
        global_metrics.risk_metrics.phantom_liquidity_index = phantom_metrics.fleeting_order_ratio;
        
        global_metrics.liquidity_metrics.fleeting_order_ratio = phantom_metrics.fleeting_order_ratio;
        global_metrics.liquidity_metrics.avg_order_lifetime_ms = phantom_metrics.avg_order_lifetime_ms;
        global_metrics.liquidity_metrics.layering_detection_score = phantom_metrics.layering_score;
        global_metrics.liquidity_metrics.spoofing_detection_index = phantom_metrics.spoofing_events as f64;
        global_metrics.liquidity_metrics.cancel_rate = phantom_metrics.cancellation_rate;
        
        global_metrics.liquidity_metrics.liquidity_realization_rate = liquidity_realization_rate;
        
        global_metrics.risk_metrics.phantom_liquidity_index = {
            let depth_penalty    = 1.0 - global_metrics.liquidity_metrics.liquidity_realization_rate;
            let spoof_penalty    = (phantom_metrics.spoofing_events as f64 / 50.0).tanh();
            let layering_penalty = phantom_metrics.layering_score;
            let flow_penalty     =
                0.5 * phantom_metrics.fleeting_order_ratio + 0.5 * phantom_metrics.cancellation_rate;
        
            (depth_penalty + spoof_penalty + layering_penalty + flow_penalty) / 4.0
        };
        
        for (coin, spread) in real_time_spreads {
            global_metrics.liquidity_metrics.bid_ask_spread_bps.insert(coin, spread);
        }

        global_metrics.performance_metrics.total_volume += streaming_volume;

        
        global_metrics.vault_metrics.tvl = vault_summary.tvl;
        global_metrics.vault_metrics.equity = vault_summary.equity;
        global_metrics.vault_metrics.apr = vault_summary.apr;
        global_metrics.vault_metrics.deployed_liquidity = vault_summary.deployed_liquidity;
        global_metrics.vault_metrics.idle_liquidity = vault_summary.idle_liquidity;
        global_metrics.vault_metrics.utilization_rate = 1.0 - (vault_summary.idle_liquidity / vault_summary.tvl).to_f64().unwrap_or(0.0);
 
       
        
        debug!("📊 Successfully integrated streaming metrics");
    }
    
    debug!("📊 Calculated metrics successfully");
    
    Ok(global_metrics)
}

#[allow(dead_code)]
async fn debug_metrics_state(metrics: &Arc<RwLock<GlobalMetrics>>) {
    let m = metrics.read().await;
    eprintln!("🔍 DEBUG METRICS STATE:");
    eprintln!("  VPIN: {:.3}", m.risk_metrics.vpin_score);
    eprintln!("  PLI: {:.1}%", m.risk_metrics.phantom_liquidity_index * 100.0);
    eprintln!("  TVL: ${:.1}M", m.vault_metrics.tvl.to_f64().unwrap_or(0.0) / 1_000_000.0);
}