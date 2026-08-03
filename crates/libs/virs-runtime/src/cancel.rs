use tokio_util::sync::CancellationToken as InnerToken;

/// 取消令牌 — 统一的异步取消信号，支持树形传播。
///
/// 内部基于 `tokio_util::sync::CancellationToken`，无竞争窗口。
///
/// 父令牌取消时，所有通过 `child_token()` 创建的子令牌自动取消。
/// 可在 `tokio::select!` 中通过 `cancelled()` 中断任意 `sleep` / `interval.tick()`。
///
/// # 示例
///
/// ```ignore
/// use virs_runtime::CancellationToken;
///
/// # tokio_test::block_on(async {
/// let (cancel, cancel_guard) = CancellationToken::new();
/// let child = cancel.child_token();
///
/// tokio::select! {
///     _ = child.cancelled() => {
///         println!("cancelled");
///     }
///     _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
///         println!("timed out");
///     }
/// }
///
/// drop(cancel_guard); // 触发取消
/// # });
/// ```
#[derive(Clone)]
pub struct CancellationToken {
    inner: InnerToken,
}

/// 取消守卫 — drop 时触发对应的 CancellationToken 取消。
///
/// 通过 `CancellationToken::new()` 创建，保证取消信号一定会被触发，
/// 即使持有令牌的代码路径因 panic 提前退出。
pub struct CancelGuard {
    inner: CancellationToken,
}

impl CancellationToken {
    /// 创建一个根令牌和对应的取消守卫。
    ///
    /// 守卫 drop 时触发取消，适用于 RAII 模式。
    /// 如需手动控制取消时机，可调用 `cancel()` 方法。
    #[must_use]
    pub fn new() -> (CancellationToken, CancelGuard) {
        let token = Self::root();
        (token.clone(), CancelGuard { inner: token })
    }

    /// 创建一个无守卫的根令牌，需通过 `cancel()` 手动触发取消。
    #[must_use]
    pub fn root() -> CancellationToken {
        CancellationToken {
            inner: InnerToken::new(),
        }
    }

    /// 创建子令牌。父令牌取消时，子令牌自动取消（但子令牌取消不影响父令牌）。
    ///
    /// 由 `tokio_util` 原生支持，无需额外 spawn 传播任务。
    #[must_use]
    pub fn child_token(&self) -> CancellationToken {
        CancellationToken {
            inner: self.inner.child_token(),
        }
    }

    /// 是否已被取消。
    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }

    /// 手动触发取消。
    pub fn cancel(&self) {
        self.inner.cancel();
    }

    /// 返回一个 Future，当令牌被取消时完成。
    pub async fn cancelled(&self) {
        self.inner.cancelled().await;
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::root()
    }
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        self.inner.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_cancel_guard_drops() {
        let (token, guard) = CancellationToken::new();
        assert!(!token.is_cancelled());
        drop(guard);
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_manual_cancel() {
        let token = CancellationToken::root();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
        // cancelled() 应立即返回
        token.cancelled().await;
    }

    #[tokio::test]
    async fn test_cancelled_in_select() {
        let token = CancellationToken::root();
        let token_clone = token.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            token_clone.cancel();
        });

        tokio::select! {
            _ = token.cancelled() => {}
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                panic!("should have been cancelled");
            }
        }
    }

    #[tokio::test]
    async fn test_child_cancels_with_parent() {
        let parent = CancellationToken::root();
        let child = parent.child_token();

        assert!(!parent.is_cancelled());
        assert!(!child.is_cancelled());

        parent.cancel();
        // tokio_util 的 child_token 是同步传播的，无需 yield
        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[tokio::test]
    async fn test_child_cancel_does_not_affect_parent() {
        let parent = CancellationToken::root();
        let child = parent.child_token();

        child.cancel();

        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancelled_no_race_window() {
        // 验证 tokio_util 实现无竞争窗口：
        // 在 cancelled() 创建后、await 之前 cancel，应立即感知
        let token = CancellationToken::root();
        let clone = token.clone();

        // 在另一个 task 中 cancel
        tokio::spawn(async move {
            clone.cancel();
        });

        // 给 spawn 时间执行
        tokio::task::yield_now().await;

        // cancelled() 应该立即返回（不会因竞争窗口挂起）
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            token.cancelled(),
        )
        .await;
        assert!(result.is_ok(), "cancelled() should not hang due to race");
    }
}
