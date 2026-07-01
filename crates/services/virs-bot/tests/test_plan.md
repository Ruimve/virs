# virs-bot 测试用例文档

> Crate: `crates/services/virs-bot`
> 状态: **66 个测试全部通过** (55 单元 + 11 集成)

---

## 1. 单元测试用例

### 1.1 strategy_tests.rs — 自动交易策略 (26)

| ID | 测试函数 | 描述 |
|----|---------|------|
| S1.1 | `s1_1_stop_loss_long` | long → entry - 1.5*atr |
| S1.2 | `s1_2_stop_loss_short` | short → entry + 1.5*atr |
| S1.3 | `s1_3_stop_loss_zero_atr` | atr=0 → entry*0.97 |
| S1.4 | `s1_4_stop_loss_zero_entry` | entry=0 → 0 |
| S1.5 | `s1_5_take_profit_long` | long → entry + 3.0*atr |
| S1.6 | `s1_6_take_profit_short` | short → entry - 3.0*atr |
| S1.7 | `s1_7_take_profit_zero_atr` | atr=0 → entry*1.06 |
| S2.1 | `s2_1_trailing_long_profit_2atr` | profit≥2atr → curr-1*atr |
| S2.2 | `s2_2_trailing_long_profit_1atr` | profit≥1atr → entry |
| S2.3 | `s2_3_trailing_short_profit_2atr` | short profit≥2atr → curr+1*atr |
| S2.4 | `s2_4_trailing_no_profit` | profit<1atr → current_stop |
| S2.5 | `s2_5_trailing_long_never_worsens` | new_stop<current_stop → 保持 |
| S3.1 | `s3_1_position_pct_high_adx` | adx≥25 → 80% |
| S3.2 | `s3_2_position_pct_medium_adx` | 20≤adx<25 → 60% |
| S3.3 | `s3_3_position_pct_low_adx` | adx<20 → 40% |
| S3.4 | `s3_4_position_pct_consecutive_losses` | losses≥2 → base*0.5 |
| S3.5 | `s3_5_position_pct_high_funding` | |funding|>0.001 → *0.5 |
| S3.6 | `s3_6_position_pct_clamp_min` | 40*0.5*0.5=10 (clamp) |
| S4.1 | `s4_1_format_stop_take_both` | sl>0, tp>0 → 两行 |
| S4.2 | `s4_2_format_stop_take_none` | sl=0, tp=0 → "未设置" |
| S4.3 | `s4_3_format_stop_take_only_sl` | sl>0, tp=0 → 仅止损 |
| S5.1 | `s5_1_cooldown_stop_loss_same_side` | stop_loss + same → 1800s |
| S5.2 | `s5_2_cooldown_stop_loss_diff_side` | stop_loss + diff → 0s |
| S5.3 | `s5_3_cooldown_take_profit_same_side` | take_profit + same → 900s |
| S5.4 | `s5_4_cooldown_llm_decision` | llm_decision → 900s |
| S5.5 | `s5_5_cooldown_unknown_reason` | unknown → 900s |

### 1.2 ai_tests.rs — 自动交易 AI 解析 (10)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A1.1 | `a1_1_action_from_str_open_long` | "open_long" → OpenLong |
| A1.2 | `a1_2_action_from_str_open_short` | "open_short" → OpenShort |
| A1.3 | `a1_3_action_from_str_close` | "close_position" → ClosePosition |
| A1.4 | `a1_4_action_from_str_hold` | "hold" → Hold |
| A1.5 | `a1_5_action_from_str_unknown` | "unknown" → Hold |
| A2.1 | `a2_1_action_as_str_all_variants` | 所有变体 → 正确字符串 |
| A3.1 | `a3_1_decision_from_json_complete` | 完整 JSON → 所有字段正确 |
| A3.2 | `a3_2_decision_from_json_missing_fields` | 缺失字段 → 默认值 |
| A3.3 | `a3_3_decision_from_json_sl_tp_zero` | sl=0/tp=0 → None |
| A3.4 | `a3_4_decision_from_json_confidence_clamped` | confidence=1.5 → 1.0 |

### 1.3 grid_ai_tests.rs — 网格 AI 解析 (8)

| ID | 测试函数 | 描述 |
|----|---------|------|
| G1.1 | `g1_1_action_from_str_adjust` | "adjust_grid" → AdjustGrid |
| G1.2 | `g1_2_action_from_str_pause` | "pause_grid" → PauseGrid |
| G1.3 | `g1_3_action_from_str_run` | "run_grid" → RunGrid |
| G1.4 | `g1_4_action_from_str_reduce` | "reduce_position" → ReducePosition |
| G1.5 | `g1_5_action_from_str_hold` | "unknown" → Hold |
| G1.6 | `g1_6_action_as_str_all_variants` | 所有变体 → 正确字符串 |
| G2.1 | `g2_1_parse_decision_complete` | 完整 JSON → 所有字段正确 |
| G2.2 | `g2_2_parse_decision_defaults` | 缺失字段 → 默认值 |

### 1.4 grid_utils_tests.rs — 网格层级计算 (6)

| ID | 测试函数 | 描述 |
|----|---------|------|
| U1.1 | `u1_1_calculate_levels_basic` | 标准参数 → 正确层级数和首层价格 |
| U1.2 | `u1_2_calculate_levels_zero_width` | upper=lower → 空列表 |
| U1.3 | `u1_3_calculate_levels_zero_count` | grid_count=0 → 空列表 |
| U1.4 | `u1_4_calculate_levels_side_assignment` | price<current → "buy", ≥current → "sell" |
| U1.5 | `u1_5_calculate_levels_sell_price` | sell_price = price * (1 + profit_pct/100) |
| U1.6 | `u1_6_calculate_levels_uses_current_price_for_qty` | quantity = qty_per_grid / current_price |

### 1.5 grid_types_tests.rs — 网格类型 (2)

| ID | 测试函数 | 描述 |
|----|---------|------|
| T1.1 | `t1_1_reset_for_relist_clears_state` | 运行时字段清零 |
| T1.2 | `t1_2_reset_for_relist_preserves_config` | price/side/quantity 保持不变 |

### 1.6 prompt_tests.rs — Prompt 格式化 (3)

| ID | 测试函数 | 描述 |
|----|---------|------|
| P1.1 | `p1_1_format_bars_positive` | count>0 → "向上N根" |
| P1.2 | `p1_2_format_bars_negative` | count<0 → "向下N根" |
| P1.3 | `p1_3_format_bars_zero` | count=0 → "无" |

---

## 2. 集成测试用例

### integration_tests.rs (11)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_stop_loss_take_profit_consistency` | stop_loss < entry < take_profit (long) |
| INT-1.2 | `int_1_2_trailing_stop_never_worsens` | trailing stop 只能改善不能恶化 |
| INT-1.3 | `int_1_3_position_pct_full_chain` | ADX + losses + funding 级联衰减 |
| INT-2.1 | `int_2_1_auto_action_roundtrip` | from_str → as_str 一致性 |
| INT-2.2 | `int_2_2_auto_decision_json_roundtrip` | JSON → AutoDecision → 字段验证 |
| INT-3.1 | `int_3_1_grid_action_roundtrip` | from_str → as_str 一致性 |
| INT-3.2 | `int_3_2_grid_decision_parse_chain` | JSON → parse → calculate_levels 链路 |
| INT-4.1 | `int_4_1_calculate_levels_then_reset` | calculate_levels → reset_for_relist 链路 |
| INT-4.2 | `int_4_2_format_stop_take_with_position_pct` | 风控链路: position_pct → format_stop_take |
| INT-5.1 | `int_5_1_cooldown_then_position_pct` | 冷却计算 → 仓位百分比链路 |
| INT-6.1 | `int_6_1_format_bars_outside_all_cases` | format_bars_outside 三种情况 |

---

## 3. 代码覆盖率

### 测试文件与模块映射

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/auto/strategy_tests.rs` | auto/strategy.rs | 26 |
| `src/auto/ai_tests.rs` | auto/ai.rs | 10 |
| `src/grid/ai_tests.rs` | grid/ai.rs | 8 |
| `src/grid/utils_tests.rs` | grid/utils/mod.rs | 6 |
| `src/grid/types_tests.rs` | grid/types.rs | 2 |
| `src/grid/utils/prompt_tests.rs` | grid/utils/prompt.rs | 3 |
| `tests/integration_tests.rs` | 跨模块链路 | 11 |
| **合计** | | **66** |

### 幂等函数业务调用验证

| 函数 | 模块 | 业务调用点 |
|------|------|-----------|
| `compute_stop_loss` | auto/strategy.rs | worker.rs:1279, 1703 |
| `compute_take_profit` | auto/strategy.rs | worker.rs:1280, 1704 |
| `compute_trailing_stop` | auto/strategy.rs | worker.rs:679 |
| `compute_position_pct` | auto/strategy.rs | worker.rs:1236 |
| `format_stop_take_profit` | auto/strategy.rs | worker.rs:867 |
| `format_position_info` | auto/strategy.rs | worker.rs:858 |
| `render_prompt` | auto/strategy.rs | worker.rs:945 |
| `compute_cooldown_secs` | auto/strategy.rs | worker.rs:163 |
| `AutoAction::from_str` | auto/ai.rs | ai.rs:99 |
| `AutoAction::as_str` | auto/ai.rs | worker.rs:794, 966, 977, 1087 |
| `AutoDecision::from_json` | auto/ai.rs | ai.rs:178 |
| `GridAction::from_str` | grid/ai.rs | worker.rs:1122 |
| `GridAction::as_str` | grid/ai.rs | worker.rs:1126, 1128 |
| `parse_grid_decision` | grid/ai.rs | ai.rs:102 |
| `calculate_levels` | grid/utils/mod.rs | worker.rs:54, 1435 |
| `reset_for_relist` | grid/types.rs | worker.rs:587 |
| `format_bars_outside` | grid/utils/prompt.rs | prompt.rs:73, 107 |
