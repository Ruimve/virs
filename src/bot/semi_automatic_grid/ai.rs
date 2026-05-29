use tracing::warn;
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ports::{CredentialStore, LlmProviderResolver};
use crate::bot::semi_automatic_grid::utils::ai_client::{call_llm_api, create_llm_http_client};

#[derive(Debug, Clone, PartialEq)]
pub enum GridAction {
    RunGrid,
    PauseGrid,
    AdjustGrid {
        upper_price: Option<f64>,
        lower_price: Option<f64>,
    },
    ReducePosition,
    CancelOrder {
        level: i32,
        side: String,
    },
    Hold,
}

impl GridAction {
    pub fn as_str(&self) -> &str {
        match self {
            Self::RunGrid => "resume_grid",
            Self::PauseGrid => "pause_grid",
            Self::AdjustGrid { .. } => "adjust_grid",
            Self::ReducePosition => "reduce_position",
            Self::CancelOrder { .. } => "cancel_order",
            Self::Hold => "hold",
        }
    }

    pub fn from_str(s: &str, upper_price: Option<f64>, lower_price: Option<f64>, cancel_level: Option<i32>, cancel_side: Option<String>) -> Self {
        match s.to_lowercase().as_str() {
            "resume_grid" => GridAction::RunGrid,
            "pause_grid" => GridAction::PauseGrid,
            "adjust_grid" => GridAction::AdjustGrid {
                upper_price,
                lower_price,
            },
            "reduce_position" => GridAction::ReducePosition,
            "cancel_order" => GridAction::CancelOrder {
                level: cancel_level.unwrap_or(0),
                side: cancel_side.unwrap_or_else(|| "buy".to_string()),
            },
            "hold" => GridAction::Hold,
            _ => {
                tracing::warn!(action = s, "Unknown LLM action, falling back to Hold");
                GridAction::Hold
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct GridDecision {
    pub action: GridAction,
    pub reason: String,
    pub confidence: f64,
    pub upper_price: Option<f64>,
    pub lower_price: Option<f64>,
    pub cancel_level: Option<i32>,
    pub cancel_side: Option<String>,
    pub grid_count: Option<i32>,
    pub grid_profit_pct: Option<f64>,
    pub quantity_per_grid: Option<f64>,
    pub leverage: Option<i32>,
    pub market_regime: Option<String>,
    pub analysis: Option<String>,
    pub funding_rate_warning: Option<String>,
    pub event_impact: Option<String>,
    pub risk_warning: Option<String>,
}

impl GridDecision {
    pub fn from_json(json: &serde_json::Value) -> Self {
        let action_str = json["recommended_action"]
            .as_str()
            .or_else(|| json["action"].as_str())
            .unwrap_or("hold");
        let reason = json["action_reason"]
            .as_str()
            .or_else(|| json["reason"].as_str())
            .unwrap_or("No reason provided")
            .to_string();
        let mut upper_price = json["upper_price"].as_f64();
        let mut lower_price = json["lower_price"].as_f64();
        let cancel_level = json["cancel_level"].as_i64().map(|v| v as i32);
        let cancel_side = json["cancel_side"].as_str().map(|s| s.to_string());
        let grid_count = json["grid_count"].as_i64().map(|v| v as i32);
        let grid_profit_pct = json["grid_profit_pct"].as_f64();
        let quantity_per_grid = json["quantity_per_grid"].as_f64();
        let leverage = json["leverage"].as_i64().map(|v| v as i32);
        let market_regime = json["market_regime"].as_str().map(|s| s.to_string());
        let analysis = json["analysis"].as_str().map(|s| s.to_string());
        let confidence = json["confidence"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);
        let funding_rate_warning = json["funding_rate_warning"].as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());
        let event_impact = json["event_impact"].as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());
        let risk_warning = json["risk_warning"].as_str()
            .filter(|s| !s.eq_ignore_ascii_case("none"))
            .map(|s| s.to_string());

        if upper_price.is_some() && upper_price.unwrap() <= 0.0 {
            warn!("GridDecision: upper_price <= 0, ignoring");
            upper_price = None;
        }
        if lower_price.is_some() && lower_price.unwrap() <= 0.0 {
            warn!("GridDecision: lower_price <= 0, ignoring");
            lower_price = None;
        }
        if let (Some(u), Some(l)) = (upper_price, lower_price) {
            if u <= l {
                warn!(upper = u, lower = l, "GridDecision: upper_price <= lower_price, ignoring both");
                upper_price = None;
                lower_price = None;
            }
        }

        let action = GridAction::from_str(action_str, upper_price, lower_price, cancel_level, cancel_side.clone());

        GridDecision {
            action,
            reason,
            confidence,
            upper_price,
            lower_price,
            cancel_level,
            cancel_side,
            grid_count,
            grid_profit_pct,
            quantity_per_grid,
            leverage,
            market_regime,
            analysis,
            funding_rate_warning,
            event_impact,
            risk_warning,
        }
    }
}

pub struct GridAiService {
    resolver: Box<dyn LlmProviderResolver>,
    credential_store: Box<dyn CredentialStore>,
    http_client: reqwest::Client,
}

impl GridAiService {
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

    pub async fn grid_decision(
        &self,
        user_id: &Uuid,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Option<(GridDecision, serde_json::Value)> {
        match self.call_llm(user_id, system_prompt, user_prompt).await {
            Ok(json) => {
                let decision = GridDecision::from_json(&json);
                Some((decision, json))
            }
            Err(e) => {
                warn!("LLM grid decision failed, falling back to rules: {}", e);
                None
            }
        }
    }
}
