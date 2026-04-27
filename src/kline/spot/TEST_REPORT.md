# Spot Handler 测试报告

## 模块概述

SpotHandler 封装了现货市场的 WebSocket 客户端管理逻辑，通过 `KlineWsClient` trait 实现交易所无关的设计。
KlineEngine 通过 SpotHandler 管理所有现货市场的订阅和取消订阅操作。

## 架构关系

```
KlineEngine
  ├── SpotHandler (market_type = Spot)
  │     └── Arc<Mutex<dyn KlineWsClient>>  ← 交易所 WS 适配器注入
  └── PerpetualHandler (market_type = Perpetual)
        └── Arc<Mutex<dyn KlineWsClient>>
```

## 测试用例

### TC-SPOT-001: 现货 Handler 市场类型

| 项目 | 内容 |
|------|------|
| 用例ID | TC-SPOT-001 |
| 测试函数 | `test_spot_handler_market_type` |
| 场景 | 验证 SpotHandler 返回正确的市场类型 |
| 前置条件 | 创建 SpotHandler 实例 |
| 输入 | 无 |
| 预期结果 | `handler.market_type()` 返回 `MarketType::Spot` |
| 实际结果 | ✅ 通过 |
| 优先级 | 高 |

### TC-SPOT-002: 现货 Handler 启停

| 项目 | 内容 |
|------|------|
| 用例ID | TC-SPOT-002 |
| 测试函数 | `test_spot_handler_start_stop` |
| 场景 | 验证 SpotHandler 能正确启动和停止 WS 客户端 |
| 前置条件 | 创建 SpotHandler 实例 |
| 步骤 | 1. 调用 `start()` 2. 检查 `is_running()` 3. 调用 `stop()` 4. 检查 `is_running()` |
| 预期结果 | 启动后 `is_running()` 返回 true，停止后返回 false |
| 实际结果 | ✅ 通过 |
| 优先级 | 高 |

### TC-SPOT-003: 现货 Handler 订阅

| 项目 | 内容 |
|------|------|
| 用例ID | TC-SPOT-003 |
| 测试函数 | `test_spot_handler_subscribe` |
| 场景 | 验证 SpotHandler 能正确向 WS 客户端发送订阅请求 |
| 前置条件 | 创建 SpotHandler 实例 |
| 步骤 | 1. 订阅 BTCUSDT 2. 订阅 ETHUSDT |
| 预期结果 | 不抛出异常，WS 客户端收到两次订阅调用 |
| 实际结果 | ✅ 通过 |
| 优先级 | 高 |

### TC-SPOT-004: 现货 Handler 取消订阅

| 项目 | 内容 |
|------|------|
| 用例ID | TC-SPOT-004 |
| 测试函数 | `test_spot_handler_unsubscribe` |
| 场景 | 验证 SpotHandler 能正确向 WS 客户端发送取消订阅请求 |
| 前置条件 | 创建 SpotHandler 实例并已订阅 |
| 步骤 | 1. 订阅 BTCUSDT 2. 取消订阅 BTCUSDT |
| 预期结果 | 不抛出异常，WS 客户端收到取消订阅调用 |
| 实际结果 | ✅ 通过 |
| 优先级 | 高 |

### TC-SPOT-005: 现货 Handler 初始未运行

| 项目 | 内容 |
|------|------|
| 用例ID | TC-SPOT-005 |
| 测试函数 | `test_spot_handler_not_running_initially` |
| 场景 | 验证 SpotHandler 创建后 WS 客户端未运行 |
| 前置条件 | 创建 SpotHandler 实例 |
| 预期结果 | `is_running()` 返回 false |
| 实际结果 | ✅ 通过 |
| 优先级 | 中 |

### TC-SPOT-006: 现货 Handler 幂等启动

| 项目 | 内容 |
|------|------|
| 用例ID | TC-SPOT-006 |
| 测试函数 | `test_spot_handler_idempotent_start` |
| 场景 | 验证 SpotHandler 多次调用 start 不会出错 |
| 前置条件 | 创建 SpotHandler 实例 |
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

使用 `MockSpotWs` 实现 `KlineWsClient` trait，通过 `AtomicBool` 和 `AtomicUsize` 跟踪状态，
避免异步锁和 unsafe 指针转换。Mock 验证了 Handler 到 WS 客户端的委托调用链路。
