pub mod accounts;
pub mod catalog;
pub mod error;
pub mod runtime;
pub mod settings;
mod storage;
pub mod usage;

use accounts::{Account, CredentialSet, ImportAccount, ImportResult};
use catalog::{CreateConnection, ModelConnection, ProviderInfo};
use error::{DeckError, Result};
use runtime::{Runtime, Task};
use serde::Serialize;
use settings::Settings;
use settings::floating::{FloatPreferences, FloatState};
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
    quota_client: Arc<usage::QuotaClient>,
    refresh_control: Arc<usage::RefreshControl>,
}

impl Deck {
    /// Call startup on a blocking thread, before admitting any commands.
    pub fn open(state_root: &Path) -> Result<Self> {
        Self::open_with_network(state_root, true)
    }
    pub fn open_with_network(state_root: &Path, network: bool) -> Result<Self> {
        Ok(Self {
            storage: Arc::new(Storage::open(state_root)?),
            runtime: Runtime::new(),
            quota_client: Arc::new(usage::QuotaClient::new(!network)?),
            refresh_control: Arc::new(usage::RefreshControl::default()),
        })
    }
    pub async fn quota_snapshot(&self) -> Result<usage::types::QuotaSnapshot> {
        let storage = self.storage.clone();
        let offline = self.quota_client.offline;
        self.runtime
            .execute(move || storage.quota_snapshot(offline))
            .await
    }
    pub async fn refresh_quotas(&self, force: bool) -> Result<usage::types::QuotaSnapshot> {
        let generation = self
            .refresh_control
            .generation
            .load(std::sync::atomic::Ordering::Acquire);
        self.runtime
            .execute_async(usage::refresh(
                self.storage.clone(),
                self.quota_client.clone(),
                self.refresh_control.clone(),
                generation,
                force,
            ))
            .await
    }
    pub async fn usage_sources(&self) -> Result<Vec<usage::types::UsageSource>> {
        let storage = self.storage.clone();
        self.runtime.execute(move || storage.usage_sources()).await
    }
    pub async fn save_usage_source(&self, source: usage::types::UsageSource) -> Result<String> {
        let storage = self.storage.clone();
        self.runtime
            .execute(move || storage.save_usage_source(source))
            .await
    }
    pub async fn remove_usage_source(&self, id: String) -> Result<()> {
        let storage = self.storage.clone();
        self.runtime
            .execute(move || storage.remove_usage_source(&id))
            .await
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
    pub async fn float_state(&self) -> Result<FloatState> {
        let storage = self.storage.clone();
        self.runtime.execute(move || storage.float_state()).await
    }
    pub async fn save_float_preferences(&self, preferences: FloatPreferences) -> Result<()> {
        let storage = self.storage.clone();
        self.runtime
            .execute(move || storage.save_float_preferences(preferences))
            .await
    }
    pub async fn save_float_size(&self, width: u32, height: u32) -> Result<()> {
        let storage = self.storage.clone();
        self.runtime
            .execute(move || storage.save_float_size(width, height))
            .await
    }
    pub async fn save_float_background(&self, background: Option<String>) -> Result<()> {
        let storage = self.storage.clone();
        self.runtime
            .execute(move || storage.save_float_background(background))
            .await
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
