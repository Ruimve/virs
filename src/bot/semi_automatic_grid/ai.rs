use tracing::warn;
use uuid::Uuid;

use crate::bot::semi_automatic_grid::ports::{CredentialStore, LlmProviderResolver};
use crate::bot::semi_automatic_grid::utils::ai_client::{call_llm_api, create_llm_http_client};

#[derive(Debug, Clone)]
pub enum GridAction {
    RunGrid,
    PauseGrid,
    AdjustGrid {
        upper_price: Option<f64>,
        lower_price: Option<f64>,
    },
    ReducePosition,
    Hold,
}

impl GridAction {
    pub fn as_str(&self) -> &str {
        match self {
            Self::RunGrid => "run_grid",
            Self::PauseGrid => "pause_grid",
            Self::AdjustGrid { .. } => "adjust_grid",
            Self::ReducePosition => "reduce_position",
            Self::Hold => "hold",
        }
    }

    pub fn from_str(s: &str, upper_price: Option<f64>, lower_price: Option<f64>) -> Self {
        match s.to_lowercase().as_str() {
            "run_grid" => GridAction::RunGrid,
            "pause_grid" => GridAction::PauseGrid,
            "adjust_grid" => GridAction::AdjustGrid {
                upper_price,
                lower_price,
            },
            "reduce_position" => GridAction::ReducePosition,
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
    pub upper_price: Option<f64>,
    pub lower_price: Option<f64>,
}

impl GridDecision {
    pub fn from_json(json: &serde_json::Value) -> Self {
        let action_str = json["action"].as_str().unwrap_or("hold");
        let reason = json["reason"]
            .as_str()
            .unwrap_or("No reason provided")
            .to_string();
        let mut upper_price = json["upper_price"].as_f64();
        let mut lower_price = json["lower_price"].as_f64();

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

        let action = GridAction::from_str(action_str, upper_price, lower_price);

        GridDecision {
            action,
            reason,
            upper_price,
            lower_price,
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
    ) -> Option<GridDecision> {
        match self.call_llm(user_id, system_prompt, user_prompt).await {
            Ok(json) => Some(GridDecision::from_json(&json)),
            Err(e) => {
                warn!("LLM grid decision failed, falling back to rules: {}", e);
                None
            }
        }
    }
}
