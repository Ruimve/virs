# virs-tactical-bot - 测试计划

**测试总数:** 8（单元测试: 8，集成测试: 0）

---

## 单元测试

### `crates/services/virs-tactical-bot/src/engine/evaluator_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `t_eval_1_basic_metrics` | 4 笔交易的基本指标计算 | PnL: +100, -50, +80, -30 | total_trades=4, winning_trades=2, losing_trades=2, win_rate=0.5, total_pnl=100.0 |
| 2 | `t_eval_2_profit_factor` | 盈亏比计算 | PnL: +100, -50 | profit_factor=2.0 |
| 3 | `t_eval_3_no_trades_returns_none` | 无交易记录返回 None | 空交易列表，策略名 `nonexistent` | evaluate 返回 None |
| 4 | `t_eval_4_max_drawdown` | 最大回撤计算 | PnL: +100, +50, -200, +30 | max_drawdown=200.0 |
| 5 | `t_eval_5_composite_score` | 综合评分计算 | win_rate=0.6, total_pnl=200.0, max_drawdown=50.0, profit_factor=2.0 | score 在 (0, 1] 范围内 |
| 6 | `t_eval_6_needs_optimization` | 策略是否需要优化 | good: win_rate=0.7, total_pnl=500.0；bad: win_rate=0.2, total_pnl=-300.0 | good 不需要优化，bad 需要优化 |

### `crates/services/virs-tactical-bot/src/prompt/ai_generator_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `g1_meta_system_prompt_mentions_json_and_actions` | 系统提示词提及 JSON 和动作值 | StrategyType::Auto | 包含 `JSON` 和 `open_long` |
| 2 | `g3_meta_user_prompt_contains_intent` | 用户提示词包含意图和名称提示 | user_intent=`做多趋势策略`，name_hint=`my_trend` | 包含 `做多趋势策略` 和 `my_trend` |
