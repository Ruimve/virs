pub mod adapter;
pub mod paper;
pub mod registry;

pub use adapter::CcxtAdapter;
pub use paper::PaperExchangeAdapter;
pub use registry::Exchanges;

#[cfg(test)]
mod adapter_tests;
