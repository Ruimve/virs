# virs-trading-bot - 测试计划

**测试总数:** 72（单元测试: 63，集成测试: 9）

---

## 单元测试

### `crates/services/virs-trading-bot/src/auto/worker/side_state_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `ss_1_1_has_position_open_with_quantity` | 持仓状态为 Open 且数量为 1.5 时判定有持仓 | Position(Long, Open, qty=1.5) | has_position() == true |
| 2 | `ss_1_2_has_position_open_zero_quantity` | 持仓数量为 0 时判定无持仓 | Position(Long, Open, qty=0.0) | has_position() == false |
| 3 | `ss_1_3_has_position_open_near_zero_quantity` | 持仓数量低于 epsilon 时判定无持仓 | Position(Short, Open, qty=1e-10) | has_position() == false |
| 4 | `ss_1_4_has_position_opening_status` | 持仓状态为 Opening 时判定无持仓 | Position(Long, Opening, qty=1.0) | has_position() == false |
| 5 | `ss_1_5_has_position_closing_status` | 持仓状态为 Closing 时判定无持仓 | Position(Long, Closing, qty=1.0) | has_position() == false |
| 6 | `ss_1_6_has_position_none` | 无持仓时判定无持仓 | position=None | has_position() == false |
| 7 | `ss_1_7_has_position_closed_status` | 持仓状态为 Closed 时判定无持仓 | Position(Long, Closed, qty=1.0) | has_position() == false |
| 8 | `ss_2_1_is_pending_open_only` | 仅有 pending_open 时判定为 pending | pending_open=Some | is_pending() == true |
| 9 | `ss_2_2_is_pending_close_only` | 仅有 pending_close 时判定为 pending | pending_close=Some | is_pending() == true |
| 10 | `ss_2_3_is_pending_both` | 同时有 pending_open 和 pending_close 时判定为 pending | 两者均为 Some | is_pending() == true |
| 11 | `ss_2_4_is_pending_none` | 无 pending 时判定非 pending | 两者均为 None | is_pending() == false |
| 12 | `ss_3_1_get_position_some` | 有持仓时 get_position 返回 Some | Position(Long, Open, qty=2.0) | 返回 Some, id 与 quantity 匹配 |
| 13 | `ss_3_2_get_position_none` | 无持仓时 get_position 返回 None | position=None | 返回 None |
| 14 | `ss_4_1_clear_position_resets_all_position_fields` | clear_position 重置所有持仓字段 | 设置 position/stop_loss=97/take_profit=106/opened_at/order_id/open_fee=0.5 | position=None, stop_loss=0.0, take_profit=0.0, opened_at=None, order_id=None, open_fee=0.0 |
| 15 | `ss_4_2_clear_position_preserves_log_id` | clear_position 保留 log_id | 设置 log_id 与持仓 | log_id 保持不变 |
| 16 | `ss_4_3_clear_position_preserves_last_close_event` | clear_position 保留 last_close_event | 设置 last_close_event=(long, stop_loss) 与持仓 | last_close_event 保持不变 |
| 17 | `ss_4_4_clear_position_on_default_is_noop` | 默认状态调用 clear_position 无副作用 | SideState::default() | position=None, stop_loss=0.0 |
| 18 | `ss_5_1_clear_on_close_resets_position_fields` | clear_on_close 重置持仓字段 | 设置持仓等字段 + take_profit 事件 | position=None, stop_loss=0.0, take_profit=0.0, opened_at=None, order_id=None, open_fee=0.0 |
| 19 | `ss_5_2_clear_on_close_clears_log_id` | clear_on_close 清除 log_id | 设置 log_id 与持仓 + stop_loss 事件 | log_id == None |
| 20 | `ss_5_3_clear_on_close_sets_last_close_event` | clear_on_close 设置 last_close_event | 设置持仓 + short/llm_decision 事件 | last_close_event == 该事件 |
| 21 | `ss_5_4_clear_on_close_overwrites_previous_last_close_event` | clear_on_close 覆盖旧的 last_close_event | 已有旧事件(long, stop_loss) + 新事件(long, take_profit) | last_close_event == 新事件 |
| 22 | `ss_6_1_default_all_fields_zero_or_none` | 默认 SideState 所有字段为零或 None | SideState::default() | 所有字段为 None 或 0.0 |

### `crates/services/virs-trading-bot/src/auto/ai_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `a1_1_action_from_str_open_long` | 字符串 "open_long" 解析为 OpenLong | "open_long" | AutoAction::OpenLong |
| 2 | `a1_2_action_from_str_open_short` | 字符串 "open_short" 解析为 OpenShort | "open_short" | AutoAction::OpenShort |
| 3 | `a1_3_action_from_str_close` | 字符串 "close_position" 解析为 ClosePosition | "close_position" | AutoAction::ClosePosition |
| 4 | `a1_4_action_from_str_hold` | 字符串 "hold" 解析为 Hold | "hold" | AutoAction::Hold |
| 5 | `a1_5_action_from_str_unknown` | 未知字符串解析为 Hold | "unknown" | AutoAction::Hold |
| 6 | `a2_1_action_as_str_all_variants` | 所有动作变体的 as_str 转换正确 | 全部 4 个变体 | 分别返回 "open_long"/"open_short"/"close_position"/"hold" |
| 7 | `a3_1_decision_from_json_complete` | 完整 JSON 解析为 AutoDecision | 含 decision/market/analysis/risk_warning, action=open_long, confidence=0.85 | action=OpenLong, reason="EMA golden cross", confidence=0.85, market_regime=Some("trending_up") |
| 8 | `a3_2_decision_from_json_missing_fields_returns_error` | 缺失字段的 JSON 返回错误 | 空 JSON {} | 返回错误 |
| 9 | `a3_4_decision_from_json_confidence_clamped` | confidence 超过 1.0 时被截断为 1.0 | action=hold, confidence=1.5 | confidence == 1.0 |

### `crates/services/virs-trading-bot/src/auto/strategy_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `s1_1_stop_loss_long` | 多头止损价计算 | entry=100, "long", atr=2.0 | result == 97.0 |
| 2 | `s1_2_stop_loss_short` | 空头止损价计算 | entry=100, "short", atr=2.0 | result == 103.0 |
| 3 | `s1_3_stop_loss_zero_atr` | ATR 为 0 时多头止损价 | entry=100, "long", atr=0.0 | result == 97.0 |
| 4 | `s1_4_stop_loss_zero_entry` | 入场价为 0 时多头止损价 | entry=0, "long", atr=2.0 | result == 0.0 |
| 5 | `s1_5_take_profit_long` | 多头止盈价计算 | entry=100, "long", atr=2.0 | result == 106.0 |
| 6 | `s1_6_take_profit_short` | 空头止盈价计算 | entry=100, "short", atr=2.0 | result == 94.0 |
| 7 | `s1_7_take_profit_zero_atr` | ATR 为 0 时多头止盈价 | entry=100, "long", atr=0.0 | result == 106.0 |
| 8 | `s2_1_trailing_long_profit_2atr` | 多头盈利 2ATR 时移动止损上移 | entry=100, price=105, "long", atr=2.0, current_stop=97 | result == 103.0 |
| 9 | `s2_2_trailing_long_profit_1atr` | 多头盈利 1ATR 时移动止损上移 | entry=100, price=102, "long", atr=2.0, current_stop=97 | result == 100.0 |
| 10 | `s2_3_trailing_short_profit_2atr` | 空头盈利 2ATR 时移动止损下移 | entry=100, price=95, "short", atr=2.0, current_stop=103 | result == 97.0 |
| 11 | `s2_4_trailing_no_profit` | 无盈利时移动止损保持原值 | entry=100, price=101, "long", atr=2.0, current_stop=97 | result == 97.0 |
| 12 | `s2_5_trailing_long_never_worsens` | 移动止损不劣化（不低于当前止损） | entry=100, price=105, "long", atr=2.0, current_stop=105 | result == 105.0 |
| 13 | `s3_1_position_pct_high_adx` | 高 ADX 仓位百分比 | adx=25, 连续亏损=0, funding=0.0 | result == 80.0 |
| 14 | `s3_2_position_pct_medium_adx` | 中 ADX 仓位百分比 | adx=20, 连续亏损=0, funding=0.0 | result == 60.0 |
| 15 | `s3_3_position_pct_low_adx` | 低 ADX 仓位百分比 | adx=15, 连续亏损=0, funding=0.0 | result == 40.0 |
| 16 | `s3_4_position_pct_consecutive_losses` | 连续亏损降低仓位 | adx=25, 连续亏损=2, funding=0.0 | result == 40.0 |
| 17 | `s3_5_position_pct_high_funding` | 高资金费率降低仓位 | adx=25, 连续亏损=0, funding=0.002 | result == 40.0 |
| 18 | `s3_6_position_pct_clamp_min` | 仓位百分比下限为 10 | adx=15, 连续亏损=2, funding=0.002 | result == 10.0 |
| 19 | `s4_1_format_stop_take_both` | 同时设置止损止盈时格式化输出 | sl=95.0, tp=110.0 | 含 "止损" 和 "止盈" |
| 20 | `s4_2_format_stop_take_none` | 均未设置时格式化输出 | sl=0.0, tp=0.0 | "未设置" |
| 21 | `s4_3_format_stop_take_only_sl` | 仅设置止损时格式化输出 | sl=95.0, tp=0.0 | 含 "止损", 不含 "止盈" |
| 22 | `s5_1_cooldown_stop_loss_same_side` | 止损平仓同侧冷却时间 | "long", "stop_loss", "long" | 1800 |
| 23 | `s5_2_cooldown_stop_loss_diff_side` | 止损平仓异侧无冷却 | "long", "stop_loss", "short" | 0 |
| 24 | `s5_3_cooldown_take_profit_same_side` | 止盈平仓同侧冷却时间 | "long", "take_profit", "long" | 900 |
| 25 | `s5_4_cooldown_llm_decision` | LLM 决策平仓冷却时间 | "long", "llm_decision", "long" | 900 |
| 26 | `s5_5_cooldown_unknown_reason` | 未知平仓原因的冷却时间 | "long", "unknown", "short" | 900 |

### `crates/services/virs-trading-bot/src/auto/worker/worker_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `t11_1_restored_instant_reflects_actual_elapsed_time` | 恢复的 Instant 反映实际经过时间（2 小时） | opened_at = 当前 - 2 小时 | restored 为 Some, elapsed 接近 2h（误差 < 10s） |
| 2 | `t11_2_restored_instant_for_recent_open` | 近期开仓恢复的 Instant（5 秒） | opened_at = 当前 - 5 秒 | restored 为 Some, elapsed 接近 5s（误差 < 5s） |
| 3 | `t11_3_restored_instant_near_max_position_duration` | 接近最大持仓时长的 Instant（47 小时） | opened_at = 当前 - 47 小时 | elapsed < 48h, 剩余时间 < 1h |
| 4 | `t11_4_restored_instant_exceeds_max_position_duration` | 超过最大持仓时长的 Instant（49 小时） | opened_at = 当前 - 49 小时 | elapsed > 48h |
| 5 | `t11_5_future_opened_at_clamped_to_zero` | 未来时间被截断为零 | opened_at = 当前 + 1 小时 | elapsed < 5s |
| 6 | `t11_6_checked_sub_returns_none_for_extreme_duration` | 极端时长时 checked_sub 返回 None 不 panic | extreme_secs = i64::MAX | 验证不 panic |

## 集成测试

### `crates/services/virs-trading-bot/tests/strategy_seed_files_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `seed_auto_default_loads_and_validates` | 加载并验证 auto/default 种子策略文件 | STRATEGIES_DIR（默认 ../../strategies） | loader 非空, strategy_type=Auto, system_prompt 非空, placeholders 非空 |

### `crates/services/virs-trading-bot/tests/integration_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `int_1_1_stop_loss_take_profit_consistency` | 止损止盈一致性（多头） | entry=100, atr=2.0 | sl < entry < tp, sl < tp |
| 2 | `int_1_2_trailing_stop_never_worsens` | 移动止损不劣化 | entry=100, atr=2.0, 先 price=105 再 price=103 | new_stop_2 >= new_stop_1 >= initial_stop |
| 3 | `int_1_3_position_pct_full_chain` | 仓位百分比全链路 | adx=30,losses=2,funding=0.003; 再 adx=30,losses=0,funding=0.0 | 依次为 20.0 与 80.0 |
| 4 | `int_2_1_auto_action_roundtrip` | AutoAction 字符串往返转换 | 5 个动作含 unknown_action | 未知动作映射为 Hold, 其余往返一致 |
| 5 | `int_2_2_auto_decision_json_roundtrip` | AutoDecision JSON 往返解析 | 含 decision.action=open_long, market.market_regime | action=OpenLong, as_str=="open_long" |
| 6 | `int_4_2_format_stop_take_with_position_pct` | 仓位百分比与止损止盈格式化联动 | adx=25 -> 80%; entry=100, atr=2.0 | pct=80, 显示含 "止损" 和 "止盈" |
| 7 | `int_5_1_cooldown_then_position_pct` | 冷却后仓位百分比计算 | stop_loss 同侧 -> 1800; adx=25 无亏损 -> 80; adx=25 亏损 2 -> 40 | 依次为 1800, 80, 40 |
| 8 | `int_6_1_format_bars_outside_all_cases` | bars_outside 格式化全场景 | 5, -3, 0 | 依次为 "向上5根", "向下3根", "无" |
