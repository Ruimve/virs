use std::pin::Pin;

use futures_core::Stream;

use crate::position::WsFeedEvent;


pub type OrderUpdateStream = Pin<Box<dyn Stream<Item = WsFeedEvent> + Send>>;
