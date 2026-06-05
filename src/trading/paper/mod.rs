//! Paper 交易适配器
//!
//! 通过 PaperExchangeAdapter 实现 Position Engine 的 Exchange trait，
//! 用于纸面交易（模拟撮合）。Market 单立即成交，Limit 单挂单等待价格触发。

pub mod adapter;
