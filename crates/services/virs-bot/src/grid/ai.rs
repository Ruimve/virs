//! Grid AI service.

use std::sync::Arc;

use crate::common::ai_client;
use crate::common::ports::CredentialStore;
use crate::common::ports::LlmProviderResolver;
use crate::grid::ports::GridBotConfig;

/// Grid AI 决策动作
#[derive(Debug, Clone, PartialEq)]
pub enum GridAction {
    Hold,
    AdjustGrid { upper_price: f64, lower_price: f64 },
    PauseGrid,
    RunGrid,
    ReducePosition,
}

impl GridAction {
    pub fn from_str(action: &str, upper_price: f64, lower_price: f64) -> Self {
        match action {
            "adjust_grid" => GridAction::AdjustGrid {
                upper_price,
                lower_price,
            },
            "pause_grid" => GridAction::PauseGrid,
            "run_grid" => GridAction::RunGrid,
            "reduce_position" => GridAction::ReducePosition,
            _ => GridAction::Hold,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            GridAction::Hold => "hold",
            GridAction::AdjustGrid { .. } => "adjust_grid",
            GridAction::PauseGrid => "pause_grid",
            GridAction::RunGrid => "run_grid",
            GridAction::ReducePosition => "reduce_position",
        }
    }
}

/// Grid AI 决策结果
#[derive(Debug, Clone)]
pub struct GridAiDecision {
    pub action: String,
    pub reason: String,
    pub confidence: f64,
    pub upper_price: f64,
    pub lower_price: f64,
    pub grid_count: i32,
    pub grid_profit_pct: f64,
    pub leverage: i32,
    pub quantity_per_grid: f64,
    pub market_regime: String,
    pub analysis: String,
    pub risk_warning: String,
}

/// Grid AI 服务
pub struct GridAiService {
    http_client: reqwest::Client,
    llm_resolver: Arc<dyn LlmProviderResolver>,
    credential_store: Arc<dyn CredentialStore>,
}

impl GridAiService {
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

    /// 执行 AI 分析
    pub async fn analyze(
        &self,
        bot: &GridBotConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<(GridAiDecision, String)> {
        let credentials = self.credential_store.load_credentials(bot.user_id).await?;
        let (api_key, base_url, model, _provider) = self.llm_resolver.resolve(&credentials)?;

        let result = ai_client::call_llm_api(
            &self.http_client,
            &api_key,
            &base_url,
            &model,
            system_prompt,
            user_prompt,
            "grid-ai",
        )
        .await?;

        let decision = parse_grid_decision(&result.content)?;
        Ok((decision, result.used_model))
    }
}

pub fn parse_grid_decision(json: &serde_json::Value) -> anyhow::Result<GridAiDecision> {
    let decision = &json["decision"];
    let grid = &json["grid"];
    let risk = &json["risk"];
    let market = &json["market"];

    Ok(GridAiDecision {
        action: decision["action"].as_str().unwrap_or("hold").to_string(),
        reason: decision["reason"].as_str().unwrap_or("").to_string(),
        confidence: decision["confidence"].as_f64().unwrap_or(0.5),
        upper_price: grid["upper_price"].as_f64().unwrap_or(0.0),
        lower_price: grid["lower_price"].as_f64().unwrap_or(0.0),
        grid_count: grid["grid_count"].as_i64().unwrap_or(10) as i32,
        grid_profit_pct: grid["grid_profit_pct"].as_f64().unwrap_or(0.5),
        leverage: risk["leverage"].as_i64().unwrap_or(5) as i32,
        quantity_per_grid: risk["quantity_per_grid"].as_f64().unwrap_or(10.0),
        market_regime: market["market_regime"]
            .as_str()
            .unwrap_or("ranging")
            .to_string(),
        analysis: json["analysis"].as_str().unwrap_or("").to_string(),
        risk_warning: json["risk_warning"].as_str().unwrap_or("").to_string(),
    })
}
