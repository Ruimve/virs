

use virs_error::{BotError, BotResult};
use virs_llm::call_llm_api;
use virs_prompt::{PromptSource, PromptTemplate, validate, to_prompt_text};
use virs_type::StrategyType;


pub struct GenerateRequest<'a> {

    pub strategy_type: StrategyType,

    pub user_intent: &'a str,

    pub name_hint: Option<&'a str>,
}


pub struct GenerateResult {
    pub template: PromptTemplate,
    pub used_model: String,
}


pub async fn generate_prompt(
    http_client: &reqwest::Client,
    api_key: &str,
    base_url: &str,
    model: &str,
    req: GenerateRequest<'_>,
) -> BotResult<GenerateResult> {
    let system = build_meta_system_prompt(req.strategy_type);
    let user = build_meta_user_prompt(&req);

    let result = call_llm_api(
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


    tpl.strategy_type = req.strategy_type;


    if let Some(name) = req.name_hint {
        if !name.is_empty() {
            tpl.name = name.to_string();
        }
    }


    tpl.source = PromptSource::AiGenerated {
        model: model.to_string(),
        generation_prompt: req.user_intent.to_string(),
    };


    validate(&tpl).map_err(|e| {
        BotError::Llm(format!("AI 生成的策略 prompt 校验失败: {e}"))
    })?;

    Ok(GenerateResult {
        template: tpl,
        used_model: result.used_model,
    })
}


pub(crate) fn build_meta_system_prompt(strategy_type: StrategyType) -> String {
    let strategy_desc = match strategy_type {
        StrategyType::Auto => {
            "Auto 趋势策略（单仓位方向判断：open_long/open_short/close_position/hold）"
        }
    };
    let placeholder_text = to_prompt_text();

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
{placeholder_text}

system_prompt 要求：
1. 定义 LLM 角色（如"你是趋势跟随交易引擎"）
2. 明确交易规则（入场条件、出场条件、风控规则）
3. 必须规定输出 JSON 格式（包含 decision.action / decision.reason / decision.confidence 等字段）
4. action 可选值：open_long, open_short, close_position, hold

user_prompt_template 要求：
1. 用 {{placeholder}} 引用指标值，不得硬编码数值
2. 包含账户余额、仓位信息、多周期指标
3. 结构清晰，分段落展示

只返回 JSON 对象，不要包含其他文字。"#,
        strategy_type_str = match strategy_type {
            StrategyType::Auto => "auto",
        }
    )
}


pub(crate) fn build_meta_user_prompt(req: &GenerateRequest<'_>) -> String {
    let name_hint = req.name_hint.unwrap_or("（由你命名）");
    format!(
        r#"请根据以下意图生成策略 prompt：

策略类型：{strategy_type}
策略名：{name_hint}
用户意图：{user_intent}

请生成完整的策略 prompt JSON。"#,
        strategy_type = match req.strategy_type {
            StrategyType::Auto => "Auto 趋势策略",
        },
        name_hint = name_hint,
        user_intent = req.user_intent,
    )
}
