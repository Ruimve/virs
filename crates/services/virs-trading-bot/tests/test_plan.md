# virs-bot 测试用例文档

> Crate: `crates/services/virs-bot`
> 状态: **50 个测试全部通过** (41 单元 + 9 集成)

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

### 1.2 ai_tests.rs — 自动交易 AI 解析 (9)

| ID | 测试函数 | 描述 |
|----|---------|------|
| A1.1 | `a1_1_action_from_str_open_long` | "open_long" → OpenLong |
| A1.2 | `a1_2_action_from_str_open_short` | "open_short" → OpenShort |
| A1.3 | `a1_3_action_from_str_close` | "close_position" → ClosePosition |
| A1.4 | `a1_4_action_from_str_hold` | "hold" → Hold |
| A1.5 | `a1_5_action_from_str_unknown` | "unknown" → Hold |
| A2.1 | `a2_1_action_as_str_all_variants` | 所有变体 → 正确字符串 |
| A3.1 | `a3_1_decision_from_json_complete` | 完整 JSON → 所有字段正确 |
| A3.2 | `a3_2_decision_from_json_missing_fields_returns_error` | 缺失字段 → 返回 error |
| A3.4 | `a3_4_decision_from_json_confidence_clamped` | confidence=1.5 → 1.0 |

### 1.3 worker_tests.rs — 持仓时间 Instant 恢复 (6)

| ID | 测试函数 | 描述 |
|----|---------|------|
| T11.1 | `t11_1_restored_instant_reflects_actual_elapsed_time` | 2h 持仓 → Instant elapsed ≈ 2h |
| T11.2 | `t11_2_restored_instant_for_recent_open` | 5s 持仓 → Instant elapsed ≈ 5s |
| T11.3 | `t11_3_restored_instant_near_max_position_duration` | 47h 持仓 → elapsed < 48h 上限 |
| T11.4 | `t11_4_restored_instant_exceeds_max_position_duration` | 49h 持仓 → elapsed > 48h 上限 |
| T11.5 | `t11_5_future_opened_at_clamped_to_zero` | 未来 opened_at → clamp 至 ~0 |
| T11.6 | `t11_6_checked_sub_returns_none_for_extreme_duration` | 极端 duration → checked_sub None |

---

## 2. 集成测试用例

### integration_tests.rs (8)

| ID | 测试函数 | 描述 |
|----|---------|------|
| INT-1.1 | `int_1_1_stop_loss_take_profit_consistency` | stop_loss < entry < take_profit (long) |
| INT-1.2 | `int_1_2_trailing_stop_never_worsens` | trailing stop 只能改善不能恶化 |
| INT-1.3 | `int_1_3_position_pct_full_chain` | ADX + losses + funding 级联衰减 |
| INT-2.1 | `int_2_1_auto_action_roundtrip` | from_str → as_str 一致性 |
| INT-2.2 | `int_2_2_auto_decision_json_roundtrip` | JSON → AutoDecision → 字段验证 |
| INT-4.2 | `int_4_2_format_stop_take_with_position_pct` | 风控链路: position_pct → format_stop_take |
| INT-5.1 | `int_5_1_cooldown_then_position_pct` | 冷却计算 → 仓位百分比链路 |
| INT-6.1 | `int_6_1_format_bars_outside_all_cases` | format_bars_outside 三种情况 |

### strategy_seed_files.rs (1)

| ID | 测试函数 | 描述 |
|----|---------|------|
| SEED-1 | `seed_auto_default_loads_and_validates` | seed 文件加载并通过校验 (`#[tokio::test]`) |

---

## 3. 代码覆盖率

| 测试文件 | 被测模块 | 测试数 |
|----------|----------|--------|
| `src/auto/strategy_tests.rs` | auto/strategy.rs | 26 |
| `src/auto/ai_tests.rs` | auto/ai.rs | 9 |
| `src/auto/worker_tests.rs` | auto/worker.rs | 6 |
| `tests/integration_tests.rs` | 跨模块链路 | 8 |
| `tests/strategy_seed_files.rs` | strategies seed 文件加载 | 1 |
| **合计** | | **50** |
