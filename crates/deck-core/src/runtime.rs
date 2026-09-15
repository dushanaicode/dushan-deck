use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{RwLock, Semaphore};

use crate::error::{DeckError, Result};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub created_at: i64,
    pub finished_at: Option<i64>,
    pub message: String,
}

pub(crate) struct Runtime {
    closing: AtomicBool,
    gate: Arc<RwLock<()>>,
    workers: Arc<Semaphore>,
}

impl Runtime {
    pub(crate) fn new() -> Self {
        Self {
            closing: AtomicBool::new(false),
            gate: Arc::new(RwLock::new(())),
            workers: Arc::new(Semaphore::new(4)),
        }
    }

    pub(crate) async fn execute<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let worker = self
            .workers
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| DeckError::ShuttingDown)?;
        let guard = self.gate.clone().read_owned().await;
        if self.closing.load(Ordering::Acquire) {
            return Err(DeckError::ShuttingDown);
        }
        tokio::task::spawn_blocking(move || {
            // Own the gate inside the blocking worker: dropping its caller must not let exit race a write.
            let (_worker, _guard) = (worker, guard);
            work()
        })
        .await
        .map_err(|_| DeckError::Worker)?
    }

    pub(crate) async fn shutdown(&self) -> Result<tokio::sync::OwnedRwLockWriteGuard<()>> {
        self.closing.store(true, Ordering::Release);
        self.workers.close();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.gate.clone().write_owned(),
        )
        .await
        .map_err(|_| DeckError::ShutdownTimeout)
    }
    pub(crate) async fn execute_async<T: Send + 'static>(
        &self,
        work: impl std::future::Future<Output = Result<T>> + Send + 'static,
    ) -> Result<T> {
        let worker = self
            .workers
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| DeckError::ShuttingDown)?;
        let guard = self.gate.clone().read_owned().await;
        if self.closing.load(Ordering::Acquire) {
            return Err(DeckError::ShuttingDown);
        }
        tokio::spawn(async move {
            let (_worker, _guard) = (worker, guard);
            work.await
        })
        .await
        .map_err(|_| DeckError::Worker)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn abandoning_a_command_does_not_release_a_running_blocking_write() {
        let runtime = Arc::new(Runtime::new());
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let caller = {
            let runtime = runtime.clone();
            tokio::spawn(async move {
                runtime
                    .execute(move || {
                        started_tx.send(()).unwrap();
                        finish_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                        Ok(())
                    })
                    .await
            })
        };
        started_rx.await.unwrap();
        caller.abort();
        let mut shutdown = tokio::spawn(async move { runtime.shutdown().await });
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut shutdown)
                .await
                .is_err()
        );
        finish_tx.send(()).unwrap();
        assert!(shutdown.await.unwrap().is_ok());
    }
}
