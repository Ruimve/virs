# virs-task - 测试计划

**测试总数:** 9（单元测试: 9，集成测试: 0）

---

## 单元测试

### `crates/foundation/virs-task/src/spawn_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `test_spawn_cancel` | 取消 spawn 任务 | spawn 60s 睡眠任务后 cancel | flag 仍为 false（任务被取消，未执行到赋值） |
| 2 | `test_independent_tasks` | 独立任务互不影响 | 任务 a 和 b 均 60s 睡眠，取消 a | a 的 flag=false，b 的 flag=false（仍在运行未完成）；取消 b 后 b 的 flag=false |
| 3 | `test_periodic_first_tick_immediate` | 周期任务首次立即执行 | 间隔 50ms，first_tick=true，等待 120ms | counter>=2 |
| 4 | `test_periodic_first_tick_delayed` | 周期任务首次延迟执行 | 间隔 100ms，first_tick=false，等待 50ms | counter=0；再等待 120ms 后 counter>=1 |
| 5 | `test_periodic_cancel_stops` | 取消周期任务停止计数 | 间隔 50ms，first_tick=true，等待 120ms 后 cancel | cancel 前 counter>=2，cancel 后 counter 不再增长 |
| 6 | `test_periodic_independent` | 周期任务独立性 | 任务 a 和 b 间隔 50ms，取消 a | b 继续计数（counter_b 增加） |

### `crates/foundation/virs-task/src/task_handle_tests.rs`

| 编号 | 测试函数 | 场景 | 输入数据 | 预期结果 |
|------|---------|------|---------|---------|
| 1 | `test_drop_cancels` | Drop 句柄取消任务 | spawn 60s 睡眠任务后 drop handle | flag 仍为 false（任务被取消） |
| 2 | `test_join_default_timeout` | 默认超时 join 等待任务结束 | spawn 60s 睡眠任务后 join | 耗时 < 6s |
| 3 | `test_join_custom_timeout` | 自定义超时 join 等待任务结束 | spawn 60s 睡眠任务后 join_with_timeout(2s) | 耗时 < 3s |
