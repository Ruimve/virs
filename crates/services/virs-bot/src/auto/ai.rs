//! Auto AI service — LLM decision for auto trading.

use std::sync::Arc;

use tracing::warn;
use uuid::Uuid;

use crate::common::ai_client;
use crate::common::ports::{CredentialStore, LlmProviderResolver};

/// Auto trading action
#[derive(Debug, Clone, PartialEq)]
pub enum AutoAction {
    OpenLong,
    OpenShort,
    ClosePosition,
    Hold,
}

impl AutoAction {
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenLong => "open_long",
            Self::OpenShort => "open_short",
            Self::ClosePosition => "close_position",
            Self::Hold => "hold",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "open_long" => Self::OpenLong,
            "open_short" => Self::OpenShort,
            "close_position" => Self::ClosePosition,
            "hold" => Self::Hold,
            _ => {
                warn!(action = s, "Unknown auto trade action, falling back to Hold");
                Self::Hold
            }
        }
    }
}

/// Auto trading AI decision result
#[derive(Debug, Clone)]
pub struct AutoDecision {
    pub action: AutoAction,
    pub reason: String,
    pub confidence: f64,
    /// LLM 返回的止损价（仅 open_long/open_short 时有效，其他动作为 None 或 0）
    pub stop_loss: Option<f64>,
    /// LLM 返回的止盈价（仅 open_long/open_short 时有效，其他动作为 None 或 0）
    pub take_profit: Option<f64>,
    pub market_regime: Option<String>,
    pub funding_rate_warning: Option<String>,
    pub event_impact: Option<String>,
    pub analysis: Option<String>,
    pub risk_warning: Option<String>,
}

impl AutoDecision {
    pub fn from_json(json: &serde_json::Value) -> Self {
        let decision = &json["decision"];
        let market = &json["market"];

        let action_str = decision["action"].as_str().unwrap_or("hold");
        let reason = decision["reason"].as_str().unwrap_or("No reason provided").to_string();
        let confidence = decision["confidence"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);

        // 解析 SL/TP：LLM 应在 open_long/open_short 时返回正数价格；其他动作或异常时为 None
        let stop_loss = decision["stop_loss"]
            .as_f64()
            .filter(|v| *v > 0.0);
        let take_profit = decision["take_profit"]
            .as_f64()
            .filter(|v| *v > 0.0);

        let market_regime = market["market_regime"].as_str().map(|s| s.to_string());
        let funding_rate_warning = market["funding_rate_warning"]
            .as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());
        let event_impact = market["event_impact"]
            .as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());

        let analysis = json["analysis"].as_str().map(|s| s.to_string());
        let risk_warning = json["risk_warning"]
            .as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());

        let action = AutoAction::from_str(action_str);

        AutoDecision {
            action,
            reason,
            confidence,
            stop_loss,
            take_profit,
            market_regime,
            funding_rate_warning,
            event_impact,
            analysis,
            risk_warning,
        }
    }
}

/// Auto AI 服务
pub struct AutoAiService {
    http_client: reqwest::Client,
    llm_resolver: Arc<dyn LlmProviderResolver>,
    credential_store: Arc<dyn CredentialStore>,
}

impl AutoAiService {
    pub fn new(
        llm_resolver: Arc<dyn LlmProviderResolver>,
        credential_store: Arc<dyn CredentialStore>,
    ) -> Self {
        Self {
            http_client: ai_client::create_llm_http_client(),
            llm_resolver,
            credential_store,
        }
    }

    pub fn is_available(&self) -> bool {
        self.llm_resolver.is_available()
    }

    pub async fn is_available_for_user(&self, user_id: Uuid) -> bool {
        if self.llm_resolver.is_available() {
            return true;
        }
        match self.credential_store.load_credentials(user_id).await {
            Ok(creds) => !creds.is_empty(),
            Err(_) => false,
        }
    }

    async fn call_llm(
        &self,
        user_id: Uuid,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<ai_client::LlmCallResult> {
        let user_creds = self.credential_store.load_credentials(user_id).await?;
        let (api_key, base_url, model, _provider) = self.llm_resolver.resolve(&user_creds)?;

        ai_client::call_llm_api(
            &self.http_client,
            &api_key,
            &base_url,
            &model,
            system_prompt,
            user_prompt,
            "auto-ai",
        )
        .await
    }

    pub async fn auto_decision(
        &self,
        user_id: Uuid,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Option<(AutoDecision, serde_json::Value, String)> {
        match self.call_llm(user_id, system_prompt, user_prompt).await {
            Ok(result) => {
                let decision = AutoDecision::from_json(&result.content);
                let used_model = result.used_model;
                Some((decision, result.content, used_model))
            }
            Err(e) => {
                warn!("LLM auto decision failed: {}", e);
                None
            }
        }
    }
}
