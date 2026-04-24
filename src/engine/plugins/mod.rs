pub mod dual_ema_trend;
pub mod bb_squeeze;
pub mod rsi_mean_reversion;
pub mod atr_breakout;
pub mod scalper_vwap;
pub mod momentum_breakout;

pub use dual_ema_trend::DualEmaTrendPlugin;
pub use bb_squeeze::BbSqueezePlugin;
pub use rsi_mean_reversion::RsiMeanReversionPlugin;
pub use atr_breakout::AtrBreakoutPlugin;
pub use scalper_vwap::ScalperVwapPlugin;
pub use momentum_breakout::MomentumBreakoutPlugin;
