CREATE TABLE accounts_expanded (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL CHECK (provider IN ('claude','openai','grok','zai','zhipu','kimi','deepseek','antigravity','cursor','cursor_agent')),
    label TEXT NOT NULL,
    identity_status TEXT NOT NULL CHECK (identity_status = 'unverified'),
    created_at INTEGER NOT NULL
) STRICT;
INSERT INTO accounts_expanded SELECT * FROM accounts;
DROP TABLE accounts;
ALTER TABLE accounts_expanded RENAME TO accounts;
CREATE TABLE quota_snapshots (
    account_id TEXT PRIMARY KEY REFERENCES accounts(id),
    result TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    retry_at INTEGER NOT NULL,
    failures INTEGER NOT NULL,
    credential_version INTEGER NOT NULL
) STRICT;
CREATE TABLE usage_sources (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id),
    harness TEXT NOT NULL CHECK (harness IN ('codex','claude_code','opencode','omp','kimi_code','grok_cli')),
    path TEXT NOT NULL,
    since INTEGER NOT NULL,
    UNIQUE (account_id, harness, path, since)
) STRICT;
PRAGMA user_version = 3;
