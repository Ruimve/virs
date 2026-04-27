# Perpetual Handler 测试报告

## 模块概述

PerpetualHandler 封装了永续合约市场的 WebSocket 客户端管理逻辑，通过 `KlineWsClient` trait 实现交易所无关的设计。
KlineEngine 通过 PerpetualHandler 管理所有永续合约市场的订阅和取消订阅操作。

## 架构关系

```
KlineEngine
  ├── SpotHandler (market_type = Spot)
  │     └── Arc<Mutex<dyn KlineWsClient>>
  └── PerpetualHandler (market_type = Perpetual)
        └── Arc<Mutex<dyn KlineWsClient>>  ← 交易所 WS 适配器注入
```

## 测试用例

### TC-PERP-001: 永续 Handler 市场类型

| 项目 | 内容 |
|------|------|
| 用例ID | TC-PERP-001 |
| 测试函数 | `test_perpetual_handler_market_type` |
| 场景 | 验证 PerpetualHandler 返回正确的市场类型 |
| 前置条件 | 创建 PerpetualHandler 实例 |
| 输入 | 无 |
| 预期结果 | `handler.market_type()` 返回 `MarketType::Perpetual` |
| 实际结果 | ✅ 通过 |
| 优先级 | 高 |

### TC-PERP-002: 永续 Handler 启停

| 项目 | 内容 |
|------|------|
| 用例ID | TC-PERP-002 |
| 测试函数 | `test_perpetual_handler_start_stop` |
| 场景 | 验证 PerpetualHandler 能正确启动和停止 WS 客户端 |
| 前置条件 | 创建 PerpetualHandler 实例 |
| 步骤 | 1. 调用 `start()` 2. 检查 `is_running()` 3. 调用 `stop()` 4. 检查 `is_running()` |
| 预期结果 | 启动后 `is_running()` 返回 true，停止后返回 false |
| 实际结果 | ✅ 通过 |
| 优先级 | 高 |

### TC-PERP-003: 永续 Handler 订阅

| 项目 | 内容 |
|------|------|
| 用例ID | TC-PERP-003 |
| 测试函数 | `test_perpetual_handler_subscribe` |
| 场景 | 验证 PerpetualHandler 能正确向 WS 客户端发送订阅请求 |
| 前置条件 | 创建 PerpetualHandler 实例 |
| 步骤 | 1. 订阅 BTCUSDT 2. 订阅 ETHUSDT |
| 预期结果 | 不抛出异常，WS 客户端收到两次订阅调用 |
| 实际结果 | ✅ 通过 |
| 优先级 | 高 |

### TC-PERP-004: 永续 Handler 取消订阅

| 项目 | 内容 |
|------|------|
| 用例ID | TC-PERP-004 |
| 测试函数 | `test_perpetual_handler_unsubscribe` |
| 场景 | 验证 PerpetualHandler 能正确向 WS 客户端发送取消订阅请求 |
| 前置条件 | 创建 PerpetualHandler 实例并已订阅 |
| 步骤 | 1. 订阅 BTCUSDT 2. 取消订阅 BTCUSDT |
| 预期结果 | 不抛出异常，WS 客户端收到取消订阅调用 |
| 实际结果 | ✅ 通过 |
| 优先级 | 高 |

### TC-PERP-005: 永续 Handler 初始未运行

| 项目 | 内容 |
|------|------|
| 用例ID | TC-PERP-005 |
| 测试函数 | `test_perpetual_handler_not_running_initially` |
| 场景 | 验证 PerpetualHandler 创建后 WS 客户端未运行 |
| 前置条件 | 创建 PerpetualHandler 实例 |
| 预期结果 | `is_running()` 返回 false |
| 实际结果 | ✅ 通过 |
| 优先级 | 中 |

### TC-PERP-006: 永续 Handler 幂等启动

| 项目 | 内容 |
|------|------|
| 用例ID | TC-PERP-006 |
| 测试函数 | `test_perpetual_handler_idempotent_start` |
| 场景 | 验证 PerpetualHandler 多次调用 start 不会出错 |
| 前置条件 | 创建 PerpetualHandler 实例 |
| 步骤 | 1. 调用 `start()` 2. 再次调用 `start()` 3. 检查 `is_running()` |
| 预期结果 | 不抛出异常，`is_running()` 返回 true |
| 实际结果 | ✅ 通过 |
| 优先级 | 中 |

## 测试统计

| 指标 | 值 |
|------|-----|
| 总用例数 | 6 |
| 通过 | 6 |
| 失败 | 0 |
| 跳过 | 0 |
| 通过率 | 100% |

## Mock 策略

使用 `MockPerpetualWs` 实现 `KlineWsClient` trait，通过 `AtomicBool` 和 `AtomicUsize` 跟踪状态，
避免异步锁和 unsafe 指针转换。Mock 验证了 Handler 到 WS 客户端的委托调用链路。

## 与 Spot Handler 的差异

当前 SpotHandler 和 PerpetualHandler 的接口和实现完全对称，差异仅在于：
1. `market_type()` 返回值不同（Spot vs Perpetual）
2. 注入的 WS 客户端连接不同的交易所端点

未来可扩展的差异点：
- 永续合约可能需要处理 funding rate 数据
- 永续合约的 symbol 格式可能不同（如 BTCUSDT vs BTCUSDT_PERP）
- 永续合约的 gap 检测阈值可能不同
