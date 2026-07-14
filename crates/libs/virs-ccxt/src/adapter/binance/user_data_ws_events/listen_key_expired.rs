use serde::Deserialize;
use virs_types::WsFeedEvent;


#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ListenKeyExpiredEvent {

    #[serde(rename = "e")]
    pub event_type: String,

    #[serde(rename = "E")]
    pub event_time: i64,

    #[serde(rename = "listenKey")]
    pub listen_key: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenKeyExpiredAction {

    NeedRecreate,

    LogOnly,
}


pub fn process(json: &str) -> Option<WsFeedEvent> {
    let event: ListenKeyExpiredEvent = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "[listenKeyExpired] 解析失败: {}",
                &json[..json.len().min(200)]
            );
            return None;
        }
    };

    tracing::error!(
        expired_key = %event.listen_key,
        event_time = event.event_time,
        "listenKey 已过期 — 需要重新创建 listenKey 后重连，当前重连使用旧 key 将失败"
    );


    None
}
