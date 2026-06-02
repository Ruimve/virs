use tracing::warn;
use uuid::Uuid;

use crate::bot::auto_trade::ports::{CredentialStore, LlmProviderResolver};
use crate::bot::common::ai_client::{call_llm_api, create_llm_http_client};

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
                tracing::warn!(action = s, "Unknown auto trade action, falling back to Hold");
                Self::Hold
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AutoDecision {
    pub action: AutoAction,
    pub reason: String,
    pub confidence: f64,
    pub close_reason: Option<String>,
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

        let action_str = decision["action"]
            .as_str()
            .unwrap_or("hold");
        let reason = decision["reason"]
            .as_str()
            .unwrap_or("No reason provided")
            .to_string();
        let confidence = decision["confidence"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);

        let market_regime = market["market_regime"].as_str().map(|s| s.to_string());
        let funding_rate_warning = market["funding_rate_warning"].as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());
        let event_impact = market["event_impact"].as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());

        let analysis = json["analysis"].as_str().map(|s| s.to_string());
        let risk_warning = json["risk_warning"].as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());

        let close_reason = decision["close_reason"]
            .as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());

        let action = AutoAction::from_str(action_str);

        AutoDecision {
            action,
            reason,
            confidence,
            close_reason,
            market_regime,
            funding_rate_warning,
            event_impact,
            analysis,
            risk_warning,
        }
    }
}

pub struct AutoAiService {
    resolver: Box<dyn LlmProviderResolver>,
    credential_store: Box<dyn CredentialStore>,
    http_client: reqwest::Client,
}

impl AutoAiService {
    pub fn new(
        resolver: Box<dyn LlmProviderResolver>,
        credential_store: Box<dyn CredentialStore>,
    ) -> Self {
        Self {
            resolver,
            credential_store,
            http_client: create_llm_http_client(),
        }
    }

    pub fn is_available(&self) -> bool {
        self.resolver.is_available()
    }

    pub async fn is_available_for_user(&self, user_id: &Uuid) -> bool {
        if self.resolver.is_available() {
            return true;
        }
        match self.credential_store.load_credentials(*user_id).await {
            Ok(creds) => !creds.is_empty(),
            Err(_) => false,
        }
    }

    pub async fn call_llm(
        &self,
        user_id: &Uuid,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let user_creds = self.credential_store.load_credentials(*user_id).await?;
        let (api_key, base_url, model, provider) =
            self.resolver.resolve(&user_creds)?;

        let result = call_llm_api(
            &self.http_client,
            &api_key,
            &base_url,
            &model,
            system_prompt,
            user_prompt,
            &provider,
        ).await?;

        Ok(result.content)
    }

    pub async fn auto_decision(
        &self,
        user_id: &Uuid,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Option<(AutoDecision, serde_json::Value)> {
        match self.call_llm(user_id, system_prompt, user_prompt).await {
            Ok(json) => {
                let decision = AutoDecision::from_json(&json);
                Some((decision, json))
            }
            Err(e) => {
                warn!("LLM auto decision failed: {}", e);
                None
            }
        }
    }
}
