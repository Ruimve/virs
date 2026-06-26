//! Binance Wallet & Account REST API (/sapi/v1) — account management, funds, API restrictions.
//!
//! Endpoints:
//! - GET /sapi/v1/account/apiRestrictions

use crate::errors::ExchangeError;
use crate::ExchangeClient;
use crate::types::*;

const BASE_URL: &str = "https://api.binance.com";

fn url(path: &str) -> String {
    format!("{BASE_URL}{path}")
}

/// GET /sapi/v1/account/apiRestrictions — fetch API key restrictions.
pub async fn fetch_api_restrictions(
    client: &ExchangeClient,
    signer: &super::BinanceSigner,
) -> Result<ApiRestrictions, ExchangeError> {
    let data = client
        .signed_get(signer, &url("/sapi/v1/account/apiRestrictions"), vec![])
        .await?;

    let ip_restrict = data.get("ipRestrict").and_then(|v| v.as_bool()).unwrap_or(false);

    Ok(ApiRestrictions {
        ip_restrict,
        ip_whitelist: Vec::new(),
        ip_not_restricted: !ip_restrict,
        create_sub_account: data.get("enableSubAccountCreation").and_then(|v| v.as_bool()).unwrap_or(false),
        read_info: data.get("enableReading").and_then(|v| v.as_bool()).unwrap_or(true),
        enable_spot_and_margin_trading: data.get("enableSpotAndMarginTrading").and_then(|v| v.as_bool()).unwrap_or(false),
        enable_withdrawals: data.get("enableWithdrawals").and_then(|v| v.as_bool()).unwrap_or(false),
        enable_internal_transfer: data.get("enableInternalTransfer").and_then(|v| v.as_bool()).unwrap_or(false),
        enable_futures: data.get("enableFutures").and_then(|v| v.as_bool()).unwrap_or(false),
        enable_vanilla_options: data.get("enableVanillaOptions").and_then(|v| v.as_bool()).unwrap_or(false),
        enable_portfolio_margin_trading: data.get("enablePortfolioMarginTrading").and_then(|v| v.as_bool()).unwrap_or(false),
        enable_fix_api_trade: data.get("enableFixApiTrade").and_then(|v| v.as_bool()).unwrap_or(false),
        enable_fix_api_read: data.get("enableFixApiRead").and_then(|v| v.as_bool()).unwrap_or(false),
        info: data,
    })
}
