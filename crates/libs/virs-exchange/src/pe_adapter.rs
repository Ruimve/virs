use std::sync::Arc;

use async_trait::async_trait;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, warn};

use virs_models as models;
use virs_types::enums::*;
use virs_types::exchange_pe::{ExchangePe, OrderUpdateStream};
use virs_types::market::*;
use virs_types::position::*;
use virs_types::OrderResult;

use virs_error::{ExchangeError, VirsResult};

use crate::registry::Exchanges;
use crate::Exchange;

// PE适配器：实现 ExchangePe trait，从 registry 查找已注册的永续合约 Exchange
pub struct CcxtExchangeAdapter {
    registry: Arc<Exchanges>,
    cached_name: String,
}

impl CcxtExchangeAdapter {
    pub fn new(registry: Arc<Exchanges>) -> Self {
        Self {
            registry,
            cached_name: "binance".to_string(),
        }
    }

    // 从 registry 中查找名称包含 "perpetual" 的交易所
    fn get_perpetual_exchange(
        &self,
    ) -> Option<dashmap::mapref::one::Ref<'_, String, Box<dyn Exchange>>> {
        for entry in self.registry.registered_names() {
            if entry.contains("perpetual") {
                if let Some(ex) = self.registry.get(&entry) {
                    return Some(ex);
                }
            }
        }
        None
    }
}

// 以下为 models::Side 与 virs_types::Side 等类型互转的辅助函数
pub fn convert_side(side: &models::Side) -> Side {
    match side {
        models::Side::Buy => Side::Buy,
        models::Side::Sell => Side::Sell,
    }
}

pub fn convert_to_models_side(side: &Side) -> models::Side {
    match side {
        Side::Buy => models::Side::Buy,
        Side::Sell => models::Side::Sell,
    }
}

pub fn convert_position_side(side: &Option<PositionSide>) -> Option<models::PositionSide> {
    side.as_ref().map(|s| match s {
        PositionSide::Long => models::PositionSide::Long,
        PositionSide::Short => models::PositionSide::Short,
    })
}

// 订单类型互转
pub fn convert_order_type(ot: &OrderType) -> models::OrderType {
    match ot {
        OrderType::Limit => models::OrderType::Limit,
        OrderType::Market => models::OrderType::Market,
        OrderType::StopMarket => models::OrderType::StopMarket,
        OrderType::TakeProfitMarket => models::OrderType::TakeProfitMarket,
        _ => models::OrderType::Market, // Stop/TakeProfit/TrailingStopMarket/Liquidation 暂无对应
    }
}

pub fn convert_order_status(status: &models::OrderStatus) -> OrderStatus {
    match status {
        models::OrderStatus::Pending => OrderStatus::Pending,
        models::OrderStatus::Open => OrderStatus::Open,
        models::OrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
        models::OrderStatus::Filled => OrderStatus::Filled,
        models::OrderStatus::Canceled => OrderStatus::Canceled,
        models::OrderStatus::Failed => OrderStatus::Failed,
    }
}

// 持仓方向互转
pub fn convert_virs_position_side(side: &models::PositionSide) -> PositionSide {
    match side {
        models::PositionSide::Long => PositionSide::Long,
        models::PositionSide::Short => PositionSide::Short,
    }
}

// 市场类型互转
pub fn convert_virs_market_type(mt: &models::MarketType) -> MarketType {
    match mt {
        models::MarketType::Perpetual => MarketType::Perpetual,
    }
}

// 将 models::ExchangePosition 转换为 ExchangePosition
pub fn convert_exchange_position(ep: &models::ExchangePosition) -> ExchangePosition {
    ExchangePosition {
        symbol: ep.symbol.clone(),
        side: convert_virs_position_side(&ep.side),
        quantity: ep.quantity,
        entry_price: ep.entry_price,
    }
}

// 返回"无永续合约交易所注册"错误
pub fn no_exchange_error() -> ExchangeError {
    ExchangeError::Internal("No perpetual exchange registered in Exchanges".to_string())
}

// ExchangePe trait 实现：将上层调用委托给 registry 中的永续合约 Exchange
#[async_trait]
impl ExchangePe for CcxtExchangeAdapter {
    fn name(&self) -> &str {
        &self.cached_name
    }

    fn market_type(&self) -> MarketType {
        MarketType::Perpetual
    }

    // 获取行情 ticker
    async fn get_ticker(&self, symbol: &str) -> VirsResult<Ticker> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;
        let t = ex.get_ticker(symbol).await?;
        Ok(Ticker {
            symbol: t.symbol.clone(),
            exchange: t.exchange,
            bid: t.bid,
            ask: t.ask,
            last: t.last,
            high_24h: t.high_24h,
            low_24h: t.low_24h,
            volume_24h: t.volume_24h,
            price_change_24h: t.price_change_24h,
            price_change_pct_24h: t.price_change_pct_24h,
            timestamp: t.timestamp,
        })
    }

    // 获取账户余额 → GET /fapi/v3/balance，仅返回 USDT
    async fn get_balance(&self) -> VirsResult<Balance> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;
        let balances = ex.get_balances().await?;
        let usdt = balances
            .iter()
            .find(|b| b.asset.eq_ignore_ascii_case("USDT"))
            .ok_or_else(|| {
                ExchangeError::NoData(
                    "USDT balance not found in exchange balances — cannot return 0.0 as it would bypass risk checks".to_string(),
                )
            })?;
        Ok(Balance {
            asset: "USDT".to_string(),
            free: usdt.free,
            used: usdt.used,
            total: usdt.total,
        })
    }

    // 获取持仓 → GET /fapi/v2/positionRisk
    async fn get_positions(&self, symbol: Option<&str>) -> VirsResult<Vec<ExchangePosition>> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;
        let positions = ex.get_positions(symbol).await?;
        Ok(positions.iter().map(convert_exchange_position).collect())
    }

    // 获取资金费率
    async fn get_funding_rate(&self, symbol: &str) -> VirsResult<FundingRate> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;
        let fr = ex.get_funding_rate(symbol).await?;
        Ok(FundingRate {
            symbol: fr.symbol,
            rate: fr.rate,
            next_funding_time: fr.next_funding_time,
        })
    }

    // 下单 → Exchange.place_order_with_options() → POST /fapi/v1/order
    async fn place_order(&self, params: PlaceOrderParams) -> VirsResult<OrderResult> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;
        let virs_order = ex
            .place_order_with_options(
                &params.symbol,
                convert_to_models_side(&params.side),
                convert_order_type(&params.order_type),
                params.amount,
                params.price,
                convert_position_side(&params.position_side),
                params.client_order_id.as_deref(),
            )
            .await?;
        Ok(OrderResult {
            order_id: virs_order.id,
            client_order_id: virs_order.client_order_id.unwrap_or_default(),
        })
    }

    // 撤单 → Exchange.cancel_order() → DELETE /fapi/v1/order
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> VirsResult<OrderResult> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;
        let virs_order = ex.cancel_order(symbol, order_id).await?;
        Ok(OrderResult {
            order_id: virs_order.id,
            client_order_id: virs_order.client_order_id.unwrap_or_default(),
        })
    }

    // 全部撤单: DELETE /fapi/v1/allOpenOrders (签名)，symbol 必填
    async fn cancel_all_orders(&self, symbol: Option<&str>) -> VirsResult<Vec<OrderResult>> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;
        let sym = symbol.ok_or_else(|| {
            ExchangeError::InvalidRequest(
                "cancel_all_orders requires a symbol for DELETE /fapi/v1/allOpenOrders".into(),
            )
        })?;
        ex.cancel_all_orders(sym).await?;
        Ok(Vec::new())
    }

    // 设置杠杆 → POST /fapi/v1/leverage
    async fn set_leverage(&self, symbol: &str, leverage: u32) -> VirsResult<()> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;
        ex.set_leverage(symbol, leverage).await?;
        Ok(())
    }

    // 查询持仓模式 → GET /fapi/v1/positionSide/dual
    async fn get_position_mode(&self) -> VirsResult<PositionMode> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;

        ex.get_position_mode().await.map_err(Into::into)
    }

    // 订阅订单更新 → Exchange.start_listenkey_order_ws() → WS listenKey 流
    async fn subscribe_order_updates(&self, symbols: &[&str]) -> VirsResult<OrderUpdateStream> {
        let ex = self
            .get_perpetual_exchange()
            .ok_or_else(no_exchange_error)?;

        match ex.start_listenkey_order_ws(None).await {
            Ok(ws_rx) => {
                info!(
                    symbols_count = symbols.len(),
                    "Starting WebSocket order updates via listenKey"
                );
                Ok(Box::pin(ReceiverStream::new(ws_rx)))
            }
            Err(e) => {
                warn!(
                    error = %e,
                    market_type = ?ex.market_type(),
                    "Failed to start listenKey order WS, order updates disabled. \
                     For perpetual: check /fapi/v1/listenKey availability."
                );
                Err(no_exchange_error().into())
            }
        }
    }
}
