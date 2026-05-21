use crate::bot::semi_automatic_grid::ports::GridBotConfig;
use crate::bot::semi_automatic_grid::types::GridLevel;

/** 网格层级计算的核心参数 */
struct LevelParams {
    grid_spacing: f64,
    profit_factor: f64,
    mid_price: f64,
}

/** 从网格配置中提取层级计算参数

当参数无效时返回 None（grid_count <= 0, 价格 <= 0, 上界 <= 下界） */
fn extract_level_params(bot: &GridBotConfig) -> Option<LevelParams> {
    if bot.grid_count <= 0 || bot.upper_price <= 0.0 || bot.lower_price <= 0.0 || bot.upper_price <= bot.lower_price {
        return None;
    }
    Some(LevelParams {
        grid_spacing: (bot.upper_price - bot.lower_price) / bot.grid_count as f64,
        profit_factor: 1.0 + bot.grid_profit_pct / 100.0,
        mid_price: (bot.upper_price + bot.lower_price) / 2.0,
    })
}

/** 根据 LLM 配置或中间价格判定单层方向

优先使用 LLM 返回的 side，回退到价格与中间价比较 */
fn determine_level_side(
    level_index: i32,
    price: f64,
    mid_price: f64,
    llm_levels: &[serde_json::Value],
) -> String {
    let llm_level = llm_levels.iter().find(|v| v["level"].as_i64() == Some(level_index as i64));
    if let Some(l) = llm_level {
        return l["side"].as_str().unwrap_or("buy").to_string();
    }
    if price < mid_price { "buy".to_string() } else { "sell".to_string() }
}

/** 根据层级方向计算买入价和卖出价

buy 层: buy_price = price, sell_price = price * profit_factor
sell 层: buy_price = price / profit_factor, sell_price = price */
fn compute_buy_sell_prices(side: &str, price: f64, profit_factor: f64) -> (f64, f64) {
    if side == "buy" {
        (price, price * profit_factor)
    } else {
        (price / profit_factor, price)
    }
}

/** 根据价格和每格金额计算每层数量（币数） */
fn compute_quantity(price: f64, quantity_per_grid: f64) -> f64 {
    if price > 0.0 { quantity_per_grid / price } else { 0.0 }
}

/** 根据网格配置计算所有层级

返回 GridLevel 向量，每个层级包含价格、方向、买卖价、数量等信息
当参数无效时返回空向量 */
pub fn calculate_levels(bot: &GridBotConfig) -> Vec<GridLevel> {
    let params = match extract_level_params(bot) {
        Some(p) => p,
        None => return vec![],
    };

    let llm_levels: Vec<serde_json::Value> = bot.grid_levels_json
        .as_ref()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();

    (0..bot.grid_count)
        .map(|i| {
            let price = bot.lower_price + params.grid_spacing * (i as f64 + 0.5);
            let side = determine_level_side(i, price, params.mid_price, &llm_levels);
            let (buy_price, sell_price) = compute_buy_sell_prices(&side, price, params.profit_factor);
            let quantity = compute_quantity(price, bot.quantity_per_grid);

            GridLevel {
                level: i,
                price,
                side,
                buy_price,
                sell_price,
                quantity,
                buy_order_id: None,
                sell_order_id: None,
                buy_filled: false,
                sell_filled: false,
                hold_quantity: 0.0,
                avg_buy_price: 0.0,
                last_fill_price: None,
                trade_id: None,
            }
        })
        .collect()
}

/** 计算网格间距（用于 API 层构建层级时） */
pub fn compute_grid_spacing(upper_price: f64, lower_price: f64, grid_count: i32) -> f64 {
    if grid_count > 1 {
        (upper_price - lower_price) / grid_count as f64
    } else {
        0.0
    }
}

/** 计算利润因子 */
pub fn compute_profit_factor(grid_profit_pct: f64) -> f64 {
    1.0 + grid_profit_pct / 100.0
}

/** 计算中间价格 */
pub fn compute_mid_price(upper_price: f64, lower_price: f64) -> f64 {
    (upper_price + lower_price) / 2.0
}
