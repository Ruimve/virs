//! Prompt 模板校验器。
//!
//! 校验维度：
//! 1. 占位符白名单：模板内所有 `{xxx}` 必须在白名单内（防止 AI 生成未知占位符）
//! 2. `required_placeholders` 与模板内实际使用的占位符一致
//! 3. system_prompt 非空且包含 JSON schema 约束（防止 AI 生成无格式约束的 prompt）
//! 4. user_prompt_template 非空
//!
//! 白名单来源：`crate::placeholder::registry::names()`，不再硬编码。

use std::collections::HashSet;

use virs_error::BotError;

use crate::placeholder;
use crate::prompt::template::PromptTemplate;

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
    // system_prompt 必须约束 LLM 的输出格式（auto 默认 prompt 含 "JSON" 字样）
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

    let known = placeholder::names();
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
