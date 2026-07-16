pub mod auto;
pub mod grid;
pub mod trading;


pub use auto::AutoBot;
pub use grid::GridBot;
pub use trading::Order;
pub use virs_types::enums::*;
pub use virs_types::market::*;


#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod auto_tests;
#[cfg(test)]
mod serde_tests;
