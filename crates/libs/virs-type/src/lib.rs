pub mod auto;
pub mod bot;
pub mod exchange;
pub mod llm;
pub mod market;
pub mod order;
pub mod position;

// 顶层 re-export：保持 `virs_type::CcxtOrder` 等扁平路径可用
pub use auto::*;
pub use bot::*;
pub use exchange::*;
pub use llm::*;
pub use market::*;
pub use order::*;
pub use position::*;
