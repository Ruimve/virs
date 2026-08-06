use crate::adapter::binance::user_data_ws::*;
use crate::adapter::binance::BinanceSigner;
use crate::ExchangeClient;
use std::sync::Arc;

#[test]
fn test_parse_invalid_json() {
    let result: Result<BinanceOrderMessage, _> = serde_json::from_str("not json");
    assert!(result.is_err());
}

#[test]
fn test_new_perpetual() {
    let client = ExchangeClient::with_api_key(
        10,
        None,
        Some("test_api_key"),
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(3),
        10,
    )
    .expect("Failed to create test ExchangeClient");
    let signer = Arc::new(BinanceSigner::new(
        "test_api_key".to_string(),
        "test_api_secret".to_string(),
    ));
    let ws = UserDataWs::new_perpetual("test_listen_key".to_string(), client, signer);
    let _ = ws;
}
