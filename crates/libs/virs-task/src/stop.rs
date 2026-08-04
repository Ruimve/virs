use std::future::Future;

use tokio_util::sync::CancellationToken;

pub struct Stop(CancellationToken);

impl Stop {
    pub(crate) fn new() -> Self {
        Stop(CancellationToken::new())
    }

    pub(crate) fn clone(&self) -> Self {
        Stop(self.0.clone())
    }

    pub(crate) fn cancel(&self) {
        self.0.cancel();
    }

    pub fn cancelled(&self) -> impl Future<Output = ()> + '_ {
        self.0.cancelled()
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}
