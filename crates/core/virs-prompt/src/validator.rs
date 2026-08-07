

use std::collections::HashSet;

use virs_error::BotError;

use crate::placeholder;
use crate::template::PromptTemplate;


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

    if !tpl.system_prompt.contains("JSON") && !tpl.system_prompt.contains("json") {
        return Err(BotError::Validation(
            "system_prompt 必须包含 JSON 输出格式约束（未找到 'JSON' 字样）".to_string(),
        ));
    }

    if tpl
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {

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


    for ph in &used {
        if !known.contains(ph.as_str()) {
            return Err(BotError::Validation(format!(
                "模板内出现未知占位符: {ph}"
            )));
        }
    }


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


pub fn extract_placeholders(template: &str) -> HashSet<String> {
    let mut result = HashSet::new();
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {

            if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                i += 2;
                continue;
            }

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
