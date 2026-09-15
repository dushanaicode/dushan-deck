use deck_core::{
    Deck,
    accounts::{ImportAccount, ImportFormat},
    catalog::{CreateConnection, Provider},
    error::DeckError,
    settings::Settings,
};
use rusqlite::{Connection, params};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

const PASSWORD: &str = "synthetic-vault-password-2026";

fn state_root(name: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var_os("DECK_TEST_ROOT")
            .expect("tests require explicit DECK_TEST_ROOT under the workspace Temp"),
    );
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert!(root.is_absolute() && root.starts_with(workspace.join("Temp")));
    let root = root.join(format!("{name}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    assert!(
        root.canonicalize()
            .unwrap()
            .starts_with(workspace.join("Temp").canonicalize().unwrap())
    );
    root
}

fn key(provider: Provider, value: &str) -> ImportAccount {
    ImportAccount {
        provider,
        label: "同一个来源".into(),
        format: ImportFormat::ApiKey,
        content: value.into(),
    }
}
fn oauth(provider: Provider, name: &str) -> ImportAccount {
    let content = match provider {
        Provider::Claude => {
            serde_json::json!({"claudeAiOauth": {"accessToken": format!("synthetic-access-{name}"), "refreshToken": format!("synthetic-refresh-{name}"), "expiresAt": 2000000000000_i64, "scopes": ["user:profile"]}, "retained": "unknown-field"})
        }
        Provider::Openai => {
            serde_json::json!({"tokens": {"access_token": format!("synthetic-access-{name}"), "refresh_token": format!("synthetic-refresh-{name}"), "id_token": format!("synthetic-id-{name}"), "account_id": format!("account-{name}")}, "retained": "unknown-field"})
        }
    };
    ImportAccount {
        provider,
        label: name.into(),
        format: if provider == Provider::Claude {
            ImportFormat::ClaudeCode
        } else {
            ImportFormat::Codex
        },
        content: content.to_string(),
    }
}

#[tokio::test]
async fn same_source_accounts_are_isolated_idempotent_encrypted_and_restartable() {
    let root = state_root("account-roundtrip");
    let deck = Deck::open(&root).unwrap();
    assert!(matches!(
        deck.import(key(Provider::Claude, "synthetic-key-a-same"))
            .await,
        Err(DeckError::VaultLocked)
    ));
    deck.unlock(PASSWORD.into()).await.unwrap();
    let a = deck
        .import(key(Provider::Claude, "synthetic-key-a-same"))
        .await
        .unwrap();
    let b = deck
        .import(key(Provider::Claude, "synthetic-key-b-same"))
        .await
        .unwrap();
    let again = deck
        .import(key(Provider::Claude, "synthetic-key-b-same"))
        .await
        .unwrap();
    let other_provider = deck
        .import(key(Provider::Openai, "synthetic-key-b-same"))
        .await
        .unwrap();
    assert_ne!(a.account_id, b.account_id);
    assert_ne!(b.account_id, other_provider.account_id);
    assert_eq!(b.account_id, again.account_id);
    assert_eq!(b.credential_set_id, again.credential_set_id);
    assert_eq!(again.outcome, "updated");
    deck.create_connection(CreateConnection {
        credential_set_id: b.credential_set_id.clone(),
        label: "测试连接".into(),
        model: "synthetic-model".into(),
        base_url: "https://example.test/v1".into(),
    })
    .await
    .unwrap();
    let settings = Settings {
        favorite_providers: vec![Provider::Openai],
        float_enabled: true,
    };
    deck.save_settings(settings.clone()).await.unwrap();
    let snapshot = deck.snapshot().await.unwrap();
    assert_eq!(snapshot.accounts.len(), 3);
    assert_eq!(snapshot.connections[0].account_id, b.account_id);
    assert!(!snapshot.connections[0].verified);
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(!json.contains("synthetic-key"));
    assert!(!json.contains("ciphertext"));
    assert!(!json.contains("fingerprint"));
    deck.shutdown().await.unwrap();
    for entry in std::fs::read_dir(&root).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        for secret in ["synthetic-key-a-same", "synthetic-key-b-same", PASSWORD] {
            assert!(
                !bytes
                    .windows(secret.len())
                    .any(|chunk| chunk == secret.as_bytes())
            );
        }
    }
    drop(deck);
    let deck = Deck::open(&root).unwrap();
    let snapshot = deck.snapshot().await.unwrap();
    assert!(snapshot.vault_configured);
    assert!(!snapshot.vault_unlocked);
    assert_eq!(snapshot.accounts.len(), 3);
    assert_eq!(snapshot.settings, settings);
    assert_eq!(
        snapshot
            .credentials
            .iter()
            .find(|item| item.id == b.credential_set_id)
            .unwrap()
            .version,
        2
    );
    assert!(matches!(
        deck.unlock("wrong-password-long-enough".into()).await,
        Err(DeckError::VaultAuthentication)
    ));
    deck.unlock(PASSWORD.into()).await.unwrap();
    assert_eq!(
        deck.import(key(Provider::Claude, "synthetic-key-b-same"))
            .await
            .unwrap()
            .account_id,
        b.account_id
    );
    deck.shutdown().await.unwrap();
}

#[tokio::test]
async fn complete_oauth_groups_and_unknown_identity_stay_separate() {
    let root = state_root("oauth-groups");
    let deck = Deck::open(&root).unwrap();
    deck.unlock(PASSWORD.into()).await.unwrap();
    for provider in [Provider::Claude, Provider::Openai] {
        let a = deck.import(oauth(provider, "A")).await.unwrap();
        let b = deck.import(oauth(provider, "B")).await.unwrap();
        let again = deck.import(oauth(provider, "B")).await.unwrap();
        assert_ne!(a.account_id, b.account_id);
        assert_eq!(b.account_id, again.account_id);
    }
    let mut invalid = oauth(Provider::Claude, "incomplete");
    invalid.content = r#"{"claudeAiOauth":{"accessToken":"synthetic-only-access"}}"#.into();
    assert!(matches!(
        deck.import(invalid).await,
        Err(DeckError::Invalid(_))
    ));
    let mut wrong_provider = oauth(Provider::Claude, "wrong-provider");
    wrong_provider.provider = Provider::Openai;
    assert!(matches!(
        deck.import(wrong_provider).await,
        Err(DeckError::Invalid(_))
    ));
    let snapshot = deck.snapshot().await.unwrap();
    assert_eq!(snapshot.accounts.len(), 4);
    assert!(
        snapshot
            .accounts
            .iter()
            .all(|account| account.identity_status == "unverified")
    );
    assert_eq!(
        snapshot
            .credentials
            .iter()
            .filter(|credential| credential.expires_at == Some(2000000000))
            .count(),
        2
    );
    deck.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_imports_deduplicate_and_shutdown_rejects_new_work() {
    let root = state_root("concurrent-import");
    let deck = Arc::new(Deck::open(&root).unwrap());
    deck.unlock(PASSWORD.into()).await.unwrap();
    let mut work = Vec::new();
    for _ in 0..8 {
        let deck = deck.clone();
        work.push(tokio::spawn(async move {
            deck.import(key(Provider::Openai, "synthetic-concurrent-key"))
                .await
                .unwrap()
        }));
    }
    let mut ids = Vec::new();
    for task in work {
        ids.push(task.await.unwrap().account_id);
    }
    assert!(ids.iter().all(|id| *id == ids[0]));
    assert_eq!(deck.snapshot().await.unwrap().accounts.len(), 1);
    deck.check_storage().await.unwrap();
    assert_eq!(deck.snapshot().await.unwrap().tasks[0].state, "succeeded");
    deck.shutdown().await.unwrap();
    assert!(matches!(
        deck.snapshot().await,
        Err(DeckError::ShuttingDown)
    ));
}

#[tokio::test]
async fn restart_marks_unfinished_tasks_interrupted_without_replaying() {
    let root = state_root("task-recovery");
    let deck = Deck::open(&root).unwrap();
    deck.shutdown().await.unwrap();
    drop(deck);
    let db = Connection::open(root.join("deck.sqlite3")).unwrap();
    for state in ["queued", "running", "waiting_user", "paused", "succeeded"] {
        db.execute(
            "INSERT INTO tasks VALUES (?1, 'storage_check', ?2, 1, NULL, 'synthetic')",
            params![state, state],
        )
        .unwrap();
    }
    drop(db);
    let deck = Deck::open(&root).unwrap();
    let snapshot = deck.snapshot().await.unwrap();
    assert_eq!(
        snapshot
            .tasks
            .iter()
            .filter(|task| task.state == "interrupted")
            .count(),
        4
    );
    assert_eq!(
        snapshot
            .tasks
            .iter()
            .find(|task| task.id == "succeeded")
            .unwrap()
            .state,
        "succeeded"
    );
    deck.shutdown().await.unwrap();
}

#[tokio::test]
async fn trust_boundaries_reject_invalid_connections_and_newer_databases() {
    let root = state_root("validation");
    let deck = Deck::open(&root).unwrap();
    deck.unlock(PASSWORD.into()).await.unwrap();
    let saved = deck
        .import(key(Provider::Claude, "synthetic-validation-key"))
        .await
        .unwrap();
    for base_url in [
        "https://user:secret@example.test",
        "https://example.test?key=secret",
        "file:///etc/passwd",
        "http://remote.test",
        "javascript:alert(1)",
    ] {
        let result = deck
            .create_connection(CreateConnection {
                credential_set_id: saved.credential_set_id.clone(),
                label: "test".into(),
                model: "test".into(),
                base_url: base_url.into(),
            })
            .await;
        assert!(matches!(result, Err(DeckError::Invalid(_))));
    }
    assert!(
        deck.create_connection(CreateConnection {
            credential_set_id: "missing".into(),
            label: "test".into(),
            model: "test".into(),
            base_url: "http://127.0.0.1:1234/v1".into()
        })
        .await
        .is_err()
    );
    assert!(
        deck.save_settings(Settings {
            favorite_providers: vec![Provider::Claude, Provider::Claude],
            float_enabled: false
        })
        .await
        .is_err()
    );
    assert!(deck.snapshot().await.unwrap().connections.is_empty());
    deck.shutdown().await.unwrap();
    drop(deck);
    Connection::open(root.join("deck.sqlite3"))
        .unwrap()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    assert!(matches!(Deck::open(&root), Err(DeckError::Invalid(_))));
}
