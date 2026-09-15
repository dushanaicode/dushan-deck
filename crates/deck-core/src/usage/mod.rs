pub(crate) mod credentials;
mod http;
mod local;
mod providers;
mod remote;
#[cfg(test)]
mod tests;
pub mod types;

use crate::{
    error::{DeckError, Result},
    storage::Storage,
};
use credentials::Credential;
pub(crate) use http::QuotaClient;
use http::{QueryError, optional_error};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::Mutex;
use types::{Harness, QuotaAccount, QuotaSnapshot};

#[derive(Default)]
pub(crate) struct RefreshControl {
    pub lock: Mutex<()>,
    pub generation: AtomicU64,
}
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| DeckError::Worker)?
}
async fn renew(
    storage: Arc<Storage>,
    client: &QuotaClient,
    mut credential: Credential,
) -> std::result::Result<Credential, QueryError> {
    let store = storage.clone();
    let id = credential.credential_id.clone();
    let task = blocking(move || store.task_start(&format!("credential_refresh:{id}")))
        .await
        .map_err(|error| QueryError::invalid(error.to_string()))?;
    let response = client.refresh(&credential).await;
    let result = match response {
        Ok(response) => {
            let store = storage.clone();
            blocking(move || {
                store.rotate(&mut credential, &response)?;
                Ok(credential)
            })
            .await
            .map_err(|error| QueryError::invalid(error.to_string()))
        }
        Err(error) => Err(error),
    };
    let succeeded = result.is_ok();
    let message = if succeeded {
        "完整续期凭据已加密保存；未修改客户端文件".into()
    } else {
        result.as_ref().err().expect("failure").message.clone()
    };
    blocking(move || {
        storage.task_finish(
            &task,
            if succeeded { "succeeded" } else { "failed" },
            &message,
        )
    })
    .await
    .map_err(|error| QueryError::invalid(error.to_string()))?;
    result
}
async fn one(
    storage: Arc<Storage>,
    client: Arc<QuotaClient>,
    mut credential: Credential,
    force: bool,
    local: Arc<local::LocalReport>,
) -> Result<()> {
    let store = storage.clone();
    let account_id = credential.account_id.clone();
    let cached = blocking(move || store.cached_quota(&account_id)).await?;
    let previous_usage = cached
        .as_ref()
        .map(|cached| cached.value.usage.clone())
        .unwrap_or_default();
    if cached.as_ref().is_some_and(|cached| {
        cached.credential_version == credential.version
            && (cached.retry_at > crate::now()
                || (!force && cached.observed_at > crate::now() - 60))
    }) {
        let mut cached = cached.expect("cache condition matched");
        merge_local(&mut cached.value, &local, &previous_usage);
        return blocking(move || {
            storage.save_quota(
                &cached.value,
                cached.observed_at,
                cached.failures,
                cached.credential_version,
                cached.retry_at,
            )
        })
        .await;
    }
    let mut renewed = false;
    let mut renewal_error = None;
    if credential
        .expires_at
        .is_some_and(|expiry| expiry < crate::now() + 60)
        && !credential.refresh().is_empty()
    {
        let id = credential.credential_id.clone();
        match renew(storage.clone(), &client, credential).await {
            Ok(value) => {
                credential = value;
                renewed = true;
            }
            Err(error) => {
                renewal_error = Some(error);
                let store = storage.clone();
                credential = blocking(move || store.credential(&id)).await?;
            }
        }
    }
    let mut result = match renewal_error {
        Some(error) => Err(error),
        None => client.quota(&credential).await,
    };
    if result
        .as_ref()
        .is_err_and(|error| error.code == "authentication")
        && !renewed
        && !credential.refresh().is_empty()
    {
        let id = credential.credential_id.clone();
        match renew(storage.clone(), &client, credential).await {
            Ok(value) => {
                credential = value;
                result = client.quota(&credential).await;
            }
            Err(error) => {
                result = Err(error);
                let store = storage.clone();
                credential = blocking(move || store.credential(&id)).await?;
            }
        }
    }
    let now = crate::now();
    let (mut value, observed_at, failures) = match result {
        Ok(value) => (value, now, 0),
        Err(error) => {
            let failures = cached.as_ref().map_or(1, |cached| cached.failures + 1);
            let observed_at = cached.as_ref().map_or(now, |cached| cached.observed_at);
            let mut value = cached.map(|cached| cached.value).unwrap_or_else(|| {
                QuotaAccount::empty(
                    credential.account_id.clone(),
                    credential.provider,
                    credential.label.clone(),
                )
            });
            value.pending.clear();
            let retry = error
                .retry_after
                .max(30 * 2_i64.pow((failures - 1).min(6) as u32))
                .min(3600);
            value.retry_at = now + retry;
            if value.ok {
                value.notice = error.message;
                value.error.clear();
            } else {
                value.error = error.message;
                value.notice.clear();
            }
            (value, observed_at, failures)
        }
    };
    if failures == 0 {
        match remote::fetch(&client, &credential).await {
            Ok(rows) => value.usage = rows,
            Err(error) => {
                value.usage = previous_usage
                    .iter()
                    .filter(|row| row.source == "remote")
                    .cloned()
                    .collect();
                optional_error(&mut value, "远端用量", &error);
            }
        }
    }
    let request_retry_at = value.retry_at;
    merge_local(&mut value, &local, &previous_usage);
    let version = credential.version;
    blocking(move || storage.save_quota(&value, observed_at, failures, version, request_retry_at))
        .await
}
fn merge_local(value: &mut QuotaAccount, local: &local::LocalReport, previous: &[types::UsageRow]) {
    if let Some(start) = value.notice.find("本机用量：") {
        value.notice.truncate(start);
        value.notice = value.notice.trim_end_matches('；').to_owned();
    }
    value.usage.retain(|row| row.source != "local");
    if let Some(rows) = local.rows.get(&value.account_id) {
        value.usage.extend(rows.clone());
    }
    if let Some(failed) = local.failed.get(&value.account_id) {
        value.usage.extend(
            previous
                .iter()
                .filter(|row| row.source == "local" && failed.contains(&row.harness))
                .cloned(),
        );
    }
    if let Some(warning) = local.warnings.get(&value.account_id) {
        optional_error(value, "本机用量", &QueryError::invalid(warning));
    }
    let mut harnesses = std::collections::BTreeMap::new();
    for row in &value.usage {
        if row.source == "local" {
            harnesses.insert(row.harness.clone(), row.harness_label.clone());
        }
    }
    value.harnesses = harnesses
        .into_iter()
        .map(|(key, label)| Harness { key, label })
        .collect();
}
pub(crate) async fn refresh(
    storage: Arc<Storage>,
    client: Arc<QuotaClient>,
    control: Arc<RefreshControl>,
    expected_generation: u64,
    force: bool,
) -> Result<QuotaSnapshot> {
    let _guard = control.lock.lock().await;
    let store = storage.clone();
    let offline = client.offline;
    let initial = blocking(move || store.quota_snapshot(offline)).await?;
    if control.generation.load(Ordering::Acquire) != expected_generation
        || initial.state == "locked"
        || client.offline
    {
        return Ok(initial);
    }
    let store = storage.clone();
    let (ids, sources, accounts) = blocking(move || {
        Ok((
            store.credential_ids()?,
            store.usage_sources()?,
            store.snapshot()?.accounts,
        ))
    })
    .await?;
    if ids.is_empty() {
        return Ok(initial);
    }
    let mut account_providers: HashMap<_, _> = accounts
        .into_iter()
        .map(|account| (account.id, account.provider))
        .collect();
    let mut credentials = Vec::new();
    let mut pins: HashMap<(String, String), Vec<String>> = HashMap::new();
    for id in ids {
        let store = storage.clone();
        let credential = blocking(move || store.credential(&id)).await?;
        account_providers.insert(credential.account_id.clone(), credential.usage_provider());
        for pin in credential.omp_pins() {
            pins.entry(pin)
                .or_default()
                .push(credential.account_id.clone());
        }
        credentials.push(credential);
    }
    let local = Arc::new(
        blocking(move || {
            Ok(local::collect(
                &sources,
                &account_providers,
                &pins,
                crate::now(),
            ))
        })
        .await?,
    );
    let mut jobs = tokio::task::JoinSet::new();
    let task_store = storage.clone();
    let task = blocking(move || task_store.task_start("quota_refresh")).await?;
    for credential in credentials {
        let storage = storage.clone();
        let client = client.clone();
        let local = local.clone();
        jobs.spawn(async move { one(storage, client, credential, force, local).await });
    }
    let mut failed = None;
    while let Some(result) = jobs.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => failed = Some(error),
            Err(_) => failed = Some(DeckError::Worker),
        }
    }
    let finish_store = storage.clone();
    let succeeded = failed.is_none();
    blocking(move || {
        finish_store.task_finish(
            &task,
            if succeeded { "succeeded" } else { "failed" },
            if succeeded {
                "额度与用量刷新已结束，请查看各账号状态"
            } else {
                "刷新过程中发生本地错误，请检查账号状态"
            },
        )
    })
    .await?;
    control.generation.fetch_add(1, Ordering::Release);
    if let Some(error) = failed {
        return Err(error);
    }
    blocking(move || storage.quota_snapshot(offline)).await
}
