//! 策略优化器：基于绩效指标，通过 LLM 优化策略 prompt。

use virs_error::{BotError, BotResult};
use virs_llm::call_llm_api;

use crate::prompt::{PromptSource, PromptTemplate, validate};

use super::types::StrategyMetrics;

/// 策略优化器。
pub struct StrategyOptimizer {
    http_client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

/// 优化结果。
pub struct OptimizationResult {
    /// 优化后的模板（版本号已递增）
    pub template: PromptTemplate,
    /// 使用的 LLM 模型
    pub used_model: String,
}

impl StrategyOptimizer {
    pub fn new(
        http_client: reqwest::Client,
        api_key: String,
        base_url: String,
        model: String,
    ) -> Self {
        Self {
            http_client,
            api_key,
            base_url,
            model,
        }
    }

    /// 优化策略 prompt。
    ///
    /// 将当前 prompt + 绩效指标发送给 LLM，要求 LLM 分析问题并输出改进版本。
    /// 返回的模板版本号已递增，来源标记为 AI 生成。
    pub async fn optimize(
        &self,
        current: &PromptTemplate,
        metrics: &StrategyMetrics,
    ) -> BotResult<OptimizationResult> {
        let system = build_optimization_system_prompt();
        let user = build_optimization_user_prompt(current, metrics);

        let result = call_llm_api(
            &self.http_client,
            &self.api_key,
            &self.base_url,
            &self.model,
            &system,
            &user,
            "strategy-optimizer",
        )
        .await?;

        let mut optimized: PromptTemplate =
            serde_json::from_value(result.content.clone()).map_err(|e| {
                BotError::Llm(format!("LLM 返回的 JSON 无法解析为 PromptTemplate: {e}"))
            })?;

        // 保留原始名称和策略类型
        optimized.name = current.name.clone();
        optimized.strategy_type = current.strategy_type;

        // 版本号递增
        optimized.version = current.version + 1;

        // 标记来源
        optimized.source = PromptSource::AiGenerated {
            model: result.used_model.clone(),
            generation_prompt: format!(
                "策略优化：基于 {} 笔交易（胜率 {:.1}%，P&L {:.2} USDT）",
                metrics.total_trades,
                metrics.win_rate * 100.0,
                metrics.total_pnl
            ),
        };

        // 校验
        validate(&optimized).map_err(|e| {
            BotError::Llm(format!("优化后的策略 prompt 校验失败: {e}"))
        })?;

        Ok(OptimizationResult {
            template: optimized,
            used_model: result.used_model,
        })
    }
}

/// 构建优化 system prompt。
fn build_optimization_system_prompt() -> String {
    let placeholder_text = crate::placeholder::to_prompt_text();

    format!(
        r#"你是一个策略 prompt 优化器。你的任务是分析一个交易策略的绩效数据，找出问题，并输出改进后的策略 prompt。

你必须返回一个 JSON 对象，包含以下字段：
{{
  "name": "策略名（保持不变）",
  "strategy_type": "auto",
  "system_prompt": "改进后的 system prompt。定义角色、交易规则、输出 JSON 格式约束。必须包含 'JSON' 字样。",
  "user_prompt_template": "改进后的用户 prompt 模板，使用 {{placeholder}} 占位符",
  "required_placeholders": ["占位符列表"],
  "description": "改进说明（中文）"
}}

可用占位符白名单（只能使用以下占位符）：
{placeholder_text}

优化原则：
1. 分析胜率、盈亏比、最大回撤，找出策略的弱点
2. 胜率低 → 改进入场条件，增加过滤条件
3. 盈亏比低 → 改进止损止盈逻辑
4. 最大回撤大 → 增加风控规则，降低仓位
5. 保持原始策略的核心逻辑，只做针对性改进
6. 不要删除占位符，只调整 prompt 中的规则描述

只返回 JSON 对象，不要包含其他文字。"#
    )
}

/// 构建优化 user prompt：包含当前 prompt 和绩效数据。
fn build_optimization_user_prompt(
    current: &PromptTemplate,
    metrics: &StrategyMetrics,
) -> String {
    format!(
        r#"请优化以下策略 prompt：

策略名称：{name}
当前版本：v{version}

=== 当前 system_prompt ===
{system_prompt}

=== 当前 user_prompt_template ===
{user_prompt_template}

=== 绩效指标（最近 {days} 天）===
- 总交易笔数：{total_trades}
- 胜率：{win_rate:.1}%（{winning}胜 / {losing}负）
- 累计盈亏：{total_pnl:.2} USDT
- 平均每笔盈亏：{avg_trade_pnl:.2} USDT
- 盈亏比：{profit_factor:.2}
- 最大回撤：{max_drawdown:.2} USDT
- 平均持仓时长：{avg_holding_mins:.0} 分钟
- 综合评分：{score:.3}（满分 1.0）

请分析问题并输出改进后的策略 prompt JSON。"#,
        name = current.name,
        version = current.version,
        system_prompt = current.system_prompt,
        user_prompt_template = current.user_prompt_template,
        days = (metrics.window_end - metrics.window_start).num_days(),
        total_trades = metrics.total_trades,
        win_rate = metrics.win_rate * 100.0,
        winning = metrics.winning_trades,
        losing = metrics.losing_trades,
        total_pnl = metrics.total_pnl,
        avg_trade_pnl = metrics.avg_trade_pnl,
        profit_factor = if metrics.profit_factor.is_infinite() {
            999.99
        } else {
            metrics.profit_factor
        },
        max_drawdown = metrics.max_drawdown,
        avg_holding_mins = metrics.avg_holding_secs / 60.0,
        score = metrics.composite_score(),
    )
}
