use tokio_util::sync::CancellationToken as InnerToken;

#[derive(Clone)]
pub struct CancellationToken {
    inner: InnerToken,
}

impl CancellationToken {
    #[must_use]
    pub fn root() -> CancellationToken {
        CancellationToken {
            inner: InnerToken::new(),
        }
    }

    #[must_use]
    pub fn child_token(&self) -> CancellationToken {
        CancellationToken {
            inner: self.inner.child_token(),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }

    pub fn cancel(&self) {
        self.inner.cancel();
    }

    pub async fn cancelled(&self) {
        self.inner.cancelled().await;
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::root()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_manual_cancel() {
        let token = CancellationToken::root();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
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
}
