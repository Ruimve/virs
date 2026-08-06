use serde::Deserialize;
use virs_type::WsFeedEvent;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountUpdateEvent {
    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "T")]
    pub transaction_time: i64,

    #[serde(rename = "a")]
    pub update_data: AccountUpdateData,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountUpdateData {
    #[serde(rename = "m")]
    pub reason_type: String,

    #[serde(rename = "B")]
    pub balances: Vec<AccountBalance>,

    #[serde(rename = "P")]
    pub positions: Vec<AccountPosition>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountBalance {
    #[serde(rename = "a")]
    pub asset: String,

    #[serde(rename = "wb")]
    pub wallet_balance: String,

    #[serde(rename = "cw")]
    pub cross_wallet_balance: String,

    #[serde(rename = "bc")]
    pub balance_change: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccountPosition {
    #[serde(rename = "s")]
    pub symbol: String,

    #[serde(rename = "pa")]
    pub position_amount: String,

    #[serde(rename = "ep")]
    pub entry_price: String,

    #[serde(rename = "bep")]
    pub breakeven_price: String,

    #[serde(rename = "cr")]
    pub accumulated_realized_pnl: String,

    #[serde(rename = "up")]
    pub unrealized_pnl: String,

    #[serde(rename = "mt")]
    pub margin_type: String,

    #[serde(rename = "iw")]
    pub isolated_wallet: String,

    #[serde(rename = "ps")]
    pub position_side: String,
}

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

pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: AccountUpdateEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                raw = &json[..json.len().min(200)],
                "解析失败"
            );
            return None;
        }
    };

    let reason = AccountUpdateReason::from_str(&event.update_data.reason_type);

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

    None
}
