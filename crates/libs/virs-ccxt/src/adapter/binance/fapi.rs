use chrono::Utc;

use crate::auth::Signer;
use crate::types::*;
use crate::ExchangeClient;
use crate::{parse_f64, parse_str, parse_u32};
use virs_error::ExchangeError;

use super::parse_order_book_side;

// 币安U本位合约 API 基础域名
const BASE_URL: &str = "https://fapi.binance.com";

// 拼接完整请求 URL
fn url(path: &str) -> String {
    format!("{BASE_URL}{path}")
}


// 连通性检测
// GET /fapi/v1/ping - 测试与币安合约服务器的连通性，返回非空即视为成功
pub async fn ping(client: &ExchangeClient) -> Result<bool, ExchangeError> {
    let data = client.public_get(&url("/fapi/v1/ping"), &[]).await?;
    Ok(!data.is_null())
}


// 获取服务器时间，用于时间同步
// GET /fapi/v1/time - 返回 serverTime (毫秒)，用于校准本地时钟避免签名时间戳偏移
pub async fn fetch_server_time(client: &ExchangeClient) -> Result<i64, ExchangeError> {
    let data = client.public_get(&url("/fapi/v1/time"), &[]).await?;
    // 解析 serverTime 字段 (毫秒时间戳)
    data.get("serverTime")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| {
            ExchangeError::Internal("serverTime missing in /fapi/v1/time response".into())
        })
}


// 24小时行情统计 + 最优挂单
// GET /fapi/v1/ticker/24hr - 返回 symbol 的滚动24小时价格变动、成交量等统计
// GET /fapi/v1/ticker/bookTicker - 返回最优买一/卖一价 (bidPrice/askPrice)
pub async fn fetch_ticker(
    client: &ExchangeClient,
    symbol: &str,
) -> Result<CcxtTicker, ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let data = client
        .public_get(&url("/fapi/v1/ticker/24hr"), &[("symbol", native.as_str())])
        .await?;

    let last = parse_f64(&data, "lastPrice");
    if last.is_none() || last == Some(0.0) {
        return Err(ExchangeError::no_data(format!(
            "No ticker data available for {} on Binance Futures",
            symbol
        )));
    }

    // 请求 bookTicker 获取 bidPrice/askPrice
    let book = client
        .public_get(&url("/fapi/v1/ticker/bookTicker"), &[("symbol", native.as_str())])
        .await?;

    Ok(CcxtTicker {
        symbol: symbol.to_string(),
        exchange: "binance".into(),
        bid: parse_f64(&book, "bidPrice"),
        ask: parse_f64(&book, "askPrice"),
        last,
        high: parse_f64(&data, "highPrice"),
        low: parse_f64(&data, "lowPrice"),
        volume: parse_f64(&data, "volume"),
        quote_volume: parse_f64(&data, "quoteVolume"),
        open: parse_f64(&data, "openPrice"),
        close: parse_f64(&data, "lastPrice"),
        price_change: parse_f64(&data, "priceChange"),
        price_change_pct: parse_f64(&data, "priceChangePercent"),
        timestamp: Some(Utc::now()),
        info: data,
    })
}


// K线数据
// GET /fapi/v1/klines - 返回K线数组，每个元素为顺序数组:
// [openTime, open, high, low, close, volume, closeTime, quoteVolume, trades, buyBaseVolume, buyQuoteVolume, ignore]
pub async fn fetch_ohlcv(
    client: &ExchangeClient,
    symbol: &str,
    timeframe: &str,
    limit: u32,
    since: Option<i64>,
) -> Result<Vec<CcxtKline>, ExchangeError> {
    // 转换为币安原生交易对符号
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let mut params: Vec<(&str, String)> = vec![
        ("symbol", native),
        ("interval", timeframe.to_string()), // K线周期 (如 1m, 1h, 1d)
        ("limit", limit.to_string()),         // 返回K线数量上限
    ];
    // 可选起始时间 (毫秒)
    if let Some(s) = since {
        params.push(("startTime", s.to_string()));
    }

    let data = client
        .public_get(
            &url("/fapi/v1/klines"),
            &params
                .iter()
                .map(|(k, v)| (*k, v.as_str()))
                .collect::<Vec<_>>(),
        )
        .await?;

    // 响应应为数组
    let arr = data.as_array().ok_or_else(|| {
        ExchangeError::no_data(format!(
            "Invalid kline response for {} on Binance Futures",
            symbol
        ))
    })?;

    if arr.is_empty() {
        return Err(ExchangeError::no_data(format!(
            "No OHLCV data available for {} ({}) on Binance Futures",
            symbol, timeframe
        )));
    }

    // 逐根解析K线，按币安数组下标映射
    let klines: Vec<CcxtKline> = arr
        .iter()
        .filter_map(|k| {
            let a = match k.as_array() {
                Some(a) if a.len() >= 6 => a,
                _ => return None,
            };
            let timestamp = a[0].as_i64()?;                 // [0] openTime
            let close_time = a.get(6).and_then(|v| v.as_i64()); // [6] closeTime
            let open = a[1].as_str().and_then(|s| s.parse().ok())?;  // [1] open
            let high = a[2].as_str().and_then(|s| s.parse().ok())?;  // [2] high
            let low = a[3].as_str().and_then(|s| s.parse().ok())?;   // [3] low
            let close = a[4].as_str().and_then(|s| s.parse().ok())?; // [4] close
            let volume = a[5].as_str().and_then(|s| s.parse().ok())?; // [5] volume
            Some(CcxtKline {
                timestamp,
                close_time,
                open,
                high,
                low,
                close,
                volume,
                quote_volume: a
                    .get(7)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok()),           // [7] quoteVolume
                trades: a.get(8).and_then(|v| v.as_i64()),   // [8] 成交笔数
            })
        })
        .collect();

    if klines.is_empty() {
        return Err(ExchangeError::no_data(format!(
            "All kline entries invalid for {} ({}) on Binance Futures",
            symbol, timeframe
        )));
    }

    Ok(klines)
}


// 订单簿
// GET /fapi/v1/depth - 返回 symbol 的买卖盘深度
// 参数: symbol, limit (有效值: 5,10,20,50,100,500,1000)
pub async fn fetch_order_book(
    client: &ExchangeClient,
    symbol: &str,
    limit: u32,
) -> Result<CcxtOrderBook, ExchangeError> {

    // 校验 limit 取值在币安允许的范围内
    const VALID_FUTURES_DEPTH_LIMITS: &[u32] = &[5, 10, 20, 50, 100, 500, 1000];
    if !VALID_FUTURES_DEPTH_LIMITS.contains(&limit) {
        return Err(ExchangeError::InvalidRequest(format!(
            "Invalid depth limit {} for futures /fapi/v1/depth — valid values: {:?}",
            limit, VALID_FUTURES_DEPTH_LIMITS
        )));
    }

    // 转换为币安原生交易对符号
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let data = client
        .public_get(
            &url("/fapi/v1/depth"),
            &[("symbol", native.as_str()), ("limit", &limit.to_string())],
        )
        .await?;

    // 解析 bids/asks 数组，每个元素为 [价格, 数量]
    let bids = parse_order_book_side(&data, "bids");
    let asks = parse_order_book_side(&data, "asks");

    if bids.is_empty() && asks.is_empty() {
        return Err(ExchangeError::no_data(format!(
            "No order book data for {} on Binance Futures",
            symbol
        )));
    }

    Ok(CcxtOrderBook {
        symbol: symbol.to_string(),
        bids,
        asks,
        timestamp: Some(Utc::now()),
        nonce: None,
    })
}


// 交易对信息
// GET /fapi/v1/exchangeInfo - 返回所有合约交易对的规格、精度、过滤器等
pub async fn fetch_markets(client: &ExchangeClient) -> Result<Vec<MarketInfo>, ExchangeError> {
    let data = client
        .public_get(&url("/fapi/v1/exchangeInfo"), &[])
        .await?;

    // 解析 symbols 数组
    let symbols = data
        .get("symbols")
        .and_then(|s| s.as_array())
        .ok_or_else(|| ExchangeError::Internal("Invalid exchangeInfo response".into()))?;

    let markets: Vec<MarketInfo> = symbols
        .iter()
        .filter_map(|s| {
            // 仅保留交易中 (TRADING) 的交易对
            let status = parse_str(s, "status")?;
            if status != "TRADING" {
                return None;
            }

            // 仅保留永续合约 (PERPETUAL)
            let contract_type = parse_str(s, "contractType")?;
            if contract_type != "PERPETUAL" {
                return None;
            }

            let base = parse_str(s, "baseAsset")?;   // 基础资产 (如 BTC)
            let quote = parse_str(s, "quoteAsset")?; // 计价资产 (如 USDT)
            let symbol = format!("{}/{}", base, quote);

            // 解析 filters 数组，提取各过滤器的限额
            let filters = s.get("filters").and_then(|f| f.as_array());
            // LOT_SIZE 过滤器: 数量上下限
            let (min_amount, max_amount) = filters
                .map(|arr| {
                    let lot = arr
                        .iter()
                        .find(|f| f.get("filterType").and_then(|v| v.as_str()) == Some("LOT_SIZE"));
                    (
                        lot.and_then(|f| parse_f64(f, "minQty")),
                        lot.and_then(|f| parse_f64(f, "maxQty")),
                    )
                })
                .unwrap_or((None, None));
            // PRICE_FILTER 过滤器: 价格上下限
            let (min_price, max_price) = filters
                .map(|arr| {
                    let pf = arr.iter().find(|f| {
                        f.get("filterType").and_then(|v| v.as_str()) == Some("PRICE_FILTER")
                    });
                    (
                        pf.and_then(|f| parse_f64(f, "minPrice")),
                        pf.and_then(|f| parse_f64(f, "maxPrice")),
                    )
                })
                .unwrap_or((None, None));
            // MIN_NOTIONAL 过滤器: 最小名义价值
            let min_cost = filters
                .and_then(|arr| {
                    arr.iter().find(|f| {
                        f.get("filterType")
                            .and_then(|v| v.as_str())
                            .map(|t| t == "MIN_NOTIONAL")
                            .unwrap_or(false)
                    })
                })
                .and_then(|f| parse_f64(f, "notional"));

            Some(MarketInfo {
                id: parse_str(s, "symbol")?,           // 币安原生符号 (如 BTCUSDT)
                symbol,
                base,
                quote,
                active: true,
                market_type: MarketType::Perpetual,
                min_amount,
                max_amount,
                min_price,
                max_price,
                min_cost,
                price_precision: parse_u32(s, "pricePrecision"),     // 价格精度小数位
                amount_precision: parse_u32(s, "quantityPrecision"), // 数量精度小数位
                info: s.clone(),
            })
        })
        .collect();

    Ok(markets)
}


// 标记价格和资金费率
// GET /fapi/v1/premiumIndex - 返回 symbol 的标记价、资金费率及下次结算时间
// 响应字段: symbol, markPrice, indexPrice, estimatedSettlePrice, lastFundingRate,
//           interestRate, nextFundingTime, time
pub async fn fetch_funding_rate(
    client: &ExchangeClient,
    symbol: &str,
) -> Result<CcxtFundingRate, ExchangeError> {
    // 转换为币安原生交易对符号
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let data = client
        .public_get(
            &url("/fapi/v1/premiumIndex"),
            &[("symbol", native.as_str())],
        )
        .await?;

    // 解析资金费率，缺失则报错 (避免误用 0.0)
    let rate = parse_f64(&data, "lastFundingRate").ok_or_else(|| {
        tracing::warn!(symbol = %symbol, "lastFundingRate missing — returning NoData instead of 0.0");
        ExchangeError::no_data(format!("lastFundingRate missing for {symbol}"))
    })?;
    // 解析下次资金结算时间 (毫秒)，过滤无效时间戳
    let next_funding_time = data
        .get("nextFundingTime")
        .and_then(|t| t.as_i64())

        .filter(|&ts| ts > 0)
        .and_then(|ts| {
            chrono::DateTime::from_timestamp_millis(ts).or_else(|| {
                tracing::warn!(ts, %symbol, "nextFundingTime timestamp invalid — returning None");
                None
            })
        });

    Ok(CcxtFundingRate {
        symbol: symbol.to_string(),
        rate,
        next_funding_time,
        info: data,
    })
}


// 资金费率历史
// GET /fapi/v1/fundingRate - 返回 symbol 的历史资金费率记录
// 支持分页拉取 (每页上限1000条)，按 startTime/endTime 区间循环抓取
pub async fn fetch_funding_history(
    client: &ExchangeClient,
    symbol: &str,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<CcxtFundingHistoryEntry>, ExchangeError> {
    // 转换为币安原生交易对符号
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let mut all_entries: Vec<CcxtFundingHistoryEntry> = Vec::new();
    let mut current_start = start_time;

    // 分页循环: 每次最多拉取1000条
    loop {
        let data = client
            .public_get(
                &url("/fapi/v1/fundingRate"),
                &[
                    ("symbol", native.as_str()),
                    ("startTime", &current_start.to_string()),
                    ("endTime", &end_time.to_string()),
                    ("limit", "1000"), // 单页最大条数
                ],
            )
            .await?;

        let arr = data.as_array().ok_or_else(|| {
            ExchangeError::Internal("Invalid fundingRate history response".into())
        })?;

        if arr.is_empty() {
            break;
        }

        // 逐条解析资金费率记录
        for item in arr {
            // 资金结算时间 (毫秒)
            let funding_time = item
                .get("fundingTime")
                .and_then(|t| t.as_i64())
                .and_then(chrono::DateTime::from_timestamp_millis)
                .ok_or_else(|| {
                    tracing::warn!("fundingTime missing or invalid — returning NoData instead of 0");
                    ExchangeError::no_data("fundingTime missing or invalid in funding history".into())
                })?;
            // 资金费率
            let rate = parse_f64(item, "fundingRate").ok_or_else(|| {
                tracing::warn!("fundingRate missing — returning NoData instead of 0.0");
                ExchangeError::no_data("fundingRate missing in funding history".into())
            })?;
            all_entries.push(CcxtFundingHistoryEntry { funding_time, rate });
        }

        // 不足1000条说明已是最后一页
        if arr.len() < 1000 {
            break;
        }

        // 以最后一条的 fundingTime 推进游标，避免重复
        if let Some(last) = arr.last() {
            current_start = last
                .get("fundingTime")
                .and_then(|t| t.as_i64())
                .unwrap_or(end_time)
                + 1;
        } else {
            break;
        }
    }

    Ok(all_entries)
}


// 合约账户余额 (V3)
// GET /fapi/v3/balance - 签名请求，返回合约账户各资产余额
// 响应字段: accountAlias, asset, balance, crossWalletBalance, crossUnPnl,
//           availableBalance, maxWithdrawAmount, marginAvailable, updateTime
pub async fn fetch_balance(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<Vec<Balance>, ExchangeError> {
    let data = client
        .signed_get(signer, &url("/fapi/v3/balance"), vec![])
        .await?;

    // 响应为余额数组
    let balances = data.as_array().ok_or_else(|| {
        ExchangeError::Internal("Invalid futures balance response from Binance".into())
    })?;

    let result: Vec<Balance> = balances
        .iter()
        .filter_map(|b| {
            // 资产名称 (如 USDT)
            let asset = parse_str(b, "asset").unwrap_or_else(|| {
                tracing::warn!("Balance asset field missing — skipping entry");
                String::new()
            });
            if asset.is_empty() {
                return None;
            }
            // 可用余额，缺失则跳过 (避免误用 0.0)
            let free = parse_f64(b, "availableBalance").unwrap_or_else(|| {
                tracing::warn!(asset = %asset, "Balance 'availableBalance' field missing or unparseable — skipping entry to avoid 0.0 propagation");
                f64::NAN
            });
            if free.is_nan() {
                return None;
            }
            // 总余额，缺失则跳过
            let total = parse_f64(b, "balance").unwrap_or_else(|| {
                tracing::warn!(asset = %asset, "Balance 'balance' field missing or unparseable — skipping entry to avoid 0.0 propagation");
                f64::NAN
            });
            if total.is_nan() {
                return None;
            }
            // 已用 = 总额 - 可用
            let used = total - free;
            // 跳过全零资产
            if free == 0.0 && used == 0.0 {
                return None;
            }
            Some(Balance {
                asset,
                free,
                used,
                total,
            })
        })
        .collect();

    Ok(result)
}


// 下单 (签名)
// POST /fapi/v1/order - 提交合约订单
// 请求参数: symbol, side, type, quantity, price, timeInForce(GTC/IOC/FOK/GTX),
//           reduceOnly, positionSide(LONG/SHORT/BOTH), newClientOrderId, stopPrice,
//           closePosition, workingType, priceProtect, newOrderRespType, goodTillDate
// 响应字段: orderId, clientOrderId, cumQty, cumQuote, executedQty, avgPrice, origQty,
//           price, reduceOnly, side, positionSide, status, stopPrice, closePosition,
//           symbol, timeInForce, origType, type, updateTime, workingType, priceProtect,
//           priceMatch, selfTradePreventionMode, goodTillDate
// 注意: positionSide 在双向持仓模式下必须指定 LONG 或 SHORT
pub async fn create_order(
    client: &ExchangeClient,
    signer: &dyn Signer,
    params: PlaceOrderParams,
) -> Result<OrderResult, ExchangeError> {
    // 转换为币安原生交易对符号
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(&params.symbol);
    // 构建请求体基础字段: 交易对、方向、类型、数量
    let mut body = serde_json::json!({
        "symbol": native,
        "side": crate::adapter::binance::BinanceExchange::side_str(&params.side),
        "type": crate::adapter::binance::BinanceExchange::order_type_str(&params.order_type),
        "quantity": params.amount,
    });

    // 限价单需要价格与 timeInForce (默认 GTC)
    if let Some(price) = params.price {
        body["price"] = serde_json::json!(price);
        body["timeInForce"] = serde_json::json!(params
            .time_in_force
            .as_ref()
            .map(|tif| match tif {
                TimeInForce::Gtc => "GTC",
                TimeInForce::Ioc => "IOC",
                TimeInForce::Fok => "FOK",
                TimeInForce::Poc => "GTX", // POC 对币安 GTX (只做 Maker)
            })
            .unwrap_or("GTC"));
    }

    // 止损/止盈触发价
    if let Some(stop_price) = params.stop_price {
        body["stopPrice"] = serde_json::json!(stop_price);
    }

    // 自定义客户端订单ID
    if let Some(ref client_id) = params.client_order_id {
        body["newClientOrderId"] = serde_json::json!(client_id);
    }


    // 双向持仓模式下根据买卖方向和持仓方向推导 positionSide
    // 注意: 单向模式 (BOTH) 不被支持，本实现要求账户开启双向持仓
    let position_side = match (&params.side, &params.position_side) {
        (Side::Buy, Some(PositionSide::Long)) => "LONG",
        (Side::Sell, Some(PositionSide::Short)) => "SHORT",
        (Side::Buy, Some(PositionSide::Short)) => "SHORT",
        (Side::Sell, Some(PositionSide::Long)) => "LONG",
        (_, None) => {
            return Err(ExchangeError::InvalidRequest(
                "position_side is required in Hedge mode — OneWay (BOTH) is not supported. \
                 Switch the Binance futures account to Hedge mode and provide position_side."
                    .into(),
            ));
        }
    };
    body["positionSide"] = serde_json::json!(position_side);


    // 仅减仓标记，仅对单向模式有效，双向模式下由 positionSide 决定
    if let Some(reduce) = params.reduce_only {
        if reduce {
            body["reduceOnly"] = serde_json::json!(true);
        }
    }

    let data = client
        .signed_post(signer, &url("/fapi/v1/order"), body)
        .await?;

    // 只提取 orderId + clientOrderId，完整订单数据由 WS ORDER_TRADE_UPDATE 推送
    let order_id = parse_str(&data, "orderId")
        .ok_or_else(|| ExchangeError::no_data("orderId missing in create_order response".into()))?;
    let client_order_id = parse_str(&data, "clientOrderId")
        .unwrap_or_default();

    Ok(OrderResult {
        order_id,
        client_order_id,
    })
}


// 撤单 (签名)
// DELETE /fapi/v1/order - 撤销指定订单
// 参数: symbol, orderId (或 origClientOrderId)
// 注意: 响应无 time 字段，只有 updateTime
pub async fn cancel_order(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
    order_id: &str,
) -> Result<OrderResult, ExchangeError> {
    // 转换为币安原生交易对符号
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let params = vec![
        ("symbol".into(), native),
        ("orderId".into(), order_id.to_string()),
    ];

    let data = client
        .signed_delete(signer, &url("/fapi/v1/order"), params)
        .await?;

    // 只提取 orderId + clientOrderId，完整订单数据由 WS ORDER_TRADE_UPDATE 推送
    let order_id = parse_str(&data, "orderId")
        .ok_or_else(|| ExchangeError::no_data("orderId missing in cancel_order response".into()))?;
    let client_order_id = parse_str(&data, "clientOrderId")
        .unwrap_or_default();

    Ok(OrderResult {
        order_id,
        client_order_id,
    })
}


// 批量撤单 (签名)
// DELETE /fapi/v1/allOpenOrders - 撤销指定交易对全部挂单
// 参数: symbol (必填)
// 响应: {"code": 200, "msg": "The operation of cancel all open order is done."}
pub async fn cancel_all_orders(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
) -> Result<(), ExchangeError> {
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let params = vec![("symbol".into(), native)];

    let _data = client
        .signed_delete(signer, &url("/fapi/v1/allOpenOrders"), params)
        .await?;

    Ok(())
}


// 变换逐全仓模式 (签名)
// POST /fapi/v1/marginType - 切换指定交易对的保证金模式
// 参数: symbol, marginType (ISOLATED 逐仓 / CROSSED 全仓)
// 注意: 忽略错误，因为重复设置会返回 -4046 (无需变更)
pub async fn set_margin_type(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
    margin_mode: MarginMode,
) -> Result<(), ExchangeError> {
    // 转换为币安原生交易对符号
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    // 映射保证金模式为币安字符串
    let margin_type_str = match margin_mode {
        MarginMode::Cross => "CROSSED",   // 全仓
        MarginMode::Isolated => "ISOLATED", // 逐仓
    };
    let body = serde_json::json!({
        "symbol": native,
        "marginType": margin_type_str,
    });

    // 忽略结果: 重复设置同一模式会报 -4046，属正常情况
    let _ = client
        .signed_post(signer, &url("/fapi/v1/marginType"), body)
        .await;
    Ok(())
}


// 调整杠杆 (签名)
// POST /fapi/v1/leverage - 调整指定交易对的杠杆倍数
// 参数: symbol, leverage (1-125)
pub async fn set_leverage(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: &str,
    leverage: u32,
) -> Result<(), ExchangeError> {
    // 转换为币安原生交易对符号
    let native = crate::adapter::binance::BinanceExchange::to_native_symbol(symbol);
    let body = serde_json::json!({
        "symbol": native,
        "leverage": leverage,
    });
    client
        .signed_post(signer, &url("/fapi/v1/leverage"), body)
        .await?;
    Ok(())
}


// 持仓信息 (V2)
// GET /fapi/v2/positionRisk - 签名请求，返回账户持仓信息
// 响应字段: entryPrice, breakEvenPrice, marginType, isAutoAddMargin, isolatedMargin,
//           leverage, liquidationPrice, markPrice, maxNotionalValue, positionAmt,
//           notional, isolatedWallet, symbol, unRealizedProfit, positionSide, updateTime
pub async fn fetch_positions(
    client: &ExchangeClient,
    signer: &dyn Signer,
    symbol: Option<&str>,
) -> Result<Vec<Position>, ExchangeError> {
    // symbol 可选，传则查指定交易对，不传查全部
    let mut params: Vec<(String, String)> = vec![];
    if let Some(sym) = symbol {
        params.push((
            "symbol".into(),
            crate::adapter::binance::BinanceExchange::to_native_symbol(sym),
        ));
    }

    let data = client
        .signed_get(signer, &url("/fapi/v2/positionRisk"), params)
        .await?;

    // 响应为持仓数组
    let arr = data.as_array().ok_or_else(|| {
        ExchangeError::Internal("Invalid positionRisk response from Binance".into())
    })?;

    let mut positions: Vec<Position> = Vec::new();
    for p in arr.iter() {
            // 持仓数量: 正数为多，负数为空，0 表示无持仓
            let pos_amt = parse_f64(p, "positionAmt").unwrap_or_else(|| {
                tracing::warn!("positionAmt missing — skipping entry to avoid silent position drop");
                f64::NAN
            });
            // 跳过空仓
            if pos_amt.is_nan() || pos_amt == 0.0 {
                continue;
            }

            // 根据数量正负判定多空方向
            let side = if pos_amt > 0.0 {
                PositionSide::Long
            } else {
                PositionSide::Short
            };
            let size = pos_amt.abs();

            let symbol_str = match parse_str(p, "symbol") {
                Some(s) => s,
                None => {
                    tracing::warn!("positionRisk symbol missing — skipping position");
                    continue;
                }
            };

            // 保证金模式: isolated 逐仓 / crossed 全仓
            let margin_type_str = parse_str(p, "marginType").unwrap_or_else(|| {
                tracing::warn!(symbol = %symbol_str, "positionRisk marginType missing — defaulting to Cross");
                String::new()
            });
            let margin_mode = match margin_type_str.as_str() {
                "isolated" => MarginMode::Isolated,
                _ => MarginMode::Cross,
            };


            // 开仓均价，缺失则跳过
            let entry_price = match parse_f64(p, "entryPrice") {
                Some(v) => v,
                None => {
                    tracing::warn!(
                        symbol = %symbol_str,
                        "positionRisk entryPrice missing — skipping position"
                    );
                    continue;
                }
            };
            // 杠杆倍数，缺失则跳过
            let leverage = match parse_u32(p, "leverage") {
                Some(v) => v,
                None => {
                    tracing::warn!(
                        symbol = %symbol_str,
                        "positionRisk leverage missing — skipping position"
                    );
                    continue;
                }
            };


            // 未实现盈亏，缺失则报错 (无法计算 PnL)
            let unrealized_pnl = match parse_f64(p, "unRealizedProfit") {
                Some(v) => v,
                None => {
                    tracing::error!(
                        symbol = %symbol_str,
                        "positionRisk unRealizedProfit missing — cannot calculate PnL"
                    );
                    return Err(ExchangeError::no_data(format!(
                        "unRealizedProfit missing for symbol {} in positionRisk response",
                        symbol_str
                    )));
                }
            };

            positions.push(Position {
                // 转回统一格式符号 (如 BTCUSDT -> BTC/USDT)
                symbol: crate::adapter::binance::BinanceExchange::to_unified_symbol(&symbol_str),
                side,
                size,
                entry_price,
                leverage,
                unrealized_pnl,
                margin_mode,
                liquidation_price: parse_f64(p, "liquidationPrice"), // 强平价
                info: p.clone(),
            });
    }

    Ok(positions)
}


// 查询持仓模式 (签名)
// GET /fapi/v1/positionSide/dual - 查询账户是否为双向持仓模式
// 响应: { "dualSidePosition": true } - true=双向持仓, false=单向持仓
// 注意: 本实现要求双向持仓 (Hedge) 模式，单向模式会返回错误
pub async fn get_position_mode(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<PositionMode, ExchangeError> {
    let data = client
        .signed_get(signer, &url("/fapi/v1/positionSide/dual"), vec![])
        .await?;

    // 解析 dualSidePosition 布尔值，默认 false (单向)
    let dual_side = data
        .get("dualSidePosition")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if dual_side {
        Ok(PositionMode::Hedge)
    } else {
        // 单向模式不支持，要求用户切换为双向持仓
        Err(ExchangeError::InvalidRequest(
            "Exchange account is in OneWay (single-position) mode. \
             VIRS requires Hedge mode. Switch to Hedge mode in Binance futures \
             settings (API key > Position Mode > Hedge Mode)."
                .into(),
        ))
    }
}


// 创建 listenKey (签名)
// POST /fapi/v1/listenKey - 创建用于 WebSocket 用户数据流的 listenKey
// 响应: { "listenKey": "...", "createdAt": ... }
pub async fn create_listen_key(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<String, ExchangeError> {
    let body = serde_json::json!({});
    let data = client
        .signed_post(signer, &url("/fapi/v1/listenKey"), body)
        .await?;
    // 解析 listenKey 字符串，缺失则报错
    data.get("listenKey")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| ExchangeError::Internal("listenKey missing in response".into()))
}


// 续期 listenKey (签名)
// PUT /fapi/v1/listenKey - 续期 listenKey，防止 WebSocket 连接断开
// 参数: listenKey (需在请求体中携带)
pub async fn keepalive_listen_key(
    client: &ExchangeClient,
    signer: &dyn Signer,
    listen_key: &str,
) -> Result<(), ExchangeError> {
    // 请求体携带 listenKey
    let body = serde_json::json!({ "listenKey": listen_key });
    client
        .signed_put(signer, &url("/fapi/v1/listenKey"), body)
        .await?;
    Ok(())
}
