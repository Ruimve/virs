use std::sync::Arc;

use tracing::warn;
use uuid::Uuid;
use virs_error::{BotError, BotResult};

use crate::common::llm_client::LlmClient;
use virs_type::{CredentialStore, LlmProviderResolver};

#[derive(Debug, Clone, PartialEq)]
pub enum AutoAction {
    OpenLong,
    OpenShort,
    ClosePosition,
    Hold,
}

/* LLM返回的未知action默认降级为Hold，避免执行不可预期的操作 */
impl AutoAction {
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenLong => "open_long",
            Self::OpenShort => "open_short",
            Self::ClosePosition => "close_position",
            Self::Hold => "hold",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "open_long" => Self::OpenLong,
            "open_short" => Self::OpenShort,
            "close_position" => Self::ClosePosition,
            "hold" => Self::Hold,
            _ => {
                warn!(
                    action = s,
                    "Unknown auto trade action, falling back to Hold"
                );
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

    pub market_regime: Option<String>,
    pub funding_rate_warning: Option<String>,
    pub event_impact: Option<String>,
    pub analysis: Option<String>,
    pub risk_warning: Option<String>,
}

impl AutoDecision {
    /* 从LLM返回的JSON解析决策：confidence被限制在[0,1]范围，
     * "none"字符串的funding_rate_warning/event_impact/risk_warning被过滤为None */
    pub fn from_json(json: &serde_json::Value) -> BotResult<Self> {
        let decision = &json["decision"];
        let market = &json["market"];

        let action_str = decision["action"].as_str().ok_or_else(|| {
            BotError::Validation("LLM response missing 'decision.action'".to_string())
        })?;
        let reason = decision["reason"]
            .as_str()
            .ok_or_else(|| {
                BotError::Validation("LLM response missing 'decision.reason'".to_string())
            })?
            .to_string();
        let confidence = decision["confidence"]
            .as_f64()
            .ok_or_else(|| {
                BotError::Validation("LLM response missing 'decision.confidence'".to_string())
            })?
            .clamp(0.0, 1.0);

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

        Ok(AutoDecision {
            action,
            reason,
            confidence,
            market_regime,
            funding_rate_warning,
            event_impact,
            analysis,
            risk_warning,
        })
    }
}

pub(crate) struct AutoAiService {
    llm_client: LlmClient,
}

impl AutoAiService {
    pub(crate) fn new(
        llm_resolver: Arc<dyn LlmProviderResolver>,
        credential_store: Arc<dyn CredentialStore>,
        llm_timeout: std::time::Duration,
    ) -> Self {
        Self {
            llm_client: LlmClient::new(llm_resolver, credential_store, llm_timeout),
        }
    }

    pub(crate) async fn is_available_for_user(&self, user_id: Uuid) -> bool {
        self.llm_client.is_available_for_user(user_id).await
    }

    pub(crate) async fn auto_decision(
        &self,
        user_id: Uuid,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Option<(AutoDecision, serde_json::Value, String)> {
        match self
            .llm_client
            .call(user_id, system_prompt, user_prompt, "auto-ai")
            .await
        {
            Ok(result) => {
                let used_model = result.used_model;
                match AutoDecision::from_json(&result.content) {
                    Ok(decision) => Some((decision, result.content, used_model)),
                    Err(e) => {
                        warn!(error = %e, "Failed to parse auto decision");
                        None
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "LLM auto decision failed");
                None
            }
        }
    }
}
