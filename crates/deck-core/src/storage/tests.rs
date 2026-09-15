use super::*;
use crate::{accounts::ImportFormat, catalog::Provider};

#[test]
fn stored_group_preserves_unknown_fields_and_authenticates_its_record() {
    let root = PathBuf::from(std::env::var_os("DECK_TEST_ROOT").unwrap())
        .join(format!("vault-{}", Uuid::new_v4()));
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert!(root.starts_with(workspace.join("Temp")));
    let store = Storage::open(&root).unwrap();
    store
        .unlock(Zeroizing::new("synthetic-test-password".into()))
        .unwrap();
    let document = serde_json::json!({ "tokens": { "access_token": "synthetic-access", "refresh_token": "synthetic-refresh", "id_token": "synthetic-id", "account_id": "claimed-account" }, "unknown": {"preserve": true} });
    let mut saved_id = None;
    for _ in 0..2 {
        let saved = store
            .import(ImportAccount {
                provider: Provider::Openai,
                label: "test".into(),
                format: ImportFormat::Codex,
                content: document.to_string(),
            })
            .unwrap();
        if let Some(id) = &saved_id {
            assert_eq!(*id, saved.account_id);
        }
        saved_id = Some(saved.account_id.clone());
        let db = store.connect().unwrap();
        let (encrypted, version): (Vec<u8>, i64) = db
            .query_row(
                "SELECT ciphertext, version FROM credential_sets WHERE id = ?1",
                [&saved.credential_set_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let guard = store.key.lock().unwrap();
        let key = guard.as_ref().unwrap();
        let aad = format!("{}/{}/{version}", saved.account_id, saved.credential_set_id);
        let bytes = vault::open(key, &encrypted, aad.as_bytes()).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            document
        );
        assert!(vault::open(key, &encrypted, b"another-account/another-session/1").is_err());
        let mut tampered = encrypted.clone();
        tampered[15] ^= 1;
        assert!(vault::open(key, &tampered, aad.as_bytes()).is_err());
    }
    store.shutdown().unwrap();
}
