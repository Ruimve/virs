use std::sync::Arc;

use crate::common::llm_client::LlmClient;
use virs_types::bot::{CredentialStore, LlmProviderResolver};
use virs_types::grid_port::GridBotConfig;
use virs_strategy::output::{StrategyAction, StrategyOutput, ToStrategyOutput};
use uuid::Uuid;
use virs_error::{BotError, BotResult};

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

#[derive(Debug, Clone)]
pub struct GridAiDecision {
    pub action: String,
    pub reason: String,
    pub confidence: f64,
    pub upper_price: f64,
    pub lower_price: f64,
    pub grid_count: i32,
    pub grid_profit_pct: f64,
    pub quantity_per_grid: f64,
    pub market_regime: String,
    pub analysis: String,
    pub risk_warning: String,
}

impl ToStrategyOutput for GridAiDecision {
    fn to_output(&self, raw: serde_json::Value, bot_id: Option<Uuid>) -> StrategyOutput {
        let action = match self.action.as_str() {
            "adjust_grid" => StrategyAction::AdjustGrid {
                upper_price: self.upper_price,
                lower_price: self.lower_price,
                grid_count: self.grid_count,
                grid_profit_pct: self.grid_profit_pct,
                quantity_per_grid: self.quantity_per_grid,
            },
            "pause_grid" => StrategyAction::PauseGrid,
            "run_grid" => StrategyAction::RunGrid,
            "reduce_position" => StrategyAction::ReducePosition,
            _ => StrategyAction::Hold,
        };
        // market_regime 为 "unknown" 时归一化为 None（与 AutoDecision 行为对齐）
        let market_regime = if self.market_regime.eq_ignore_ascii_case("unknown") {
            None
        } else {
            Some(self.market_regime.clone())
        };
        StrategyOutput {
            action,
            reason: self.reason.clone(),
            confidence: self.confidence,
            market_regime,
            analysis: Some(self.analysis.clone()),
            risk_warning: Some(self.risk_warning.clone()),
            funding_rate_warning: None,
            event_impact: None,
            decision_raw: raw,
            bot_id,
        }
    }
}

pub struct GridAiService {
    llm_client: LlmClient,
}

impl GridAiService {
    pub fn new(
        llm_resolver: Arc<dyn LlmProviderResolver>,
        credential_store: Arc<dyn CredentialStore>,
        llm_timeout: std::time::Duration,
    ) -> Self {
        Self {
            llm_client: LlmClient::new(llm_resolver, credential_store, llm_timeout),
        }
    }

    /// 检查指定用户是否有可用的 LLM 凭证（与 AutoAiService 对齐）。
    pub async fn is_available_for_user(&self, user_id: Uuid) -> bool {
        self.llm_client.is_available_for_user(user_id).await
    }

    pub async fn analyze(
        &self,
        bot: &GridBotConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> BotResult<(GridAiDecision, serde_json::Value, String)> {
        let result = self
            .llm_client
            .call(bot.user_id, system_prompt, user_prompt, "grid-ai")
            .await?;

        let decision = parse_grid_decision(&result.content)?;
        Ok((decision, result.content, result.used_model))
    }
}

pub fn parse_grid_decision(json: &serde_json::Value) -> BotResult<GridAiDecision> {
    let decision = &json["decision"];
    let grid = &json["grid"];
    let risk = &json["risk"];
    let market = &json["market"];

    let action = decision["action"]
        .as_str()
        .ok_or_else(|| BotError::Validation("LLM response missing 'decision.action'".to_string()))?;
    let reason = decision["reason"]
        .as_str()
        .ok_or_else(|| BotError::Validation("LLM response missing 'decision.reason'".to_string()))?
        .to_string();
    let confidence = decision["confidence"]
        .as_f64()
        .ok_or_else(|| {
            BotError::Validation("LLM response missing 'decision.confidence'".to_string())
        })?
        .clamp(0.0, 1.0);

    let upper_price = grid["upper_price"].as_f64().ok_or_else(|| {
        BotError::Validation("LLM response missing 'grid.upper_price'".to_string())
    })?;
    let lower_price = grid["lower_price"].as_f64().ok_or_else(|| {
        BotError::Validation("LLM response missing 'grid.lower_price'".to_string())
    })?;
    let grid_count = grid["grid_count"].as_i64().ok_or_else(|| {
        BotError::Validation("LLM response missing 'grid.grid_count'".to_string())
    })? as i32;
    let grid_profit_pct = grid["grid_profit_pct"].as_f64().ok_or_else(|| {
        BotError::Validation("LLM response missing 'grid.grid_profit_pct'".to_string())
    })?;

    let quantity_per_grid = risk["quantity_per_grid"].as_f64().ok_or_else(|| {
        BotError::Validation("LLM response missing 'risk.quantity_per_grid'".to_string())
    })?;

    let market_regime = market["market_regime"]
        .as_str()
        .ok_or_else(|| {
            BotError::Validation("LLM response missing 'market.market_regime'".to_string())
        })?
        .to_string();
    let analysis = json["analysis"]
        .as_str()
        .ok_or_else(|| BotError::Validation("LLM response missing 'analysis'".to_string()))?
        .to_string();
    let risk_warning = json["risk_warning"]
        .as_str()
        .ok_or_else(|| BotError::Validation("LLM response missing 'risk_warning'".to_string()))?
        .to_string();

    Ok(GridAiDecision {
        action: action.to_string(),
        reason,
        confidence,
        upper_price,
        lower_price,
        grid_count,
        grid_profit_pct,
        quantity_per_grid,
        market_regime,
        analysis,
        risk_warning,
    })
}
