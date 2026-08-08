# virs-prompt - 测试计划

**测试总数:** 25（单元测试: 25，集成测试: 0）

---

## 单元测试

### `crates/core/virs-prompt/src/validator_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `v1_valid_template_passes` | 有效模板校验通过 | name=test, system_prompt 非空, user_prompt 含 {h1_current_price}, required_placeholders=["h1_current_price"] | validate 返回 Ok |
| 2 | `v2_empty_system_prompt_rejected` | 空 system_prompt 被拒绝 | system_prompt="   "（空白） | 返回 BotError::Validation, 消息含 "system_prompt 不能为空" |
| 3 | `v4_unknown_placeholder_rejected` | 未知占位符被拒绝 | user_prompt="{unknown_field}", required_placeholders=["unknown_field"] | 返回 BotError::Validation, 消息含 "未知占位符" |
| 4 | `v5_declared_but_unused_rejected` | 声明但未使用的占位符被拒绝 | required_placeholders 额外添加 "h1_rsi"（未在模板中使用） | 返回 BotError::Validation, 消息含 "未使用" |
| 5 | `v6_used_but_not_declared_rejected` | 使用但未声明的占位符被拒绝 | user_prompt="{h1_current_price} {h1_rsi}", required_placeholders 仅含 h1_current_price | 返回 BotError::Validation, 消息含 "未在 required_placeholders 中声明" |
| 6 | `v7_invalid_name_rejected` | 非法 name 被拒绝 | name="test name!"（含空格和感叹号） | 返回 BotError::Validation, 消息含 "name" |
| 7 | `v8_escape_braces_not_extracted` | 转义花括号不被提取为占位符 | 文本 "{{not_a_placeholder}} {real}" | 仅提取出 "real"，长度=1 |

### `crates/core/virs-prompt/src/render_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `r1_replaces_account_placeholders` | 替换账户类占位符 | total_balance=10000.0, available_balance=5000.0, used_margin=3000.0, margin_usage_rate=30.0, leverage=10 | "10000.00 5000.00 3000.00 30.0 10" |
| 2 | `r2_replaces_symbol_placeholders` | 替换交易对类占位符 | symbol=BTCUSDT, exchange=binance, min_qty=0.001 | "BTCUSDT binance 0.001000" |
| 3 | `r3_replaces_funding_rate_as_percentage` | 资金费率按百分比替换 | funding_rate=0.0001, funding_next_time="2026-07-19 16:00:00" | "0.0100% 2026-07-19 16:00:00" |
| 4 | `r4_replaces_h1_indicators` | 替换 H1 周期指标占位符 | h1_current_price=50000.0, ema20=49500.0, ema50=49000.0, cross=金叉(多头), bars_ago=5 | "50000.00 49500.00 49000.00 金叉(多头) 5" |
| 5 | `r5_replaces_m15_indicators` | 替换 M15 周期指标占位符 | m15_current_price=50000.0, cross=金叉(多头), bars_ago=3 | "50000.00 金叉(多头) 3" |
| 6 | `r6_replaces_bars_outside_band` | 替换布林带外 K 线数占位符 | h1_bars_outside_band=2, m15_bars_outside_band=-1 | "向上2根 向下1根" |
| 7 | `r8_replaces_statistics` | 替换统计类占位符 | total_trades=50, win_trades=30, loss_trades=20, total_pnl=500.0, consecutive_losses=2 | "50 30 20 500.00 2" |
| 8 | `r9_no_op_for_absent_placeholders` | 无占位符时原样返回 | 模板="hello world" | "hello world" |
| 9 | `r10_ema_cross_bars_none_when_negative` | EMA 交叉 K 线数为负时显示无近期交叉 | h1/m15 的 ema_cross_bars_ago=-1 | "无近期交叉 无近期交叉" |

### `crates/core/virs-prompt/src/writer_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `w1_create_strategy_writes_folder` | 创建策略写入文件夹结构 | name=test_create, strategy=Auto, version=1 | 路径以 "test_create" 结尾, 含 meta.json 和 v1/ 子文件夹, v1/ 下含 system_prompt.md/user_prompt_template.md/required_placeholders.json/description.md, meta.version=1, 四个文件内容与模板一致 |
| 2 | `w2_create_strategy_rejects_existing` | 创建策略拒绝已存在的文件夹 | 先创建 test_dup, 再次调用 create_strategy | 第二次返回 Err |
| 3 | `w3_save_new_version_creates_v2` | 保存新版本创建 v2 文件夹 | 先 create_strategy(v1), 再 save_new_version(v2), v2 description 不同 | v1/ 和 v2/ 均存在, v2 的 system_prompt.md 和 description.md 为新内容, v1 的 system_prompt.md 和 description.md 不变, meta.version=2 |
| 4 | `w4_save_new_version_rejects_nonexistent_strategy` | 为不存在的策略保存新版本报错 | 策略文件夹不存在, 调用 save_new_version | 返回 Err |
| 5 | `w5_delete_strategy_removes_folder` | 删除策略移除文件夹 | 创建 test_del 后调用 delete_strategy | 删除后路径不存在; 再次删除返回 Err |
| 6 | `w6_create_without_env_var_errors` | 无环境变量创建报错 | 未设置 STRATEGIES_DIR 环境变量 | create_strategy 返回 Err |

### `crates/core/virs-prompt/src/loader_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `l1_loads_auto_strategies` | 加载 Auto 策略目录 | STRATEGIES_DIR 环境变量或默认 "../../strategies"（存在时） | loader 非空（加载到策略） |
| 2 | `l2_get_loaded_strategy` | 获取已加载的默认策略 | 加载后查询 StrategyType::Auto 的 "default" 模板 | 返回 Some（找到 default 策略） |
| 3 | `l3_nonexistent_dir_returns_empty` | 不存在目录返回空加载器 | 目录="/nonexistent/strategies" | loader.list(Auto).await.is_empty()=true |
