//! Prompt 模板校验器。
//!
//! 校验维度：
//! 1. 占位符白名单：模板内所有 `{xxx}` 必须在白名单内（防止 AI 生成未知占位符）
//! 2. `required_placeholders` 与模板内实际使用的占位符一致
//! 3. system_prompt 非空且包含 JSON schema 约束（防止 AI 生成无格式约束的 prompt）
//! 4. user_prompt_template 非空
//!
//! 白名单维护：新增占位符时，同步更新 [`KNOWN_PLACEHOLDERS`] 和
//! [`placeholder::placeholder_to_indicator`]（如该占位符对应指标）。

use std::collections::HashSet;

use virs_error::BotError;

use crate::prompt::template::{PromptSource, PromptTemplate, StrategyType};

/// 占位符白名单。覆盖 auto + grid 两个默认 prompt 中出现的全部占位符。
///
/// 新增占位符必须在此注册，否则校验失败。
pub const KNOWN_PLACEHOLDERS: &[&str] = &[
    // ── 通用上下文（非指标）──
    "timestamp",
    "total_balance",
    "available_balance",
    "used_margin",
    "margin_usage_rate",
    "symbol",
    "exchange",
    "leverage",
    "min_qty",
    "position_info",
    "position_duration",
    "stop_take_profit_info",
    "recent_close_info",
    "funding_rate",
    "funding_next_time",
    "total_trades",
    "win_trades",
    "loss_trades",
    "total_pnl",
    "consecutive_losses",
    "trigger_reason",
    "h1_ema_cross",
    "h1_change",
    "h1_bb_width_pct",
    "h1_current_price",
    "h1_volume",
    "h1_volume_sma20",
    // ── Auto grid 共享的指标占位符 ──
    "h4_ema20",
    "h4_ema50",
    "h4_rsi",
    "h4_macd_histogram",
    "h4_adx",
    "h4_macd",
    "h4_macd_signal",
    "h4_bb_width_pct",
    "h1_ema20",
    "h1_ema50",
    "h1_ema_cross_bars_ago",
    "h1_ema_gap_pct",
    "h1_ema_gap_trend",
    "h1_rsi",
    "h1_macd",
    "h1_macd_signal",
    "h1_macd_histogram",
    "h1_adx",
    "h1_atr",
    "h1_bb_upper",
    "h1_bb_middle",
    "h1_bb_lower",
    "h1_high_50",
    "h1_low_50",
    "m15_current_price",
    "m15_ema20",
    "m15_ema50",
    "m15_ema_cross",
    "m15_ema_cross_bars_ago",
    "m15_rsi",
    "m15_macd",
    "m15_macd_signal",
    "m15_macd_histogram",
    "m15_atr",
    "m15_adx",
    "m15_volume",
    "m15_volume_sma20",
    "m15_high_50",
    "m15_low_50",
    // ── Grid 专属 ──
    "grid_status",
    "last_adjust_time",
    "current_grid_config",
    "h1_atr_sma20",
    "h1_candle_body",
    "h1_bars_outside_band",
    "h1_bandwidth_5bars_ago",
    "h1_high_20",
    "h1_low_20",
    "nearest_round_up",
    "nearest_round_down",
    "m15_bb_width_pct",
    "m15_atr_sma20",
    "m15_bars_outside_band",
    "event_flag",
    "event_description",
];

/// 校验单个模板。
pub fn validate(tpl: &PromptTemplate) -> Result<(), BotError> {
    if tpl.system_prompt.trim().is_empty() {
        return Err(BotError::Validation(
            "system_prompt 不能为空".to_string(),
        ));
    }
    if tpl.user_prompt_template.trim().is_empty() {
        return Err(BotError::Validation(
            "user_prompt_template 不能为空".to_string(),
        ));
    }
    // system_prompt 必须约束 LLM 的输出格式（auto/grid 默认 prompt 均含 "JSON" 字样）
    if !tpl.system_prompt.contains("JSON") && !tpl.system_prompt.contains("json") {
        return Err(BotError::Validation(
            "system_prompt 必须包含 JSON 输出格式约束（未找到 'JSON' 字样）".to_string(),
        ));
    }
    // name 合法性
    if tpl
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        // ok
    } else {
        return Err(BotError::Validation(
            "name 不能为空且只能包含字母数字/下划线/连字符".to_string(),
        ));
    }
    if tpl.name.is_empty() {
        return Err(BotError::Validation(
            "name 不能为空且只能包含字母数字/下划线/连字符".to_string(),
        ));
    }

    let known: HashSet<&str> = KNOWN_PLACEHOLDERS.iter().copied().collect();
    let used = extract_placeholders(&tpl.user_prompt_template);

    // 白名单校验
    for ph in &used {
        if !known.contains(ph.as_str()) {
            return Err(BotError::Validation(format!(
                "模板内出现未知占位符: {ph}"
            )));
        }
    }

    // required_placeholders 与实际使用一致
    let declared: HashSet<&str> = tpl.required_placeholders.iter().map(|s| s.as_str()).collect();
    for ph in &used {
        if !declared.contains(ph.as_str()) {
            return Err(BotError::Validation(format!(
                "user_prompt_template 使用了 '{ph}'，但未在 required_placeholders 中声明"
            )));
        }
    }
    for ph in &tpl.required_placeholders {
        if !used.contains(ph) {
            return Err(BotError::Validation(format!(
                "required_placeholders 声明了 '{ph}'，但 user_prompt_template 中未使用"
            )));
        }
    }

    Ok(())
}

/// 从模板字符串中提取 `{xxx}` 占位符。忽略 `{{` 转义。
///
/// 占位符必须为合法标识符：以 ASCII 字母/下划线开头，后接字母/数字/下划线。
/// 其他形式（如 `{中文}`、`{a b}`）不会被识别为占位符。
pub fn extract_placeholders(template: &str) -> HashSet<String> {
    let mut result = HashSet::new();
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            // 转义 {{
            if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                i += 2;
                continue;
            }
            // 找到闭合 }
            if let Some(end) = template[i + 1..].find('}') {
                let ph = &template[i + 1..i + 1 + end];
                if is_valid_placeholder(ph) {
                    result.insert(ph.to_string());
                }
                i += end + 2;
                continue;
            }
        }
        i += 1;
    }
    result
}

/// 判断字符串是否为合法占位符名：`[A-Za-z_][A-Za-z0-9_]*`
fn is_valid_placeholder(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 判断是否为 AI 生成模板。
pub fn is_ai_generated(tpl: &PromptTemplate) -> bool {
    matches!(tpl.source, PromptSource::AiGenerated { .. })
}

/// 按策略类型返回该类型允许的占位符白名单子集。
///
/// 当前实现：auto 与 grid 共享同一白名单（指标占位符通用）。
/// 若未来需要严格隔离，可在此按 `strategy_type` 过滤。
pub fn allowed_placeholders(_strategy_type: StrategyType) -> &'static [&'static str] {
    KNOWN_PLACEHOLDERS
}
