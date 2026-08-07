use crate::auth::Signer;
use crate::ExchangeClient;
use virs_error::ExchangeError;
use virs_type::ApiRestrictions;


const BASE_URL: &str = "https://api.binance.com";


fn url(path: &str) -> String {
    format!("{BASE_URL}{path}")
}


pub async fn fetch_api_restrictions(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<ApiRestrictions, ExchangeError> {
    let data = client
        .signed_get(signer, &url("/sapi/v1/account/apiRestrictions"), vec![])
        .await?;


    let ip_restrict = data
        .get("ipRestrict")
        .and_then(|v| v.as_bool());

    Ok(ApiRestrictions {
        ip_restrict,
        ip_whitelist: Vec::new(),
        ip_not_restricted: ip_restrict.map(|v| !v),

        create_sub_account: data
            .get("enableSubAccountCreation")
            .and_then(|v| v.as_bool()),

        read_info: data
            .get("enableReading")
            .and_then(|v| v.as_bool()),

        enable_withdrawals: data
            .get("enableWithdrawals")
            .and_then(|v| v.as_bool()),

        enable_internal_transfer: data
            .get("enableInternalTransfer")
            .and_then(|v| v.as_bool()),

        enable_futures: data
            .get("enableFutures")
            .and_then(|v| v.as_bool()),

        enable_vanilla_options: data
            .get("enableVanillaOptions")
            .and_then(|v| v.as_bool()),

        enable_portfolio_margin_trading: data
            .get("enablePortfolioMarginTrading")
            .and_then(|v| v.as_bool()),

        enable_fix_api_trade: data
            .get("enableFixApiTrade")
            .and_then(|v| v.as_bool()),

        enable_fix_api_read: data
            .get("enableFixApiRead")
            .and_then(|v| v.as_bool()),
        info: data,
    })
}
