use super::{Storage, vault};
use crate::{
    error::{DeckError, Result},
    usage::{
        credentials::Credential,
        types::{QuotaAccount, QuotaSnapshot, UsageSource},
    },
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

pub(crate) struct CachedQuota {
    pub value: QuotaAccount,
    pub observed_at: i64,
    pub retry_at: i64,
    pub failures: i64,
    pub credential_version: i64,
}
impl Storage {
    pub(crate) fn credential_ids(&self) -> Result<Vec<String>> {
        Ok(self
            .connect()?
            .prepare("SELECT id FROM credential_sets ORDER BY rowid")?
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }
    pub(crate) fn credential(&self, id: &str) -> Result<Credential> {
        let guard = self.key.lock().map_err(|_| DeckError::Worker)?;
        let key = guard.as_ref().ok_or(DeckError::VaultLocked)?.clone();
        let db = self.connect()?;
        let (account_id, provider, label, version, source, expires_at, ciphertext, fingerprint): (String,String,String,i64,String,Option<i64>,Vec<u8>,String) = db.query_row(
            "SELECT a.id, a.provider, a.label, c.version, c.source, c.expires_at, c.ciphertext, c.fingerprint FROM credential_sets c JOIN accounts a ON a.id = c.account_id WHERE c.id = ?1", [id],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)))?;
        let aad = format!("{account_id}/{id}/{version}");
        let bytes = vault::open(&key, &ciphertext, aad.as_bytes())?;
        Ok(Credential {
            account_id,
            credential_id: id.into(),
            provider: serde_json::from_value(provider.into())?,
            label,
            version,
            source,
            expires_at,
            document: serde_json::from_slice(&bytes)?,
            key,
            fingerprint,
        })
    }
    pub(crate) fn rotate(
        &self,
        credential: &mut Credential,
        response: &serde_json::Value,
    ) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (current_version, current_fingerprint, ciphertext): (i64, String, Vec<u8>) = tx
            .query_row(
                "SELECT version, fingerprint, ciphertext FROM credential_sets WHERE id = ?1",
                [&credential.credential_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        if current_fingerprint != credential.fingerprint {
            return Err(DeckError::Invalid("当前会话已换用其他凭据，不能覆盖"));
        }
        if current_version != credential.version {
            let latest = vault::open(
                &credential.key,
                &ciphertext,
                format!(
                    "{}/{}/{current_version}",
                    credential.account_id, credential.credential_id
                )
                .as_bytes(),
            )?;
            credential.document = serde_json::from_slice(&latest)?;
        }
        let (document, fingerprint, expires_at) = credential.rotated(response)?;
        let version = current_version + 1;
        let encrypted = vault::seal(
            &credential.key,
            &serde_json::to_vec(&document)?,
            format!(
                "{}/{}/{version}",
                credential.account_id, credential.credential_id
            )
            .as_bytes(),
        )?;
        tx.execute("UPDATE credential_sets SET ciphertext = ?1, fingerprint = ?2, expires_at = ?3, version = ?4 WHERE id = ?5", params![encrypted, fingerprint, expires_at, version, credential.credential_id])?;
        tx.commit()?;
        credential.version = version;
        credential.document = document;
        credential.fingerprint = fingerprint;
        credential.expires_at = expires_at;
        Ok(())
    }
    pub(crate) fn cached_quota(&self, account_id: &str) -> Result<Option<CachedQuota>> {
        let db = self.connect()?;
        let row: Option<(String,i64,i64,i64,i64)> = db.query_row("SELECT result, observed_at, retry_at, failures, credential_version FROM quota_snapshots WHERE account_id = ?1", [account_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional()?;
        row.map(
            |(value, observed_at, retry_at, failures, credential_version)| {
                Ok(CachedQuota {
                    value: serde_json::from_str(&value)?,
                    observed_at,
                    retry_at,
                    failures,
                    credential_version,
                })
            },
        )
        .transpose()
    }
    pub(crate) fn save_quota(
        &self,
        result: &QuotaAccount,
        observed_at: i64,
        failures: i64,
        version: i64,
        request_retry_at: i64,
    ) -> Result<()> {
        self.connect()?.execute("INSERT INTO quota_snapshots VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(account_id) DO UPDATE SET result=excluded.result, observed_at=excluded.observed_at, retry_at=excluded.retry_at, failures=excluded.failures, credential_version=excluded.credential_version", params![result.account_id, serde_json::to_string(result)?, observed_at, request_retry_at, failures, version])?;
        Ok(())
    }
    pub(crate) fn quota_snapshot(&self, offline: bool) -> Result<QuotaSnapshot> {
        let snapshot = self.snapshot()?;
        let mut results = Vec::new();
        let mut latest = None;
        let mut stale = false;
        for account in snapshot.accounts {
            if let Some(cached) = self.cached_quota(&account.id)? {
                latest = Some(latest.map_or(cached.observed_at, |current: i64| {
                    current.max(cached.observed_at)
                }));
                stale |= cached.failures > 0;
                results.push(cached.value);
            } else {
                let mut value = QuotaAccount::empty(account.id, account.provider, account.label);
                value.pending = if offline {
                    "离线模式：未访问额度服务"
                } else if !snapshot.vault_unlocked {
                    "请在主窗口解锁凭据库"
                } else {
                    "尚未查询额度"
                }
                .into();
                results.push(value);
            }
        }
        let fetched_at = latest
            .and_then(|time| chrono::DateTime::<chrono::Utc>::from_timestamp(time, 0))
            .map(|time| time.to_rfc3339());
        let has_success = results.iter().any(|result| result.ok);
        Ok(QuotaSnapshot {
            results,
            state: if !snapshot.vault_unlocked {
                "locked"
            } else if stale {
                if has_success { "stale" } else { "error" }
            } else {
                "cached"
            },
            fetched_at,
            error: String::new(),
        })
    }
    pub(crate) fn usage_sources(&self) -> Result<Vec<UsageSource>> {
        let db = self.connect()?;
        let rows = db
            .prepare(
                "SELECT id, account_id, harness, path, since FROM usage_sources ORDER BY since",
            )?
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, account_id, harness, path, since)| {
                Ok(UsageSource {
                    id,
                    account_id,
                    harness: serde_json::from_value(harness.into())?,
                    path,
                    since,
                })
            })
            .collect()
    }
    pub(crate) fn save_usage_source(&self, mut source: UsageSource) -> Result<String> {
        if source.since < 0 || source.since > crate::now() + 86400 {
            return Err(DeckError::Invalid("归属起始时间无效"));
        }
        let path = std::path::Path::new(&source.path);
        if !path.is_absolute() {
            return Err(DeckError::Invalid("用量来源需要完整的绝对路径"));
        }
        let canonical = path.canonicalize()?;
        let metadata = canonical.metadata()?;
        if matches!(source.harness, crate::usage::types::LocalHarness::Opencode)
            != metadata.is_file()
        {
            return Err(DeckError::Invalid(
                "OpenCode 需要数据库文件，其他客户端需要会话目录",
            ));
        }
        source.path = canonical.to_string_lossy().into_owned();
        source.id = uuid::Uuid::new_v4().to_string();
        let mut connection = self.connect()?;
        let db = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let collision: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM usage_sources s JOIN accounts a ON a.id=s.account_id WHERE s.harness=?1 AND s.path=?2 AND s.since=?3 AND s.account_id != ?4 AND a.provider=(SELECT provider FROM accounts WHERE id=?4))", params![source.harness.key(),source.path,source.since,source.account_id], |row| row.get(0))?;
        if collision {
            return Err(DeckError::Invalid("同一客户端的相同起点已归属其他账号"));
        }
        db.execute("INSERT INTO usage_sources VALUES (?1,?2,?3,?4,?5) ON CONFLICT(account_id,harness,path,since) DO NOTHING", params![source.id, source.account_id, source.harness.key(), source.path, source.since])?;
        let id = db.query_row("SELECT id FROM usage_sources WHERE account_id = ?1 AND harness = ?2 AND path = ?3 AND since = ?4", params![source.account_id, source.harness.key(), source.path, source.since], |row| row.get(0))?;
        db.commit()?;
        Ok(id)
    }
    pub(crate) fn remove_usage_source(&self, id: &str) -> Result<()> {
        self.connect()?
            .execute("DELETE FROM usage_sources WHERE id = ?1", [id])?;
        Ok(())
    }
    pub(crate) fn task_start(&self, kind: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.connect()?.execute(
            "INSERT INTO tasks VALUES (?1,?2,'running',?3,NULL,'正在刷新额度与用量')",
            params![id, kind, crate::now()],
        )?;
        Ok(id)
    }
    pub(crate) fn task_finish(&self, id: &str, state: &str, message: &str) -> Result<()> {
        self.connect()?.execute(
            "UPDATE tasks SET state=?1, message=?2, finished_at=?3 WHERE id=?4",
            params![state, message, crate::now(), id],
        )?;
        Ok(())
    }
}
