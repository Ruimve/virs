pub mod auto;
pub mod grid;
pub mod trading;

pub use auto::AutoBot;
pub use grid::GridBot;
pub use trading::Order;

#[cfg(test)]
mod auto_tests;
#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod serde_tests;
