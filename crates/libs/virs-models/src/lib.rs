pub mod auto;
pub mod trading;

pub use auto::AutoBot;
pub use trading::Order;

#[cfg(test)]
mod auto_tests;
#[cfg(test)]
mod serde_tests;
