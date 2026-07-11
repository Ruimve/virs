//! ACCOUNT_UPDATE — Balance 和 Position 更新推送
//!
//! 官方描述: 当账户信息有变动时(包括资金、仓位、保证金模式等发生变化)，才会推送此事件。
//! 注意: 订单状态变化没有引起账户和持仓变化的，不会推送此事件。
//!
//! 官方文档: https://developers.binance.com/zh-CN/docs/products/derivatives-trading-usds-futures/user-data-streams

use serde::Deserialize;
use virs_types::WsFeedEvent;

/// ACCOUNT_UPDATE 事件
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountUpdateEvent {
    /// 事件类型
    #[serde(rename = "e")]
    pub event_type: String,
    /// 事件时间
    #[serde(rename = "E")]
    pub event_time: i64,
    /// 撮合时间
    #[serde(rename = "T")]
    pub transaction_time: i64,
    /// 更新数据
    #[serde(rename = "a")]
    pub update_data: AccountUpdateData,
}

/// 更新数据 (a 字段)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountUpdateData {
    /// 事件原因类型 Event reason type
    #[serde(rename = "m")]
    pub reason_type: String,
    /// 余额列表 Balances
    #[serde(rename = "B")]
    pub balances: Vec<AccountBalance>,
    /// 持仓列表 Positions（可能为空数组 — 如 FUNDING_FEE 全仓场景）
    #[serde(rename = "P")]
    pub positions: Vec<AccountPosition>,
}

/// 余额信息 (B 数组元素)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountBalance {
    /// 资产 Asset
    #[serde(rename = "a")]
    pub asset: String,
    /// 钱包余额 Wallet Balance
    #[serde(rename = "wb")]
    pub wallet_balance: String,
    /// 全仓钱包余额 Cross Wallet Balance
    #[serde(rename = "cw")]
    pub cross_wallet_balance: String,
    /// 余额变化量（不含盈亏和手续费）Balance Change except PnL and Commission
    #[serde(rename = "bc")]
    pub balance_change: String,
}

/// 持仓信息 (P 数组元素)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountPosition {
    /// 订单符号 Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// 持仓数量 Position Amount
    #[serde(rename = "pa")]
    pub position_amount: String,
    /// 开仓均价 Entry Price
    #[serde(rename = "ep")]
    pub entry_price: String,
    /// 盈亏平衡价 Breakeven Price
    #[serde(rename = "bep")]
    pub breakeven_price: String,
    /// (税前)累计已实现盈亏 (Pre-fee) Accumulated Realized
    #[serde(rename = "cr")]
    pub accumulated_realized_pnl: String,
    /// 未实现盈亏 Unrealized PnL
    #[serde(rename = "up")]
    pub unrealized_pnl: String,
    /// 保证金类型 Margin Type: isolated / crossed
    #[serde(rename = "mt")]
    pub margin_type: String,
    /// 逐仓钱包余额 Isolated Wallet (if isolated position)
    #[serde(rename = "iw")]
    pub isolated_wallet: String,
    /// 持仓方向 Position Side: LONG / SHORT / BOTH
    #[serde(rename = "ps")]
    pub position_side: String,
}

/// 事件原因类型枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountUpdateReason {
    Deposit,
    Withdraw,
    Order,
    FundingFee,
    WithdrawReject,
    Adjustment,
    InsuranceClear,
    AdminDeposit,
    AdminWithdraw,
    MarginTransfer,
    MarginTypeChange,
    AssetTransfer,
    OptionsPremiumFee,
    OptionsSettleProfit,
    AutoExchange,
    CoinSwapDeposit,
    CoinSwapWithdraw,
    Unknown(String),
}

impl AccountUpdateReason {
    pub fn from_str(s: &str) -> Self {
        match s {
            "DEPOSIT" => Self::Deposit,
            "WITHDRAW" => Self::Withdraw,
            "ORDER" => Self::Order,
            "FUNDING_FEE" => Self::FundingFee,
            "WITHDRAW_REJECT" => Self::WithdrawReject,
            "ADJUSTMENT" => Self::Adjustment,
            "INSURANCE_CLEAR" => Self::InsuranceClear,
            "ADMIN_DEPOSIT" => Self::AdminDeposit,
            "ADMIN_WITHDRAW" => Self::AdminWithdraw,
            "MARGIN_TRANSFER" => Self::MarginTransfer,
            "MARGIN_TYPE_CHANGE" => Self::MarginTypeChange,
            "ASSET_TRANSFER" => Self::AssetTransfer,
            "OPTIONS_PREMIUM_FEE" => Self::OptionsPremiumFee,
            "OPTIONS_SETTLE_PROFIT" => Self::OptionsSettleProfit,
            "AUTO_EXCHANGE" => Self::AutoExchange,
            "COIN_SWAP_DEPOSIT" => Self::CoinSwapDeposit,
            "COIN_SWAP_WITHDRAW" => Self::CoinSwapWithdraw,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// 处理 ACCOUNT_UPDATE 原始 JSON
///
/// 当前阶段: 解析并记录日志，返回 None（待 WsFeedEvent 扩展后传递到下游）。
/// 后续阶段: 返回 WsFeedEvent::AccountUpdate { reason, balances, positions, timestamp }
pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: AccountUpdateEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "[ACCOUNT_UPDATE] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    let reason = AccountUpdateReason::from_str(&event.update_data.reason_type);

    // 资金费率结算 — 特殊日志
    if reason == AccountUpdateReason::FundingFee {
        for b in &event.update_data.balances {
            if let Ok(change) = b.balance_change.parse::<f64>() {
                if change != 0.0 {
                    tracing::info!(
                        asset = %b.asset,
                        change = change,
                        wallet_balance = %b.wallet_balance,
                        "资金费率结算 — 余额变更"
                    );
                }
            }
        }
    }

    // 仓位变更 — 记录日志
    for p in &event.update_data.positions {
        tracing::debug!(
            symbol = %p.symbol,
            position_side = %p.position_side,
            position_amount = %p.position_amount,
            entry_price = %p.entry_price,
            unrealized_pnl = %p.unrealized_pnl,
            margin_type = %p.margin_type,
            reason = ?reason,
            "ACCOUNT_UPDATE 持仓变更"
        );
    }

    // 待 WsFeedEvent 扩展后，返回 WsFeedEvent::AccountUpdate
    None
}
