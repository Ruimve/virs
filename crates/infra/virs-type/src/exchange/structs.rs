use std::pin::Pin;

use futures_core::Stream;

use crate::position::WsFeedEvent;


/* 订单更新流类型别名：使用 Pin<Box<dyn Stream>> 实现异步流式订单事件传递 */
pub type OrderUpdateStream = Pin<Box<dyn Stream<Item = WsFeedEvent> + Send>>;
