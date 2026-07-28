use crate::auth::Signer;
use crate::ExchangeClient;
use virs_error::ExchangeError;
use virs_types::ApiRestrictions;

// 币安现货 API 基础域名 (注意: 现货域名，非合约域名 fapi.binance.com)
const BASE_URL: &str = "https://api.binance.com";

// 拼接完整请求 URL
fn url(path: &str) -> String {
    format!("{BASE_URL}{path}")
}

// API权限查询 (签名，现货域名)
// GET /sapi/v1/account/apiRestrictions - 查询当前 API Key 的各项权限开关
pub async fn fetch_api_restrictions(
    client: &ExchangeClient,
    signer: &dyn Signer,
) -> Result<ApiRestrictions, ExchangeError> {
    let data = client
        .signed_get(signer, &url("/sapi/v1/account/apiRestrictions"), vec![])
        .await?;

    // 解析 IP 限制开关
    let ip_restrict = data
        .get("ipRestrict")
        .and_then(|v| v.as_bool());

    Ok(ApiRestrictions {
        ip_restrict,
        ip_whitelist: Vec::new(),
        ip_not_restricted: ip_restrict.map(|v| !v),
        // 是否允许创建子账户
        create_sub_account: data
            .get("enableSubAccountCreation")
            .and_then(|v| v.as_bool()),
        // 是否允许读取信息
        read_info: data
            .get("enableReading")
            .and_then(|v| v.as_bool()),
        // 是否允许提币
        enable_withdrawals: data
            .get("enableWithdrawals")
            .and_then(|v| v.as_bool()),
        // 是否允许内部转账
        enable_internal_transfer: data
            .get("enableInternalTransfer")
            .and_then(|v| v.as_bool()),
        // 是否允许合约交易
        enable_futures: data
            .get("enableFutures")
            .and_then(|v| v.as_bool()),
        // 是否允许欧式期权交易
        enable_vanilla_options: data
            .get("enableVanillaOptions")
            .and_then(|v| v.as_bool()),
        // 是否允许组合保证金交易
        enable_portfolio_margin_trading: data
            .get("enablePortfolioMarginTrading")
            .and_then(|v| v.as_bool()),
        // 是否允许 FIX 协议交易
        enable_fix_api_trade: data
            .get("enableFixApiTrade")
            .and_then(|v| v.as_bool()),
        // 是否允许 FIX 协议读取
        enable_fix_api_read: data
            .get("enableFixApiRead")
            .and_then(|v| v.as_bool()),
        info: data,
    })
}
