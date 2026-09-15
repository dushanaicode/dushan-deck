pub mod accounts;
pub mod catalog;
pub mod error;
pub mod runtime;
pub mod settings;
mod storage;

use accounts::{Account, CredentialSet, ImportAccount, ImportResult};
use catalog::{CreateConnection, ModelConnection, ProviderInfo};
use error::{DeckError, Result};
use runtime::{Runtime, Task};
use serde::Serialize;
use settings::Settings;
use std::{
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use storage::Storage;
use zeroize::Zeroizing;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema_version: u32,
    pub providers: Vec<ProviderInfo>,
    pub accounts: Vec<Account>,
    pub credentials: Vec<CredentialSet>,
    pub connections: Vec<ModelConnection>,
    pub tasks: Vec<Task>,
    pub settings: Settings,
    pub vault_configured: bool,
    pub vault_unlocked: bool,
}

pub struct Deck {
    storage: Arc<Storage>,
    runtime: Runtime,
}

impl Deck {
    /// Call startup on a blocking thread, before admitting any commands.
    pub fn open(state_root: &Path) -> Result<Self> {
        Ok(Self {
            storage: Arc::new(Storage::open(state_root)?),
            runtime: Runtime::new(),
        })
    }

    pub async fn snapshot(&self) -> Result<Snapshot> {
        let storage = self.storage.clone();
        self.runtime.execute(move || storage.snapshot()).await
    }
    pub async fn unlock(&self, password: String) -> Result<()> {
        let password = Zeroizing::new(password);
        let storage = self.storage.clone();
        self.runtime.execute(move || storage.unlock(password)).await
    }
    pub async fn lock(&self) -> Result<()> {
        let storage = self.storage.clone();
        self.runtime.execute(move || storage.lock()).await
    }
    pub async fn import(&self, request: ImportAccount) -> Result<ImportResult> {
        let storage = self.storage.clone();
        self.runtime.execute(move || storage.import(request)).await
    }
    pub async fn create_connection(&self, request: CreateConnection) -> Result<String> {
        let storage = self.storage.clone();
        self.runtime
            .execute(move || storage.create_connection(request))
            .await
    }
    pub async fn save_settings(&self, settings: Settings) -> Result<()> {
        let storage = self.storage.clone();
        self.runtime
            .execute(move || storage.save_settings(settings))
            .await
    }
    pub async fn check_storage(&self) -> Result<String> {
        let storage = self.storage.clone();
        self.runtime.execute(move || storage.check()).await
    }
    pub async fn shutdown(&self) -> Result<()> {
        let guard = self.runtime.shutdown().await?;
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            storage.shutdown()
        })
        .await
        .map_err(|_| DeckError::Worker)?
    }
}

fn validate_label(label: &str) -> Result<()> {
    if label.trim().is_empty() || label.chars().count() > 80 || label.chars().any(char::is_control)
    {
        return Err(DeckError::Invalid("名称须为 1–80 个字符，且不含控制字符"));
    }
    Ok(())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_secs() as i64
}
