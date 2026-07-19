//! AI 策略 prompt 生成器。
//!
//! 通过 LLM 生成符合 [`PromptTemplate`] 格式的策略 prompt。
//!
//! 流程：
//! 1. 调用方提供策略类型（auto/grid）+ 用户意图描述
//! 2. 构造元 prompt（教 LLM 如何生成策略 prompt）
//! 3. 调用 LLM（复用 [`crate::common::ai_client::call_llm_api`])
//! 4. 解析 LLM 返回的 JSON 为 [`PromptTemplate`]
//! 5. 校验（占位符白名单 + JSON schema 约束）
//! 6. 返回校验通过的模板

use virs_error::{BotError, BotResult};

use crate::common::ai_client;
use crate::strategy::prompt::template::{PromptSource, PromptTemplate, StrategyType};
use crate::strategy::prompt::validator::validate;

/// AI 生成请求。
pub struct GenerateRequest<'a> {
    /// 策略类型
    pub strategy_type: StrategyType,
    /// 用户意图描述（如"做一个趋势跟随策略，4h 定方向，1h 入场"）
    pub user_intent: &'a str,
    /// 模板名（文件名，不含扩展名）。为空时由 LLM 命名
    pub name_hint: Option<&'a str>,
}

/// AI 生成结果。
pub struct GenerateResult {
    pub template: PromptTemplate,
    pub used_model: String,
}

/// 调用 LLM 生成策略 prompt。
///
/// `api_key` / `base_url` / `model` 由调用方解析（通常从用户 AI 凭证或全局配置获取）。
pub async fn generate_prompt(
    http_client: &reqwest::Client,
    api_key: &str,
    base_url: &str,
    model: &str,
    req: GenerateRequest<'_>,
) -> BotResult<GenerateResult> {
    let system = build_meta_system_prompt(req.strategy_type);
    let user = build_meta_user_prompt(&req);

    let result = ai_client::call_llm_api(
        http_client,
        api_key,
        base_url,
        model,
        &system,
        &user,
        "strategy-generator",
    )
    .await?;

    let mut tpl: PromptTemplate = serde_json::from_value(result.content.clone()).map_err(|e| {
        BotError::Llm(format!(
            "LLM 返回的 JSON 无法解析为 PromptTemplate: {e}"
        ))
    })?;

    // 强制覆盖 strategy_type（防止 LLM 返回错误的类型）
    tpl.strategy_type = req.strategy_type;

    // 如果调用方提供了 name_hint，覆盖 LLM 的命名
    if let Some(name) = req.name_hint {
        if !name.is_empty() {
            tpl.name = name.to_string();
        }
    }

    // 标记来源为 AI 生成
    tpl.source = PromptSource::AiGenerated {
        model: model.to_string(),
        generation_prompt: req.user_intent.to_string(),
    };

    // 校验
    validate(&tpl).map_err(|e| {
        BotError::Llm(format!("AI 生成的策略 prompt 校验失败: {e}"))
    })?;

    Ok(GenerateResult {
        template: tpl,
        used_model: result.used_model,
    })
}

/// 元 system prompt：教 LLM 如何生成策略 prompt。
fn build_meta_system_prompt(strategy_type: StrategyType) -> String {
    let strategy_desc = match strategy_type {
        StrategyType::Auto => {
            "Auto 趋势策略（单仓位方向判断：open_long/open_short/close_position/hold）"
        }
        StrategyType::Grid => {
            "Grid 网格策略（网格结构调整：adjust_grid/pause_grid/run_grid/reduce_position/hold）"
        }
    };

    format!(
        r#"你是一个策略 prompt 生成器。你的任务是根据用户的意图描述，生成一个 {strategy_desc} 的策略 prompt。

你必须返回一个 JSON 对象，包含以下字段：
{{
  "name": "策略名（英文，下划线分隔，如 trend_following）",
  "strategy_type": "{strategy_type_str}",
  "system_prompt": "给交易 LLM 的 system prompt。定义角色、交易规则、输出 JSON 格式约束。必须包含 'JSON' 字样。",
  "user_prompt_template": "用户 prompt 模板，使用 {{placeholder}} 占位符引用指标和上下文",
  "required_placeholders": ["占位符列表，与 user_prompt_template 中使用的 {{placeholder}} 一一对应"],
  "description": "策略的中文描述"
}}

可用占位符白名单（只能使用以下占位符，不得发明新的）：
- 通用：timestamp, symbol, exchange, leverage, total_balance, available_balance, used_margin, margin_usage_rate, min_qty, funding_rate, funding_next_time
- 仓位：position_info, position_duration, stop_take_profit_info, recent_close_info, total_trades, win_trades, loss_trades, total_pnl, consecutive_losses, trigger_reason
- H1 指标：h1_current_price, h1_rsi, h1_atr, h1_adx, h1_macd, h1_macd_signal, h1_macd_histogram, h1_ema20, h1_ema50, h1_ema_cross_bars_ago, h1_ema_gap_pct, h1_ema_gap_trend, h1_bb_upper, h1_bb_middle, h1_bb_lower, h1_high_20, h1_low_20, h1_high_50, h1_low_50, h1_volume, h1_volume_sma20, h1_candle_body, h1_bars_outside_band, h1_bandwidth_5bars_ago, h1_ema_cross, h1_change, h1_bb_width_pct, nearest_round_up, nearest_round_down, h1_atr_sma20
- M15 指标：m15_current_price, m15_rsi, m15_macd, m15_macd_signal, m15_macd_histogram, m15_atr, m15_adx, m15_ema20, m15_ema50, m15_ema_cross, m15_ema_cross_bars_ago, m15_volume, m15_volume_sma20, m15_high_50, m15_low_50, m15_bb_width_pct, m15_atr_sma20, m15_bars_outside_band
- H4 指标：h4_ema20, h4_ema50, h4_adx, h4_rsi, h4_macd, h4_macd_signal, h4_macd_histogram, h4_bb_width_pct
- Grid 专属：grid_status, last_adjust_time, current_grid_config, event_flag, event_description

system_prompt 要求：
1. 定义 LLM 角色（如"你是趋势跟随交易引擎"）
2. 明确交易规则（入场条件、出场条件、风控规则）
3. 必须规定输出 JSON 格式（包含 decision.action / decision.reason / decision.confidence 等字段）
4. 对于 Auto 策略，action 可选值：open_long, open_short, close_position, hold
5. 对于 Grid 策略，action 可选值：adjust_grid, pause_grid, run_grid, reduce_position, hold

user_prompt_template 要求：
1. 用 {{placeholder}} 引用指标值，不得硬编码数值
2. 包含账户余额、仓位信息、多周期指标
3. 结构清晰，分段落展示

只返回 JSON 对象，不要包含其他文字。"#,
        strategy_type_str = match strategy_type {
            StrategyType::Auto => "auto",
            StrategyType::Grid => "grid",
        }
    )
}

/// 元 user prompt：传递用户意图。
fn build_meta_user_prompt(req: &GenerateRequest<'_>) -> String {
    let name_hint = req.name_hint.unwrap_or("（由你命名）");
    format!(
        r#"请根据以下意图生成策略 prompt：

策略类型：{strategy_type}
策略名：{name_hint}
用户意图：{user_intent}

请生成完整的策略 prompt JSON。"#,
        strategy_type = match req.strategy_type {
            StrategyType::Auto => "Auto 趋势策略",
            StrategyType::Grid => "Grid 网格策略",
        },
        name_hint = name_hint,
        user_intent = req.user_intent,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g1_meta_system_prompt_contains_json_constraint() {
        let s = build_meta_system_prompt(StrategyType::Auto);
        assert!(s.contains("JSON"));
        assert!(s.contains("open_long"));
    }

    #[test]
    fn g2_meta_system_prompt_grid_contains_grid_actions() {
        let s = build_meta_system_prompt(StrategyType::Grid);
        assert!(s.contains("adjust_grid"));
        assert!(s.contains("pause_grid"));
    }

    #[test]
    fn g3_meta_user_prompt_contains_intent() {
        let req = GenerateRequest {
            strategy_type: StrategyType::Auto,
            user_intent: "做多趋势策略",
            name_hint: Some("my_trend"),
        };
        let u = build_meta_user_prompt(&req);
        assert!(u.contains("做多趋势策略"));
        assert!(u.contains("my_trend"));
    }
}
