//! Grid AI service.

use std::sync::Arc;

use crate::common::ai_client;
use crate::common::ports::CredentialStore;
use crate::common::ports::LlmProviderResolver;
use crate::grid::ports::GridBotConfig;
use tracing::warn;
use virs_error::BotResult;

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
        llm_timeout: std::time::Duration,
    ) -> Self {
        Self {
            http_client: ai_client::create_llm_http_client(llm_timeout),
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
    ) -> BotResult<(GridAiDecision, String)> {
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

        let decision = parse_grid_decision(&result.content);
        Ok((decision, result.used_model))
    }
}

pub fn parse_grid_decision(json: &serde_json::Value) -> GridAiDecision {
    let decision = &json["decision"];
    let grid = &json["grid"];
    let risk = &json["risk"];
    let market = &json["market"];

    let action = decision["action"].as_str().unwrap_or_else(|| {
        warn!("LLM response missing 'decision.action' field — defaulting to hold");
        "hold"
    });
    let reason = decision["reason"]
        .as_str()
        .unwrap_or("No reason provided")
        .to_string();
    let confidence = decision["confidence"]
        .as_f64()
        .unwrap_or_else(|| {
            warn!("LLM response missing 'decision.confidence' field — defaulting to 0.0");
            0.0
        })
        .clamp(0.0, 1.0);

    // Grid price bounds are critical — 0.0 means invalid grid range.
    // Log at warn level so operators can detect incomplete LLM responses.
    let upper_price = grid["upper_price"].as_f64().unwrap_or_else(|| {
        warn!("LLM response missing 'grid.upper_price' — grid range is invalid (0.0)");
        0.0
    });
    let lower_price = grid["lower_price"].as_f64().unwrap_or_else(|| {
        warn!("LLM response missing 'grid.lower_price' — grid range is invalid (0.0)");
        0.0
    });
    let grid_count = grid["grid_count"].as_i64().unwrap_or_else(|| {
        warn!("LLM response missing 'grid.grid_count' — defaulting to 0");
        0
    }) as i32;
    let grid_profit_pct = grid["grid_profit_pct"].as_f64().unwrap_or_else(|| {
        warn!("LLM response missing 'grid.grid_profit_pct' — defaulting to 0.0");
        0.0
    });

    let leverage = risk["leverage"].as_i64().unwrap_or_else(|| {
        warn!("LLM response missing 'risk.leverage' — defaulting to 1");
        1
    }) as i32;
    let quantity_per_grid = risk["quantity_per_grid"].as_f64().unwrap_or_else(|| {
        warn!("LLM response missing 'risk.quantity_per_grid' — defaulting to 0.0");
        0.0
    });

    let market_regime = market["market_regime"]
        .as_str()
        .unwrap_or_else(|| {
            warn!("LLM response missing 'market.market_regime' — defaulting to 'unknown'");
            "unknown"
        })
        .to_string();
    let analysis = json["analysis"]
        .as_str()
        .unwrap_or("No analysis provided")
        .to_string();
    let risk_warning = json["risk_warning"]
        .as_str()
        .unwrap_or("No risk warning")
        .to_string();

    GridAiDecision {
        action: action.to_string(),
        reason,
        confidence,
        upper_price,
        lower_price,
        grid_count,
        grid_profit_pct,
        leverage,
        quantity_per_grid,
        market_regime,
        analysis,
        risk_warning,
    }
}
