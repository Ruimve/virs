

use virs_error::{BotError, BotResult};
use virs_llm::call_llm_api;
use virs_prompt::{PromptTemplate, validate, to_prompt_text};
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


/* AI策略prompt生成：根据用户意图描述，调用LLM生成完整的策略prompt模板，并校验合法性 */
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
    let strategy_type_str = match strategy_type {
        StrategyType::Auto => "auto",
    };

    format!(
        r#"你是一个策略 prompt 生成器。你的任务是根据用户的意图描述，生成一个 {strategy_desc} 的策略 prompt。

## 返回格式

你必须返回一个 JSON 对象，且只返回 JSON，不要包含 markdown 代码块标记或任何其他文字。

JSON 对象必须包含且仅包含以下字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| name | string | 策略名，英文+下划线+连字符，如 trend_following |
| strategy_type | string | 固定为 "{strategy_type_str}" |
| system_prompt | string | 交易 LLM 的 system prompt，定义角色和交易规则 |
| user_prompt_template | string | 用户 prompt 模板，用 {{placeholder}} 引用指标 |
| required_placeholders | string[] | 占位符列表，必须与 user_prompt_template 中使用的 {{placeholder}} 完全一致 |
| description | string | 策略的中文描述 |

## 关键规则

### required_placeholders 一致性规则（最重要）

required_placeholders 数组必须与 user_prompt_template 中实际使用的 {{placeholder}} 完全一致：
- user_prompt_template 中每出现一个 {{placeholder}}，该占位符名称必须出现在 required_placeholders 中
- required_placeholders 中的每个占位符，必须在 user_prompt_template 中至少使用一次
- 不得遗漏、不得多余——系统会自动校验，不一致将被拒绝

### 占位符白名单

只能使用以下占位符，不得发明新的占位符：
{placeholder_text}

### system_prompt 规则

1. 定义 LLM 角色（如"你是趋势跟随交易引擎"）
2. 明确交易规则：入场条件、出场条件、风控规则
3. 不要包含 JSON 输出格式约束（由系统自动拼接）
4. action 可选值：open_long, open_short, close_position, hold

### user_prompt_template 规则

1. 用 {{placeholder}} 引用指标值，不得硬编码数值
2. 必须包含账户余额、仓位信息、多周期指标
3. 结构清晰，分段落展示
4. 所有 {{placeholder}} 必须来自上方白名单

## 示例

{{
  "name": "trend_following",
  "strategy_type": "{strategy_type_str}",
  "system_prompt": "你是趋势跟随交易引擎。规则：\n1. EMA20 上穿 EMA50 且价格在布林带中轨上方 → open_long\n2. EMA20 下穿 EMA50 且价格在布林带中轨下方 → open_short\n3. 达到 2% 止盈或 1% 止损 → close_position\n4. 不满足以上条件 → hold",
  "user_prompt_template": "账户余额：{{total_balance}} USDT\n可用余额：{{available_balance}} USDT\n杠杆：{{leverage}}x\n当前仓位方向：{{position_side}}\n当前持仓量：{{position_qty}}\n\nH1 指标：\n当前价格：{{h1_current_price}}\nEMA20：{{h1_ema20}}\nEMA50：{{h1_ema50}}\nEMA 交叉状态：{{h1_ema_cross_status}}\n布林带外 K 线数：{{h1_bars_outside_band}}\n\nM15 指标：\n当前价格：{{m15_current_price}}\nEMA 交叉状态：{{m15_ema_cross_status}}",
  "required_placeholders": ["total_balance", "available_balance", "leverage", "position_side", "position_qty", "h1_current_price", "h1_ema20", "h1_ema50", "h1_ema_cross_status", "h1_bars_outside_band", "m15_current_price", "m15_ema_cross_status"],
  "description": "基于 EMA 交叉和布林带的趋势跟随策略"
}}

只返回 JSON 对象。"#
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
