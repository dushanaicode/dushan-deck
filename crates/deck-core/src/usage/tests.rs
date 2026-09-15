use super::*;
use crate::{
    Deck,
    accounts::{ImportAccount, ImportFormat},
    catalog::Provider,
    settings::floating::FloatPreferences,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Mutex as StdMutex, atomic::AtomicBool},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn test_root() -> PathBuf {
    let base = PathBuf::from(std::env::var_os("DECK_TEST_ROOT").expect("explicit Temp test path"));
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert!(base.starts_with(workspace.join("Temp")));
    let path = base.join(format!("quota-contract-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}
fn token(marker: &str) -> String {
    format!("header.{}.signature", URL_SAFE_NO_PAD.encode(json!({"sub":"auth|user_fixture", "jti":marker, "https://api.openai.com/auth":{"chatgpt_account_id":"account-fixture"}, "https://api.openai.com/profile":{"email":"test@example.test"}}).to_string()))
}
fn payload(path: &str) -> Value {
    let path = path.split('?').next().unwrap();
    match path {
        "/api.anthropic.com/api/oauth/usage" => {
            json!({"five_hour":{"utilization":25,"resets_at":"2030-01-01T00:00:00Z"}})
        }
        "/api.anthropic.com/api/oauth/profile" => {
            json!({"account":{"uuid":"claude-fixture","email":"test@example.test"},"organization":{"organization_type":"claude_max","rate_limit_tier":"default_claude_max_20x"}})
        }
        "/chatgpt.com/backend-api/wham/usage" => {
            json!({"plan_type":"pro", "rate_limit":{"primary_window":{"limit_window_seconds":18000,"used_percent":20,"reset_at":2000000000}},"rate_limit_reset_credits":{"available_count":2}})
        }
        "/chatgpt.com/backend-api/subscriptions" => {
            json!({"subscription_plan":"pro_lite","active_until":"2030-01-01T00:00:00Z"})
        }
        "/chatgpt.com/backend-api/wham/profiles/me" => {
            json!({"stats":{"lifetime_tokens":1000,"daily_usage_buckets":[{"start_date":chrono::Local::now().format("%Y-%m-%d").to_string(),"tokens":100}]}})
        }
        "/chatgpt.com/backend-api/accounts/check/v4-2023-04-27" => {
            json!({"accounts":{"fixture":{"account":{"account_id":"account-fixture"},"entitlement":{"subscription_plan":"pro_lite","expires_at":"2030-02-01T00:00:00Z"}},"other":{"account":{"account_id":"not-this-account"},"entitlement":{"subscription_plan":"enterprise","expires_at":"2031-02-01T00:00:00Z"}}}})
        }
        "/api.anthropic.com/v1/organizations/usage_report/messages" => {
            json!({"has_more":false,"data":[{"starting_at":chrono::Utc::now().to_rfc3339(),"results":[{"model":"claude-admin-fixture","uncached_input_tokens":100,"cache_read_input_tokens":20,"cache_creation":{"ephemeral_5m_input_tokens":3},"output_tokens":30}]}]})
        }
        "/api.deepseek.com/user/balance" => {
            json!({"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"20.5"}]})
        }
        "/api.z.ai/api/monitor/usage/quota/limit"
        | "/bigmodel.cn/api/monitor/usage/quota/limit" => {
            json!({"data":{"limits":[{"type":"TOKENS_LIMIT","unit":3,"percentage":40,"nextResetTime":2000000000000_i64}]}})
        }
        "/api.z.ai/api/paas/v4/user" | "/open.bigmodel.cn/api/paas/v4/user" => {
            json!({"email":"test@example.test","plan":"Pro"})
        }
        "/api.z.ai/api/monitor/usage/model-usage"
        | "/bigmodel.cn/api/monitor/usage/model-usage" => {
            json!({"data":{"modelDataList":[{"modelName":"glm-fixture","tokensUsage":[100,200]}]}})
        }
        "/api.kimi.com/coding/v1/usages" => {
            json!({"usage":{"limit":100,"used":15},"user":{"email":"test@example.test","membership":{"level":"LEVEL_ADVANCED"}}})
        }
        "/cli-chat-proxy.grok.com/v1/billing" => {
            json!({"config":{"creditUsagePercent":50,"currentPeriod":{"end":"2030-01-01T00:00:00Z"}}})
        }
        "/cli-chat-proxy.grok.com/v1/user" => {
            json!({"email":"test@example.test","subscriptionTier":"supergrokheavy"})
        }
        "/grok.com/rest/tasks/usage" => {
            json!({"frequentUsage":1,"frequentLimit":10,"occasionalUsage":2,"occasionalLimit":10})
        }
        "/grok.com/rest/subscriptions" => {
            json!({"subscriptions":[{"status":"SUBSCRIPTION_STATUS_ACTIVE","tier":"SUBSCRIPTION_TIER_SUPER_GROK_HEAVY"}]})
        }
        "/cursor.com/api/usage-summary" => {
            json!({"totalSpend":20,"totalLimit":100,"membershipType":"Pro"})
        }
        "/api2.cursor.sh/auth/exchange_user_api_key" => {
            json!({"accessToken":"synthetic-cursor-exchanged"})
        }
        "/api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage" => {
            json!({"planUsage":{"totalPercentUsed":5},"billingCycleEnd":2000000000000_i64})
        }
        "/api2.cursor.sh/aiserver.v1.DashboardService/GetMe" => {
            json!({"email":"test@example.test"})
        }
        "/api2.cursor.sh/aiserver.v1.DashboardService/GetPlanInfo" => {
            json!({"planInfo":{"planName":"Pro"}})
        }
        "/cloudcode-pa.googleapis.com/v1internal:loadCodeAssist" => {
            json!({"cloudaicompanionProject":"project-fixture","paidTier":{"id":"g1-pro-tier"}})
        }
        "/daily-cloudcode-pa.sandbox.googleapis.com/v1internal:retrieveUserQuotaSummary" => {
            json!({"groups":[{"buckets":[{"bucketId":"gemini-weekly","remainingFraction":0.4}]}]})
        }
        "/auth.openai.com/oauth/token"
        | "/platform.claude.com/v1/oauth/token"
        | "/auth.x.ai/oauth2/token"
        | "/api2.cursor.sh/oauth/token"
        | "/oauth2.googleapis.com/token" => {
            json!({"access_token":token("rotated"),"refresh_token":"synthetic-refresh-rotated","id_token":token("rotated-id"),"expires_in":3600})
        }
        _ => panic!("Unexpected provider route: {path}"),
    }
}
struct Server {
    origin: String,
    calls: Arc<StdMutex<Vec<String>>>,
    fail: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(StdMutex::new(Vec::new()));
        let fail = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (requests, failure, paused, arrived, released) = (
            calls.clone(),
            fail.clone(),
            pause.clone(),
            entered.clone(),
            release.clone(),
        );
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let (requests, failure, paused, arrived, released) = (
                    requests.clone(),
                    failure.clone(),
                    paused.clone(),
                    arrived.clone(),
                    released.clone(),
                );
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let mut buf = [0; 4096];
                    loop {
                        let length = stream.read(&mut buf).await.unwrap();
                        if length == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&buf[..length]);
                        if let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&bytes[..end]);
                            let length: usize = headers
                                .lines()
                                .find_map(|line| {
                                    line.to_lowercase()
                                        .strip_prefix("content-length: ")
                                        .and_then(|value| value.parse().ok())
                                })
                                .unwrap_or(0);
                            if bytes.len() >= end + 4 + length {
                                break;
                            }
                        }
                    }
                    let request = String::from_utf8(bytes).unwrap();
                    let path = request
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .to_owned();
                    requests.lock().unwrap().push(request);
                    if paused.load(Ordering::Acquire) {
                        arrived.notify_one();
                        released.notified().await;
                    }
                    let (status, value) = if failure.load(Ordering::Acquire) {
                        (429, json!({"error":"synthetic limited"}))
                    } else {
                        (200, payload(&path))
                    };
                    let body = value.to_string();
                    let response = format!(
                        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nRetry-After: 120\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });
        Self {
            origin,
            calls,
            fail,
            pause,
            entered,
            release,
            task,
        }
    }
    fn client(&self) -> QuotaClient {
        let mut client = QuotaClient::new(false).unwrap();
        client.test_origin = Some(self.origin.clone());
        client
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn imported(store: &Storage, provider: Provider, expiry: i64) -> Credential {
    let result = store.import(ImportAccount { provider, label: "fixture".into(), format: ImportFormat::QuotaJson,
        content: json!({"provider":provider,"access":token("before"),"refresh":"synthetic-refresh","api_key":if matches!(provider, Provider::Zai | Provider::Zhipu | Provider::Kimi | Provider::Deepseek | Provider::CursorAgent) { "synthetic-api-key" } else { "" },"expiry":expiry,"client_id":"synthetic-client","client_secret":"synthetic-client-secret","unknown":"retained"}).to_string() }).unwrap();
    store.credential(&result.credential_set_id).unwrap()
}

#[tokio::test]
async fn all_provider_http_routes_and_remote_usage_are_connected() {
    let server = Server::start().await;
    let client = server.client();
    let store = Storage::open(&test_root()).unwrap();
    store
        .unlock(zeroize::Zeroizing::new("synthetic-test-password".into()))
        .unwrap();
    for provider in Provider::ALL {
        let credential = imported(&store, provider, 2000000000);
        let result = client.quota(&credential).await.unwrap();
        assert!(result.ok, "{provider:?}");
        assert!(!result.windows.is_empty());
        if matches!(provider, Provider::Openai | Provider::Zai | Provider::Zhipu) {
            assert!(
                !remote::fetch(&client, &credential)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
    }
    let admin = store
        .import(ImportAccount {
            provider: Provider::Claude,
            label: "admin".into(),
            format: ImportFormat::ApiKey,
            content: "synthetic-admin-key".into(),
        })
        .unwrap();
    let admin = store.credential(&admin.credential_set_id).unwrap();
    let usage = remote::fetch(&client, &admin).await.unwrap();
    assert!(usage.iter().all(|row| row.total_tokens == Some(153)));
    assert!(!usage.is_empty());
    let requests = server.calls.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|value| value.contains("ChatGPT-Account-Id".to_lowercase().as_str()))
    );
    assert!(
        requests
            .iter()
            .any(|value| value.contains("WorkosCursorSessionToken=user_fixture%3A%3A"))
    );
    assert!(
        requests
            .iter()
            .any(|value| value.contains("connect-protocol-version: 1"))
    );
    assert!(
        requests
            .iter()
            .all(|value| !value.contains("/consume") && !value.contains("/chat/completions"))
    );
}

#[tokio::test]
async fn concurrent_refresh_is_shared_and_rate_limit_retains_the_last_success() {
    let server = Server::start().await;
    let mut deck = Deck::open(&test_root()).unwrap();
    deck.quota_client = Arc::new(server.client());
    deck.unlock("synthetic-test-password".into()).await.unwrap();
    deck.import(ImportAccount {
        provider: Provider::Deepseek,
        label: "fixture".into(),
        format: ImportFormat::ApiKey,
        content: "synthetic-key".into(),
    })
    .await
    .unwrap();
    let deck = Arc::new(deck);
    let mut jobs = vec![];
    for _ in 0..8 {
        let deck = deck.clone();
        jobs.push(tokio::spawn(async move {
            deck.refresh_quotas(false).await.unwrap()
        }));
    }
    for job in jobs {
        assert_eq!(
            job.await.unwrap().results[0].windows[0].text.as_deref(),
            Some("¥20.5")
        );
    }
    assert_eq!(server.calls.lock().unwrap().len(), 1);
    server.fail.store(true, Ordering::Release);
    let stale = deck.refresh_quotas(true).await.unwrap();
    assert_eq!(stale.state, "stale");
    assert!(stale.results[0].notice.contains("429"));
    assert_eq!(stale.results[0].windows[0].text.as_deref(), Some("¥20.5"));
    deck.refresh_quotas(true).await.unwrap();
    assert_eq!(server.calls.lock().unwrap().len(), 2);
    deck.shutdown().await.unwrap();
}

#[tokio::test]
async fn refresh_rotation_is_persisted_as_one_group_and_never_writes_the_client() {
    let root = test_root();
    let client_file = root.join("client-b.json");
    std::fs::write(&client_file, "synthetic-client-account-B").unwrap();
    let server = Server::start().await;
    let store = Arc::new(Storage::open(&root).unwrap());
    store
        .unlock(zeroize::Zeroizing::new("synthetic-test-password".into()))
        .unwrap();
    let credential = imported(&store, Provider::Openai, 1);
    let id = credential.credential_id.clone();
    let rotated = renew(store.clone(), &server.client(), credential)
        .await
        .unwrap();
    assert_eq!(rotated.refresh(), "synthetic-refresh-rotated");
    assert_eq!(rotated.document["secret"]["unknown"], "retained");
    assert_eq!(
        store.credential(&id).unwrap().refresh(),
        "synthetic-refresh-rotated"
    );
    assert_eq!(
        std::fs::read_to_string(client_file).unwrap(),
        "synthetic-client-account-B"
    );
    assert!(
        store
            .snapshot()
            .unwrap()
            .tasks
            .iter()
            .all(|task| task.state == "succeeded")
    );
}

#[tokio::test]
async fn exit_waits_for_the_running_http_operation_and_finishes_its_write() {
    let server = Server::start().await;
    server.pause.store(true, Ordering::Release);
    let mut deck = Deck::open(&test_root()).unwrap();
    deck.quota_client = Arc::new(server.client());
    deck.unlock("synthetic-test-password".into()).await.unwrap();
    deck.import(ImportAccount {
        provider: Provider::Deepseek,
        label: "fixture".into(),
        format: ImportFormat::ApiKey,
        content: "synthetic-key".into(),
    })
    .await
    .unwrap();
    let deck = Arc::new(deck);
    let querying = {
        let deck = deck.clone();
        tokio::spawn(async move { deck.refresh_quotas(true).await })
    };
    server.entered.notified().await;
    let mut exit = {
        let deck = deck.clone();
        tokio::spawn(async move { deck.shutdown().await })
    };
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut exit)
            .await
            .is_err()
    );
    server.release.notify_one();
    querying.await.unwrap().unwrap();
    exit.await.unwrap().unwrap();
}

fn jsonl(path: &std::path::Path, rows: &[Value]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        rows.iter()
            .map(|row| row.to_string() + "\n")
            .collect::<String>(),
    )
    .unwrap();
}
#[test]
fn all_six_local_sources_deduplicate_and_respect_account_switches_and_pins() {
    use types::{LocalHarness, UsageSource};
    let root = test_root();
    let now = crate::now();
    let earlier = now - 600;
    let later = now - 300;
    let iso = |time| {
        chrono::DateTime::<chrono::Utc>::from_timestamp(time, 0)
            .unwrap()
            .to_rfc3339()
    };
    let codex = root.join("codex");
    let first = json!({"type":"event_msg","timestamp":iso(earlier),"payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":50,"reasoning_output_tokens":10}}}});
    let second = json!({"type":"event_msg","timestamp":iso(later),"payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":150,"cached_input_tokens":30,"output_tokens":90,"reasoning_output_tokens":20}}}});
    let entries = [
        json!({"type":"turn_context","payload":{"model":"gpt-fixture"}}),
        first.clone(),
        first,
        second,
    ];
    jsonl(&codex.join("sessions/rollout.jsonl"), &entries);
    jsonl(&codex.join("archived_sessions/rollout.jsonl"), &entries);
    let claude = root.join("claude");
    let part = |output| json!({"type":"assistant","timestamp":iso(earlier),"requestId":"request-a","message":{"id":"message-a","model":"claude-fixture","usage":{"input_tokens":100,"cache_read_input_tokens":10,"cache_creation_input_tokens":5,"output_tokens":output}}});
    jsonl(&claude.join("project/session.jsonl"), &[part(20), part(40)]);
    let kimi = root.join("kimi");
    jsonl(
        &kimi.join("session/wire.jsonl"),
        &[
            json!({"type":"usage.record","time":earlier*1000,"model":"kimi-fixture","usage":{"inputOther":10,"inputCacheRead":2,"inputCacheCreation":3,"output":5}}),
        ],
    );
    let omp = root.join("omp");
    jsonl(
        &omp.join("session.jsonl"),
        &[
            json!({"type":"credential_pin","provider":"openai","hash":"known-full-pin"}),
            json!({"type":"message","id":"one","timestamp":iso(earlier),"message":{"provider":"openai","model":"gpt-fixture","usage":{"input":50,"cacheRead":5,"output":10,"totalTokens":65}}}),
            json!({"type":"credential_pin","provider":"openai","hash":"unmatched-pin"}),
            json!({"type":"message","id":"two","timestamp":iso(later),"message":{"provider":"openai","model":"gpt-fixture","usage":{"input":500,"output":100}}}),
        ],
    );
    let grok = root.join("grok");
    std::fs::create_dir_all(&grok).unwrap();
    std::fs::write(grok.join("signals.json"),json!({"totalTokensBeforeCompaction":500,"contextTokensUsed":80,"primaryModelId":"grok-fixture"}).to_string()).unwrap();
    let opencode = root.join("opencode.db");
    let db = rusqlite::Connection::open(&opencode).unwrap();
    db.execute_batch("CREATE TABLE message (id TEXT PRIMARY KEY,time_created INTEGER,data TEXT)")
        .unwrap();
    db.execute("INSERT INTO message VALUES ('one',?1,?2)",rusqlite::params![earlier*1000,json!({"providerID":"anthropic","modelID":"claude-fixture","tokens":{"input":40,"output":10,"cache":{"read":5,"write":2},"total":57}}).to_string()]).unwrap();
    drop(db);
    let mut sources = vec![];
    let mut add = |account: &str, harness, path: &std::path::Path, since| {
        sources.push(UsageSource {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account.into(),
            harness,
            path: path.to_string_lossy().into_owned(),
            since,
        });
    };
    add("openai-a", LocalHarness::Codex, &codex, earlier - 1);
    add("openai-b", LocalHarness::Codex, &codex, later);
    add("claude", LocalHarness::ClaudeCode, &claude, earlier - 1);
    add("claude", LocalHarness::Opencode, &opencode, earlier - 1);
    add("kimi", LocalHarness::KimiCode, &kimi, earlier - 1);
    add("grok", LocalHarness::GrokCli, &grok, earlier - 1);
    add("openai-a", LocalHarness::Omp, &omp, earlier - 1);
    let providers = HashMap::from([
        ("openai-a".into(), Provider::Openai),
        ("openai-b".into(), Provider::Openai),
        ("claude".into(), Provider::Claude),
        ("kimi".into(), Provider::Kimi),
        ("grok".into(), Provider::Grok),
    ]);
    let pins = HashMap::from([(
        ("openai".into(), "known-full-pin".into()),
        vec!["openai-a".into()],
    )]);
    let report = local::collect(&sources, &providers, &pins, now + 1);
    let total = |account: &str, harness: &str| {
        report.rows[account]
            .iter()
            .find(|row| {
                row.harness == harness && row.period == crate::settings::floating::UsagePeriod::All
            })
            .unwrap()
            .total_tokens
            .unwrap()
    };
    assert_eq!(total("openai-a", "codex"), 150);
    assert_eq!(total("openai-b", "codex"), 90);
    assert_eq!(total("claude", "claude_code"), 155);
    assert_eq!(total("claude", "opencode"), 57);
    assert_eq!(total("kimi", "kimi_code"), 20);
    assert_eq!(total("grok", "grok_cli"), 580);
    assert_eq!(total("openai-a", "omp"), 65);
    assert!(report.warnings["openai-a"].contains("1 条记录无法"));
}

#[test]
fn float_preferences_and_migration_preserve_existing_accounts_and_reject_unsafe_images() {
    let root = test_root();
    let db = rusqlite::Connection::open(root.join("deck.sqlite3")).unwrap();
    db.execute_batch(include_str!("../storage/schema.sql"))
        .unwrap();
    db.execute(
        "INSERT INTO settings VALUES (1,?1)",
        [serde_json::to_string(&crate::settings::Settings::default()).unwrap()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO accounts VALUES ('existing','claude','existing','unverified',1)",
        [],
    )
    .unwrap();
    db.execute("INSERT INTO credential_sets VALUES ('credential','existing','fingerprint','api_key','manual_key',1,NULL,x'010203')",[]).unwrap();
    db.execute("INSERT INTO model_connections VALUES ('connection','existing','credential','retained','model','https://example.test',0)",[]).unwrap();
    drop(db);
    let store = Storage::open(&root).unwrap();
    assert_eq!(store.snapshot().unwrap().accounts[0].id, "existing");
    assert_eq!(store.snapshot().unwrap().credentials[0].id, "credential");
    assert_eq!(
        store.snapshot().unwrap().connections[0].credential_set_id,
        "credential"
    );
    let mut preferences = FloatPreferences {
        alpha: 63,
        on_top: false,
        bg_blur: 15,
        ..FloatPreferences::default()
    };
    store.save_float_preferences(preferences.clone()).unwrap();
    store.save_float_size(460, 720).unwrap();
    assert!(
        store
            .save_float_background(Some("data:image/svg+xml;base64,PHN2Zz4=".into()))
            .is_err()
    );
    preferences.alpha = 1;
    assert!(store.save_float_preferences(preferences).is_err());
    drop(store);
    let state = Storage::open(&root).unwrap().float_state().unwrap();
    assert_eq!(state.preferences.alpha, 63);
    assert_eq!((state.width, state.height), (460, 720));
    assert!(!state.preferences.on_top);
}

#[tokio::test]
async fn each_oauth_provider_uses_its_own_refresh_contract() {
    let server = Server::start().await;
    let client = server.client();
    let store = Storage::open(&test_root()).unwrap();
    store
        .unlock(zeroize::Zeroizing::new("synthetic-test-password".into()))
        .unwrap();
    for provider in [
        Provider::Openai,
        Provider::Claude,
        Provider::Grok,
        Provider::Cursor,
        Provider::Antigravity,
    ] {
        let mut credential = imported(&store, provider, 1);
        if provider == Provider::Antigravity {
            let fields = credential.document["secret"].as_object_mut().unwrap();
            fields.remove("client_id");
            fields.remove("client_secret");
        }
        let response = client.refresh(&credential).await.unwrap();
        assert_eq!(response["refresh_token"], "synthetic-refresh-rotated");
    }
    let calls = server.calls.lock().unwrap();
    assert_eq!(calls.len(), 5);
    assert!(
        calls
            .iter()
            .any(|request| request.contains("/auth.x.ai/oauth2/token")
                && request.contains("application/x-www-form-urlencoded"))
    );
    assert!(
        calls
            .iter()
            .any(|request| request.contains("/oauth2.googleapis.com/token")
                && request.contains("client_id="))
    );
}

#[test]
fn primary_protocols_preserve_unknown_values_and_percentages() {
    for (provider, payload, expected) in [
        (
            crate::catalog::Provider::Claude,
            serde_json::json!({"five_hour":{"utilization":25,"resets_at":"2030-01-01T00:00:00Z"}}),
            75.,
        ),
        (
            crate::catalog::Provider::Openai,
            serde_json::json!({"rate_limit":{"primary_window":{"limit_window_seconds":18000,"used_percent":20,"reset_at":2000000000}}}),
            80.,
        ),
        (
            crate::catalog::Provider::Zai,
            serde_json::json!({"data":{"limits":[{"type":"TOKENS_LIMIT","unit":3,"percentage":40}]}}),
            60.,
        ),
        (
            crate::catalog::Provider::Zhipu,
            serde_json::json!({"limits":[{"type":"CREDIT_LIMIT","unit":6,"percentage":35}]}),
            65.,
        ),
        (
            crate::catalog::Provider::Kimi,
            serde_json::json!({"usage":{"limit":100,"used":15}}),
            85.,
        ),
        (
            crate::catalog::Provider::Grok,
            serde_json::json!({"config":{"creditUsagePercent":50}}),
            50.,
        ),
        (
            crate::catalog::Provider::Cursor,
            serde_json::json!({"totalSpend":20,"totalLimit":100}),
            80.,
        ),
        (
            crate::catalog::Provider::CursorAgent,
            serde_json::json!({"planUsage":{"totalPercentUsed":5}}),
            95.,
        ),
        (
            crate::catalog::Provider::Antigravity,
            serde_json::json!({"groups":[{"buckets":[{"bucketId":"gemini-weekly","remainingFraction":0.4}]}]}),
            40.,
        ),
    ] {
        let mut output = QuotaAccount::empty("fixture".into(), provider, "fixture".into());
        providers::parse_primary(provider, &payload, &mut output).unwrap();
        assert_eq!(output.windows[0].remaining_percent, Some(expected));
    }
    let mut output = QuotaAccount::empty(
        "fixture".into(),
        crate::catalog::Provider::Deepseek,
        "fixture".into(),
    );
    providers::parse_primary(crate::catalog::Provider::Deepseek,&serde_json::json!({"balance_infos":[{"currency":"CNY","total_balance":"20.5"}],"is_available":true}),&mut output).unwrap();
    assert_eq!(output.windows[0].text.as_deref(), Some("¥20.5"));
}
