# HLP Toshogu Terminal Dashboard

HLP-TOSHOGU is a real-time terminal dashboard for monitoring vault performance and market microstructure on Hyperliquid's perpetual futures DEX. It provides advanced metrics including VPIN (Volume-Synchronized Probability of Informed Trading), phantom liquidity detection, and liquidation cascade risk assessment.

## 🚀 Quick Start

### 1. Installation

```bash
# Clone the repository
git clone https://github.com/ironcrypto/hlp-toshogu.git
cd hlp-toshogu

# Build the project
cargo build --release
```

### 2. Configuration

```bash
# Generate a sample configuration file
cargo run -- --generate-config

# Edit the configuration
vim config.toml
```

### 3. Configuration Setup

Set your Hyperliquid user address in `config.toml`. By default the dashboard tracks HLP Vault Address.

```toml
[default]
operating_mode = "Live"
hyperliquid_api_url = "https://api.hyperliquid.xyz"
hyperliquid_ws_url = "wss://api.hyperliquid.xyz/ws"
user_address = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303"
enable_websocket = false
update_interval_ms = 1000

[alert_thresholds]
vpin_warning = 0.3
vpin_critical = 0.5
phantom_liquidity_warning = 0.4
phantom_liquidity_critical = 0.6
liquidation_risk_warning = 0.7
liquidation_risk_critical = 0.85
max_drawdown_warning = 0.15
max_drawdown_critical = 0.25

[ui_settings]
refresh_rate_ms = 100
theme = "dark"
show_debug_info = false
auto_scroll_alerts = true
```

### 4. Run the Dashboard

```bash
# Run with default config
cargo run --release

# Run with custom config
cargo run --release -- --config my-config.toml

# Run in debug with logs saving
cargo run -- --debug 2> logs/mylog.log
```

## 🎯 Key Features

### **Market Microstructure Analysis**
- **VPIN (Volume-Synchronized Probability of Informed Trading)**: Real-time calculation from live trades using Easley, López de Prado, O'Hara methodology
- **Phantom Liquidity Index**: Detection of fake/fleeting liquidity with pattern recognition
- **Order Flow Toxicity**: Monitoring of spoofing, layering, and manipulation patterns
- **Fill Probability Analysis**: Real execution vs promised liquidity

### **Risk Management**
- **Position Concentration Analysis**: Real-time monitoring of vault exposures
- **Liquidation Cascade Modeling**: Advanced risk assessment
- **Real-time Margin Utilization**: Live tracking of capital deployment
- **Automated Risk Alerts**: Multi-threshold warning system

### **Performance Analytics**
- **Sharpe & Sortino Ratios**: Risk-adjusted performance metrics
- **Drawdown Analysis**: Maximum and current drawdown tracking
- **Realized Spread Analysis**: Execution quality measurement
- **Adverse Selection Cost**: Market impact assessment

## 📊 Dashboard Tabs

### Overview Tab
- Critical metrics at a glance
- Market microstructure health score
- System status indicators
- Real-time alerts feed

### Liquidity Tab
- Spread & depth analysis by asset
- Order lifetime distributions
- Manipulation detection scores
- Phantom liquidity breakdown

### Risk Tab
- VPIN toxicity analysis with deep dive
- Position concentration matrix
- Liquidation risk assessment
- Risk mitigation recommendations

### Performance Tab
- Returns & risk-adjusted ratios
- PnL breakdown and attribution
- Execution quality metrics
- Volume & utilization tracking

### Positions Tab
- Real-time position overview
- Entry prices and unrealized PnL
- Margin utilization by position
- Risk classification per asset

### Alerts Tab
- Real-time alert stream
- Alert history with timestamps
- Severity-based color coding
- Scrollable alert history

## 🔧 Technical Architecture

### Project Structure
```
hlp-toshigo/
├── src/
│   ├── api/
│   │   ├── mod.rs          # API module exports
│   │   ├── provider.rs     # Data provider trait definition
│   │   └── sdk.rs          # Hyperliquid SDK implementation
│   │
│   ├── metrics/
│   │   ├── mod.rs          # Metrics calculation functions
│   │   ├── risk.rs         # Risk metrics (VPIN, liquidation risk)
│   │   └── streaming.rs    # Real-time streaming metrics engine
│   │
│   ├── model/
│   │   ├── mod.rs          # Data model exports
│   │   └── vault.rs        # Vault and market data structures
│   │
│   ├── ui/
│   │   ├── mod.rs          # UI module exports
│   │   └── ui.rs           # Ratatui-based terminal interface
│   │
│   ├── alert.rs            # Alert generation and management
│   ├── config.rs           # Configuration management
│   ├── lib.rs              # Library exports
│   └── main.rs             # Application entry point
│
├── config.toml             # Runtime configuration
├── Cargo.toml              # Dependencies and metadata
└── README.md               # This file
```


### Data Flow
1. **SDK Integration**: Real-time data via Hyperliquid REST API
2. **Metric Calculation**: Advanced algorithms process raw data
3. **Risk Assessment**: Multi-layered risk analysis
4. **Alert Generation**: Threshold-based warnings
5. **UI Rendering**: Real-time dashboard updates

## 📈 Performance Metrics

### **System Requirements**
- **Memory**: ~50MB baseline, scales with trade history
- **CPU**: <5% on modern systems
- **Network**: HTTP/WebSocket connection
- **Storage**: Minimal (logs and configuration only)

### **Performance Characteristics**
- **UI Refresh**: 100ms default (configurable)
- **Alert Response**: <1s from trigger to display
- **API Latency**: <100ms for most endpoints
- **Memory Management**: Automatic cleanup (1-hour retention)

## 🔮 Advanced Features

### **VPIN Implementation**
Based on Easley, López de Prado, O'Hara (2012) methodology:
- Volume bucket synchronization ($10k default)
- 50-bucket rolling window
- Buy/sell classification via tick rule
- Real-time toxicity scoring

### **Phantom Liquidity Detection**
Multi-component analysis:
- Fleeting orders (<100ms lifetime) - 25% weight
- Fill probability vs promised depth - 20% weight  
- Layering detection - 20% weight
- Spoofing patterns - 20% weight
- Liquidity realization rate - 15% weight

### **Risk Management Algorithms**
- **Liquidation Risk**: Equity ratio + drawdown factors
- **Cascade Risk**: Concentration + correlation modeling
- **Cross-Exchange**: Manipulation detection across venues

## 🎮 Controls

| Key | Action |
|-----|--------|
| `Tab` | Switch between dashboard tabs |
| `↑/↓` | Scroll through content |
| `Q` / `Esc` | Quit application |

## 📚 Academic References

This implementation leverages rigorous academic research:

- **VPIN Methodology**: Easley, D., López de Prado, M., & O'Hara, M. (2012). "The Volume Clock: Insights into the High Frequency Paradigm"
- **Phantom Liquidity**: Ye, M., Yao, C., & Gai, J. (2013). "The Externalities of High Frequency Trading"
- **Market Microstructure**: O'Hara, M. (2015). "High Frequency Market Microstructure"
- **JELLY Incident Analysis**: Hyperliquid Team (2024). Post-incident technical analysis

## 🛠️ Development

### Building from Source

```bash
# Development build
cargo build

# Release build with optimizations
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```


## 📝 Configuration Reference

### Operating Modes
- **Live**: Real Hyperliquid API data
- **Demo**: Simulated data (not implemented)

### Alert Thresholds
Customize warning and critical levels for:
- VPIN toxicity scores
- Phantom liquidity percentages  
- Liquidation risk levels
- Maximum drawdown limits

### UI Settings
- Refresh rate (50ms minimum)
- Color themes
- Debug information display
- Auto-scroll behaviors

## ⚠️ DISCLAIMER

**This is a PROTOTYPE implementation and should not be used for production trading decisions.**

### Current Limitations

This dashboard is in early development with several known issues and incomplete features:

#### UI/Display Issues
- **Rendering glitches**: The terminal UI occasionally has rendering artifacts, especially when resizing the terminal or during high-frequency updates
- **stderr/stdout conflicts**: Log messages may overlay the UI in some configurations (use `2> logs/debug.log` to redirect)

#### Missing Features
- **No position tracking**: The Positions tab is non-functional as wallet address integration is not implemented
- **Performance metrics incomplete**: The following metrics are placeholders:
  - Sharpe/Sortino ratios
  - Daily/Unrealized PnL
  - Historical performance tracking
  - Actual vault APR calculations

#### What IS Working
The development focus has been on real-time market microstructure analysis:
- ✅ **Live order book data ingestion** via WebSocket
- ✅ **Real-time trade flow processing**
- ✅ **VPIN calculation** from actual volume buckets
- ✅ **Phantom liquidity detection** through order lifetime analysis
- ✅ **Spoofing/layering pattern recognition**
- ✅ **Bid-ask spread tracking**
- ✅ **Order book depth analysis**

### Intended Use

This prototype demonstrates advanced liquidity health monitoring capabilities for Hyperliquid perps. It serves as:
- A proof-of-concept for microstructure analysis
- A framework for building production-grade monitoring tools
- An educational example of processing high-frequency market data

**DO NOT use this tool for:**
- Making trading decisions
- Risk management in production
- Financial reporting
- Regulatory compliance

### Data Accuracy

While the order flow and liquidity metrics are calculated from real market data, users should be aware:
- No data persistence between sessions
- Metrics reset on restart
- Some calculations use simplified models
- Network latency affects real-time accuracy

For production use, significant additional development would be required including proper error handling, data validation, state persistence, and comprehensive testing.

## 📄 License

MIT License - see LICENSE file for details.

