use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Default-value helpers (used as `#[serde(default = "...")]` path expressions)
// ---------------------------------------------------------------------------

fn default_sync_interval() -> u64 {
    10
}

fn default_poll_interval() -> u64 {
    10
}

fn default_ws_reconnect_timeout() -> u64 {
    30
}

fn default_pnl_snapshot_interval() -> u64 {
    60
}

fn default_max_position_per_symbol() -> f64 {
    1.0
}

fn default_max_total_position() -> f64 {
    3.0
}

fn default_max_order_amount() -> f64 {
    0.3
}

fn default_max_drawdown() -> f64 {
    0.15
}

fn default_max_leverage() -> u32 {
    20
}

fn default_funding_rate_threshold() -> f64 {
    0.001
}

fn default_liquidation_buffer() -> f64 {
    0.2
}

fn default_max_consecutive_losses() -> u32 {
    5
}

// ---------------------------------------------------------------------------
// RiskConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    #[serde(default = "default_max_position_per_symbol")]
    pub max_position_per_symbol_pct: f64,

    #[serde(default = "default_max_total_position")]
    pub max_total_position_pct: f64,

    #[serde(default = "default_max_order_amount")]
    pub max_order_amount_pct: f64,

    #[serde(default = "default_max_drawdown")]
    pub max_drawdown_pct: f64,

    #[serde(default = "default_max_leverage")]
    pub max_leverage: u32,

    #[serde(default = "default_funding_rate_threshold")]
    pub funding_rate_threshold: f64,

    #[serde(default = "default_liquidation_buffer")]
    pub liquidation_buffer_pct: f64,

    #[serde(default = "default_max_consecutive_losses")]
    pub max_consecutive_losses: u32,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_per_symbol_pct: default_max_position_per_symbol(),
            max_total_position_pct: default_max_total_position(),
            max_order_amount_pct: default_max_order_amount(),
            max_drawdown_pct: default_max_drawdown(),
            max_leverage: default_max_leverage(),
            funding_rate_threshold: default_funding_rate_threshold(),
            liquidation_buffer_pct: default_liquidation_buffer(),
            max_consecutive_losses: default_max_consecutive_losses(),
        }
    }
}

// ---------------------------------------------------------------------------
// EngineConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub engine_id: String,

    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,

    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,

    #[serde(default = "default_ws_reconnect_timeout")]
    pub ws_reconnect_timeout_secs: u64,

    #[serde(default)]
    pub risk: RiskConfig,

    #[serde(default = "default_pnl_snapshot_interval")]
    pub pnl_snapshot_interval_secs: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            engine_id: "default".to_string(),
            sync_interval_secs: default_sync_interval(),
            poll_interval_secs: default_poll_interval(),
            ws_reconnect_timeout_secs: default_ws_reconnect_timeout(),
            risk: RiskConfig::default(),
            pnl_snapshot_interval_secs: default_pnl_snapshot_interval(),
        }
    }
}
