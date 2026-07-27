pub mod llm_client;
pub mod ports;

// ai_client 和 indicators 已移至 virs-strategy crate。
// 通过 re-export 保持向后兼容（virs_bot::common::ai_client::* 等路径不变）。
pub use virs_strategy::llm_client as ai_client;
pub use virs_strategy::market as indicators;
