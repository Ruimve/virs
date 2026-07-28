pub mod adapter;
pub mod paper;
pub mod registry;

pub use adapter::CcxtAdapter;
pub use paper::PaperExchangeAdapter;
pub use registry::Exchanges;

// 重导出 ExchangePe trait，方便调用方直接通过 virs_exchange::ExchangePe 使用
pub use virs_types::ExchangePe;

#[cfg(test)]
mod adapter_tests;
