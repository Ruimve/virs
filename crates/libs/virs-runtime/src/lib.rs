//! virs-runtime: 统一的运行时任务管理基础设施
//!
//! 提供 CancellationToken（树形取消传播）、TaskSupervisor（JoinHandle 管理 + 优雅关闭）、
//! PeriodicTask（周期任务原语）、RetryTask（退避重试原语）四个核心抽象。
//!
//! 所有需要后台任务管理的 crate 通过依赖 virs-runtime 获得统一的生命周期管理能力。

mod cancel;
mod periodic;
mod retry;
mod supervisor;

pub use cancel::CancellationToken;
pub use periodic::{PeriodicTask, PeriodicTaskBuilder};
pub use retry::{BackoffStrategy, RetryTask, RetryTaskBuilder};
pub use supervisor::{SupervisedHandle, TaskSupervisor, TaskSupervisorBuilder};
