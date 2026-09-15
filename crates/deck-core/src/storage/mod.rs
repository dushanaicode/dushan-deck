mod floating;
#[cfg(test)]
mod tests;
mod usage;
mod vault;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    Snapshot,
    accounts::{Account, CredentialSet, ImportAccount, ImportResult},
    catalog::{self, CreateConnection, ModelConnection},
    error::{DeckError, Result},
    runtime::Task,
    settings::Settings,
};

pub(crate) struct Storage {
    path: PathBuf,
    key: Mutex<Option<Zeroizing<[u8; 32]>>>,
}

impl Storage {
    pub(crate) fn open(state_root: &Path) -> Result<Self> {
        if !state_root.is_absolute() {
            return Err(DeckError::Invalid("state-root 必须是绝对路径"));
        }
        std::fs::create_dir_all(state_root)?;
        let store = Self {
            path: state_root.join("deck.sqlite3"),
            key: Mutex::new(None),
        };
        let mut db = store.connect()?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "FULL")?;
        // SQLite's table-rebuild migration requires foreign keys disabled before BEGIN.
        db.pragma_update(None, "foreign_keys", false)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => {
                tx.execute_batch(include_str!("schema.sql"))?;
                tx.execute(
                    "INSERT INTO settings VALUES (1, ?1)",
                    [serde_json::to_string(&Settings::default())?],
                )?;
            }
            1..=3 => {}
            _ => {
                return Err(DeckError::Invalid(
                    "数据库版本比当前应用新，请使用对应版本打开",
                ));
            }
        }
        if version < 2 {
            tx.execute_batch(include_str!("migration_002.sql"))?;
            tx.execute(
                "INSERT INTO float_settings VALUES (1, ?1, 290, 430, NULL)",
                [serde_json::to_string(
                    &crate::settings::floating::FloatPreferences::default(),
                )?],
            )?;
        }
        if version < 3 {
            tx.execute_batch(include_str!("migration_003.sql"))?;
        }
        if tx
            .prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_some()
        {
            return Err(DeckError::Invalid("数据库迁移后的引用校验失败"));
        }
        tx.execute("UPDATE tasks SET state = 'interrupted', finished_at = ?1, message = '上次运行中断；未自动重放' WHERE state IN ('queued', 'running', 'waiting_user', 'paused')", [crate::now()])?;
        tx.commit()?;
        db.pragma_update(None, "foreign_keys", true)?;
        Ok(store)
    }

    pub(crate) fn connect(&self) -> Result<Connection> {
        let db = Connection::open(&self.path)?;
        db.busy_timeout(Duration::from_secs(2))?;
        db.pragma_update(None, "foreign_keys", true)?;
        db.pragma_update(None, "temp_store", "MEMORY")?;
        Ok(db)
    }

    pub(crate) fn unlock(&self, password: Zeroizing<String>) -> Result<()> {
        let mut current_key = self.key.lock().map_err(|_| DeckError::Worker)?;
        let mut db = self.connect()?;
        let meta: Option<(Vec<u8>, Vec<u8>)> = db
            .query_row(
                "SELECT salt, verifier FROM vault_meta WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let key = match meta {
            Some((salt, verifier)) => {
                let key = vault::derive(&password, &salt)?;
                let marker = vault::open(&key, &verifier, b"dushan-deck/vault/v1")?;
                if marker.as_slice() != b"Dushan Deck" {
                    return Err(DeckError::VaultAuthentication);
                }
                key
            }
            None => {
                let salt = vault::salt()?;
                let key = vault::derive(&password, &salt)?;
                let verifier = vault::seal(&key, b"Dushan Deck", b"dushan-deck/vault/v1")?;
                let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute(
                    "INSERT INTO vault_meta VALUES (1, ?1, ?2)",
                    params![salt.as_slice(), verifier],
                )?;
                tx.commit()?;
                key
            }
        };
        *current_key = Some(key);
        Ok(())
    }

    pub(crate) fn lock(&self) -> Result<()> {
        *self.key.lock().map_err(|_| DeckError::Worker)? = None;
        Ok(())
    }

    pub(crate) fn import(&self, mut request: ImportAccount) -> Result<ImportResult> {
        let credential = request.parse()?;
        let key_guard = self.key.lock().map_err(|_| DeckError::Worker)?;
        let key = key_guard.as_ref().ok_or(DeckError::VaultLocked)?;
        let account_id = Uuid::new_v4().to_string();
        let credential_id = Uuid::new_v4().to_string();
        let aad = format!("{account_id}/{credential_id}/1");
        let encrypted = vault::seal(key, &credential.payload, aad.as_bytes())?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT account_id, id FROM credential_sets WHERE fingerprint = ?1",
                [&credential.fingerprint],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let result = if let Some((account_id, credential_set_id)) = existing {
            // Exact same session: update the whole encrypted document without mixing token fields.
            let version: i64 = tx.query_row(
                "SELECT version FROM credential_sets WHERE id = ?1",
                [&credential_set_id],
                |row| row.get(0),
            )?;
            let next_version = version + 1;
            let encrypted = vault::seal(
                key,
                &credential.payload,
                format!("{account_id}/{credential_set_id}/{next_version}").as_bytes(),
            )?;
            tx.execute("UPDATE credential_sets SET ciphertext = ?1, version = ?2, expires_at = ?3 WHERE id = ?4", params![encrypted, next_version, credential.expires_at, credential_set_id])?;
            tx.execute(
                "UPDATE accounts SET label = ?1 WHERE id = ?2",
                params![request.label.trim(), account_id],
            )?;
            ImportResult {
                account_id,
                credential_set_id,
                outcome: "updated",
            }
        } else {
            tx.execute(
                "INSERT INTO accounts VALUES (?1, ?2, ?3, 'unverified', ?4)",
                params![
                    account_id,
                    request.provider.as_str(),
                    request.label.trim(),
                    crate::now()
                ],
            )?;
            tx.execute(
                "INSERT INTO credential_sets VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)",
                params![
                    credential_id,
                    account_id,
                    credential.fingerprint,
                    credential.kind,
                    credential.source,
                    credential.expires_at,
                    encrypted
                ],
            )?;
            ImportResult {
                account_id,
                credential_set_id: credential_id,
                outcome: "created",
            }
        };
        tx.commit()?;
        Ok(result)
    }

    pub(crate) fn create_connection(&self, request: CreateConnection) -> Result<String> {
        request.validate()?;
        let id = Uuid::new_v4().to_string();
        let db = self.connect()?;
        let changed = db.execute("INSERT INTO model_connections (id, account_id, credential_set_id, label, model, base_url) SELECT ?1, account_id, id, ?2, ?3, ?4 FROM credential_sets WHERE id = ?5", params![id, request.label.trim(), request.model.trim(), request.base_url.trim(), request.credential_set_id])?;
        if changed == 0 {
            return Err(DeckError::Invalid("所选凭据组不存在"));
        }
        Ok(id)
    }

    pub(crate) fn save_settings(&self, settings: Settings) -> Result<()> {
        settings.validate()?;
        self.connect()?.execute(
            "UPDATE settings SET value = ?1 WHERE id = 1",
            [serde_json::to_string(&settings)?],
        )?;
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Result<Snapshot> {
        let unlocked = self.key.lock().map_err(|_| DeckError::Worker)?.is_some();
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let vault_configured =
            tx.query_row("SELECT EXISTS(SELECT 1 FROM vault_meta)", [], |row| {
                row.get(0)
            })?;
        let settings: String =
            tx.query_row("SELECT value FROM settings WHERE id = 1", [], |row| {
                row.get(0)
            })?;
        let accounts = tx.prepare("SELECT json_object('id',id,'provider',provider,'label',label,'identityStatus',identity_status,'createdAt',created_at) FROM accounts ORDER BY created_at, id")?.query_map([], |row| row.get::<_, String>(0))?.map(|row| Ok(serde_json::from_str::<Account>(&row?)?)).collect::<Result<Vec<_>>>()?;
        let credentials = tx.prepare("SELECT id, account_id, kind, source, version, expires_at FROM credential_sets ORDER BY id")?.query_map([], |row| Ok(CredentialSet { id: row.get(0)?, account_id: row.get(1)?, kind: row.get(2)?, source: row.get(3)?, version: row.get(4)?, expires_at: row.get(5)? }))?.collect::<std::result::Result<Vec<_>, _>>()?;
        let connections = tx.prepare("SELECT id, account_id, credential_set_id, label, model, base_url, verified FROM model_connections ORDER BY rowid")?.query_map([], |row| Ok(ModelConnection { id: row.get(0)?, account_id: row.get(1)?, credential_set_id: row.get(2)?, label: row.get(3)?, model: row.get(4)?, base_url: row.get(5)?, verified: row.get(6)? }))?.collect::<std::result::Result<Vec<_>, _>>()?;
        let tasks = tx.prepare("SELECT id, kind, state, created_at, finished_at, message FROM tasks ORDER BY rowid DESC LIMIT 30")?.query_map([], |row| Ok(Task { id: row.get(0)?, kind: row.get(1)?, state: row.get(2)?, created_at: row.get(3)?, finished_at: row.get(4)?, message: row.get(5)? }))?.collect::<std::result::Result<Vec<_>, _>>()?;
        tx.commit()?;
        Ok(Snapshot {
            schema_version: 3,
            providers: catalog::providers(),
            accounts,
            credentials,
            connections,
            tasks,
            settings: serde_json::from_str(&settings)?,
            vault_configured,
            vault_unlocked: unlocked,
        })
    }

    pub(crate) fn check(&self) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let db = self.connect()?;
        db.execute("INSERT INTO tasks VALUES (?1, 'storage_check', 'running', ?2, NULL, '正在核对本地数据库')", params![id, crate::now()])?;
        let result = (|| -> Result<bool> {
            let quick: String = db.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
            let foreign_keys = db
                .prepare("PRAGMA foreign_key_check")?
                .query([])?
                .next()?
                .is_none();
            Ok(quick == "ok" && foreign_keys)
        })();
        let (state, message) = match &result {
            Ok(true) => ("succeeded", "SQLite 完整性与数据引用检查通过"),
            Ok(false) => ("failed", "检测到数据库完整性或引用错误"),
            Err(_) => ("failed", "数据库检查执行失败"),
        };
        db.execute(
            "UPDATE tasks SET state = ?1, message = ?2, finished_at = ?3 WHERE id = ?4",
            params![state, message, crate::now(), id],
        )?;
        result?;
        Ok(id)
    }

    pub(crate) fn shutdown(&self) -> Result<()> {
        self.lock()?;
        self.connect()?
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}
