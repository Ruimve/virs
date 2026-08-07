# virs-type - 测试计划

**测试总数:** 43（单元测试: 38，集成测试: 5）

---

## 单元测试

### `crates/infra/virs-type/src/market/structs_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `m1_1_normal_total` | 正常余额计算总额 | Balance: asset=USDT, free=100.0, used=50.0, total=150.0 | compute_total()=150.0 |
| 2 | `m1_2_zero_total` | 零余额计算总额 | Balance: free=0.0, used=0.0, total=0.0 | compute_total()=0.0 |
| 3 | `s2_1_ticker_roundtrip` | Ticker 序列化反序列化往返 | symbol=BTC/USDT, bid=99.0, ask=101.0, last=100.0 | 反序列化后与原对象相等 |
| 4 | `s2_2_balance_roundtrip` | Balance 序列化反序列化往返 | asset=USDT, free=100.0, used=50.0, total=150.0 | 反序列化后与原对象相等 |
| 5 | `s2_3_exchange_position_roundtrip` | ExchangePosition 序列化反序列化往返 | symbol=BTC/USDT, side=Long, quantity=1.0, entry_price=50000.0, margin_mode=Cross | 反序列化后与原对象相等 |

### `crates/infra/virs-type/src/position/structs_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `p1_1_open_is_open` | 开仓状态判断 | status=Open 的仓位 | is_open()=true |
| 2 | `p6_1_long_pnl` | 多头未实现盈亏计算 | side=Long, entry_price=50000.0, quantity=1.0, 当前价=51000.0 | unrealized_pnl=1000.0 |
| 3 | `p6_2_short_pnl` | 空头未实现盈亏计算 | side=Short, entry_price=50000.0, quantity=1.0, 当前价=49000.0 | unrealized_pnl=1000.0 |
| 4 | `p2_1_open_first_fill` | 开仓首次成交 | 开仓中多头仓位, fill: 开仓方向, price=100.0, qty=1.0, pnl=0.0 | 未关闭, status=Open, quantity=1.0, entry_price=100.0, realized_pnl=0.0 |
| 5 | `p2_2_open_add_to_existing_weighted_avg` | 加仓使用加权平均价 | 已开仓 qty=1.0, entry=100.0, fill: price=120.0, qty=1.0 | quantity=2.0, entry_price=110.0 |
| 6 | `p2_3_open_zero_fill_price_does_not_update_entry` | 成交价为 0 时不更新开仓价 | 已开仓 qty=1.0, entry=100.0, fill: price=0.0, qty=0.5 | quantity=1.5, entry_price=100.0（不变） |
| 7 | `p2_4_close_partial_entry_unchanged` | 部分平仓时开仓价不变 | 已开仓 qty=2.0, entry=110.0, fill: 平仓, qty=0.5, pnl=50.0 | 未关闭, status=Open, quantity=1.5, entry_price=110.0, realized_pnl=50.0 |
| 8 | `p2_5_close_full_returns_closed` | 全部平仓返回已关闭 | 已开仓 qty=1.0, entry=100.0, fill: 平仓, qty=1.0, pnl=100.0 | is_closed=true, status=Closed, quantity=0.0, entry_price=100.0, realized_pnl=100.0 |
| 9 | `p2_6_close_with_zero_fill_price_still_updates` | 平仓成交价为 0 时仍更新盈亏 | 已开仓 qty=1.0, entry=100.0, fill: 平仓, price=0.0, qty=0.5, pnl=50.0 | 未关闭, quantity=0.5, realized_pnl=50.0 |
| 10 | `p2_7_open_after_partial_close_marginal_cost` | 部分平仓后加仓的边际成本 | 开仓 fill: price=100.0, qty=2.0; 平仓 qty=1.0, pnl=50.0; 加仓 fill: price=120.0, qty=1.0 | quantity=2.0, entry_price=110.0 |
| 11 | `p2_8_full_replay_sequence_matches_runtime` | 完整回放序列与运行时一致 | 开仓 qty=2.0@100; 平仓 qty=2.0, pnl=200.0; 重新开仓 qty=1.0@110 | 第一次平仓 is_closed=true, realized_pnl=200.0; 重开后 status=Open, quantity=1.0, entry_price=110.0 |
| 12 | `p3_1_new_for_replay_initial_state` | 回放新建仓位初始状态 | exchange=binance, symbol=BTC/USDT, side=Long, client_order_id=CL123, created_at=ts(5000) | status=Opening, quantity=0.0, entry_price=0.0, realized_pnl=0.0, client_order_id=CL123 |
| 13 | `p3_2_new_for_replay_then_apply_fill_matches_runtime` | 回放仓位应用成交后与运行时一致 | 回放仓位经 3 次成交: 开仓100@1, 加仓110@1, 平仓0.5@25 | status=Open, quantity=1.5, entry_price=105.0, realized_pnl=25.0 |
| 14 | `p4_1_uuid_deterministic_same_inputs` | 相同输入生成确定性 UUID | exchange=binance, symbol=BTC/USDT, side=Long（两次调用） | 两次生成的 UUID 相等 |
| 15 | `p4_2_uuid_differs_by_side` | 不同方向生成不同 UUID | Long vs Short（其余参数相同） | 两个 UUID 不相等 |
| 16 | `p4_3_uuid_differs_by_symbol` | 不同交易对生成不同 UUID | BTC/USDT vs ETH/USDT（其余参数相同） | 两个 UUID 不相等 |

### `crates/infra/virs-type/src/auto/structs_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `a1_1_seventy_percent` | 胜率计算 70% | total_trades=10, win_trades=7, loss_trades=3 | win_rate()=70.0 |
| 2 | `a1_2_zero_wins` | 零胜率 | total_trades=10, win_trades=0, loss_trades=10 | win_rate()=0.0 |
| 3 | `a1_3_zero_trades_division_protection` | 零交易除零保护 | total_trades=0, win_trades=0, loss_trades=0 | win_rate()=0.0 |
| 4 | `a2_1_thirty_percent` | 败率计算 30% | total_trades=10, win_trades=7, loss_trades=3 | loss_rate()=30.0 |
| 5 | `a2_2_zero_trades_division_protection` | 零交易败率除零保护 | total_trades=0, win_trades=0, loss_trades=0 | loss_rate()=0.0 |
| 6 | `a3_1_positive_return` | 正收益率 | total_pnl=1000.0, initial_capital=10000.0 | total_return_pct()=10.0 |
| 7 | `a3_2_negative_return` | 负收益率 | total_pnl=-500.0, initial_capital=10000.0 | total_return_pct()=-5.0 |
| 8 | `a3_3_zero_capital_division_protection` | 零初始资金除零保护 | total_pnl=1000.0, initial_capital=0.0 | total_return_pct()=0.0 |
| 9 | `a4_1_running_status` | 运行中状态判断 | status=running | is_running()=true |
| 10 | `a4_2_stopped_status` | 已停止状态判断（运行中） | status=running | is_running()=false（针对 is_stopped 检查的反例） |
| 11 | `a5_1_stopped_status` | 已停止状态判断 | status=stopped | is_stopped()=true |
| 12 | `a5_2_running_status` | 运行中状态判断（停止的反例） | status=running | is_stopped()=false |
| 13 | `s4_1_auto_bot_roundtrip` | AutoBot 序列化反序列化往返 | name=auto_bot, symbol=ETH/USDT, status=running, total_pnl=250.0, win_trades=10, total_trades=15 | 反序列化后 win_trades、total_pnl、status 与原对象相等 |

### `crates/infra/virs-type/src/order/structs_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `s3_1_order_roundtrip` | Order 序列化反序列化往返 | id=order_123, symbol=BTC/USDT, side=Buy, type=Limit, price=50000.0, filled=0.5, status=PartiallyFilled | 反序列化后与原对象相等 |

### `crates/infra/virs-type/src/exchange/enums_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `e3_1_market_type_display` | MarketType 显示格式 | MarketType::Perpetual | to_string()="perpetual" |
| 2 | `e3_2_margin_mode_serde` | MarginMode 序列化反序列化 | MarginMode::Cross | 序列化为 "\"cross\""，反序列化后相等 |
| 3 | `e3_3_position_mode_serde` | PositionMode 序列化反序列化 | PositionMode::Hedge | 反序列化后与原值相等 |

## 集成测试

### `crates/infra/virs-type/tests/integration_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `int_1_1_long_position_pnl_chain` | 多头仓位盈亏链路 | side=Long, entry=50000.0, qty=1.0, 当前价=51000.0 | unrealized_pnl=1000.0 |
| 2 | `int_1_2_short_position_pnl_chain` | 空头仓位盈亏链路 | side=Short, entry=50000.0, qty=1.0, 当前价=49000.0 | unrealized_pnl=1000.0 |
| 3 | `int_3_1_win_plus_loss_equals_100` | 胜率与败率之和为 100 | total_trades=20, win_trades=14, loss_trades=6, total_pnl=500.0, initial_capital=10000.0 | win_rate=70.0, loss_rate=30.0, total_return_pct=5.0, is_running=true |
| 4 | `int_3_2_negative_return` | 负收益率与停止状态 | status=stopped, total_trades=10, win_trades=3, loss_trades=7, total_pnl=-1500.0, initial_capital=10000.0 | total_return_pct=-15.0, win_rate=30.0, is_stopped=true, is_running=false |
| 5 | `int_5_2_auto_bot_serde_then_win_rate` | AutoBot 序列化后胜率一致性 | status=running, total_trades=25, win_trades=15, loss_trades=10, total_pnl=800.0 | 反序列化后 win_rate 与原始一致, is_running=true |
