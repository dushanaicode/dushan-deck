CREATE TABLE vault_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    salt BLOB NOT NULL CHECK (length(salt) = 16),
    verifier BLOB NOT NULL
) STRICT;
CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL CHECK (provider IN ('claude', 'openai')),
    label TEXT NOT NULL,
    identity_status TEXT NOT NULL CHECK (identity_status = 'unverified'),
    created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE credential_sets (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id),
    fingerprint TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK (kind IN ('api_key', 'oauth')),
    source TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    expires_at INTEGER,
    ciphertext BLOB NOT NULL,
    UNIQUE (id, account_id)
) STRICT;
CREATE TABLE model_connections (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id),
    credential_set_id TEXT NOT NULL,
    label TEXT NOT NULL,
    model TEXT NOT NULL,
    base_url TEXT NOT NULL,
    verified INTEGER NOT NULL DEFAULT 0 CHECK (verified IN (0, 1)),
    FOREIGN KEY (credential_set_id, account_id) REFERENCES credential_sets(id, account_id)
) STRICT;
CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    value TEXT NOT NULL
) STRICT;
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'waiting_user', 'paused', 'succeeded', 'failed', 'cancelled', 'interrupted')),
    created_at INTEGER NOT NULL,
    finished_at INTEGER,
    message TEXT NOT NULL
) STRICT;
PRAGMA user_version = 1;
