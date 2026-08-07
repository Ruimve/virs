# virs-exchange - 测试计划

**测试总数:** 13（单元测试: 0，集成测试: 13）

---

## 集成测试

### `crates/core/virs-exchange/tests/integration_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `int_1_1_side_roundtrip` | 方向类型一致性验证 | `Side::Buy` | 等于 `Side::Buy` |
| 2 | `int_1_2_order_type_roundtrip` | 订单类型一致性验证 | `OrderType::Limit` | 等于 `OrderType::Limit` |
| 3 | `int_1_3_position_side_consistency` | 持仓方向一致性 | `PositionSide::Long` | 等于 `PositionSide::Long` |
| 4 | `int_1_4_market_type_consistency` | 市场类型一致性 | `MarketType::Perpetual` | 等于 `MarketType::Perpetual` |
| 5 | `int_2_1_paper_exchange_creation_and_balance` | 模拟交易所创建和余额查询 | name=`binance`，MarketType::Perpetual，初始余额=100000.0 | name=`binance`，asset=`USDT`，total=100000.0，free=100000.0，used=0.0 |
| 6 | `int_2_2_paper_market_order_updates_balance` | 模拟市价单更新余额和持仓 | BTCUSDT @ 50000.0，买入 0.1 BTC，杠杆=10 | order_id 非空，used>0，free<50000，持仓 1 条 Long，quantity=0.1 |
| 7 | `int_3_1_registry_register_and_get` | 注册表查询不存在的交易所 | key: `nonexistent` | 返回 None |
| 8 | `int_3_2_registry_get_nonexistent` | 注册表查询不存在的交易所 | key: `does_not_exist` | 返回 None |
| 9 | `int_3_3_registry_list_names` | 新建注册表列名为空 | 新建 Exchanges | names 为空 |
| 10 | `int_4_1_paper_mode_routes_public_to_real` | 模拟模式公开请求路由到真实交易所 | `get_ticker("BTCUSDT")` | 真实交易所 get_ticker 被调用 |
| 11 | `int_4_2_paper_mode_routes_private_to_paper` | 模拟模式私有请求路由到模拟交易所 | `get_balance()` | 真实交易所 get_balance 未调用，balance.total=10000.0 |
| 12 | `int_4_3_paper_mode_set_leverage_calls_both` | 模拟模式 set_leverage 调用真实交易所并支持下单 | set_leverage("BTCUSDT", 10) + 市价单 0.01 BTC | 真实 set_leverage 被调用，下单成功 |
| 13 | `int_4_4_paper_mode_name_returns_real_name` | 模拟模式 name 返回真实交易所名 | MockRealExchange | name=`binance`，market_type=Perpetual |
