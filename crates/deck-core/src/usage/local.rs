use super::{
    providers::{text, timestamp},
    types::{LocalHarness, TokenBreakdown, UsageModel, UsageRow, UsageSource},
};
use crate::{
    catalog::Provider,
    error::{DeckError, Result},
    settings::floating::UsagePeriod,
};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::{Duration, Instant, UNIX_EPOCH},
};

#[derive(Clone)]
pub(super) struct Event {
    pub at: i64,
    pub provider: Provider,
    pub model: String,
    pub total: u64,
    pub breakdown: Option<TokenBreakdown>,
    pub id: String,
    pub pin: Option<(String, String)>,
}
pub(super) struct LocalReport {
    pub rows: HashMap<String, Vec<UsageRow>>,
    pub warnings: HashMap<String, String>,
    pub failed: HashMap<String, std::collections::BTreeSet<String>>,
}
fn count(value: &Value) -> Option<u64> {
    value.as_u64().filter(|value| *value <= 1_000_000_000_000)
}
fn sum(values: impl Iterator<Item = Option<u64>>) -> u64 {
    values.map(Option::unwrap_or_default).sum()
}
fn tokens(value: &Value, keys: [&str; 5]) -> TokenBreakdown {
    TokenBreakdown {
        input: count(&value[keys[0]]),
        cached: count(&value[keys[1]]),
        cache_write: count(&value[keys[2]]),
        output: count(&value[keys[3]]),
        reasoning: count(&value[keys[4]]),
    }
}
fn total(value: &TokenBreakdown) -> u64 {
    sum([
        value.input,
        value.cached,
        value.cache_write,
        value.output,
        value.reasoning,
    ]
    .into_iter())
}
pub(super) fn provider(value: &str) -> Option<Provider> {
    Some(match value {
        "openai" | "openai-codex" | "codex" => Provider::Openai,
        "xai" | "xai-oauth" | "grok" => Provider::Grok,
        "anthropic" | "claude" => Provider::Claude,
        "kimi" | "kimi-code" | "kimi-for-coding" => Provider::Kimi,
        "zai" | "zai-coding-plan" => Provider::Zai,
        "zhipu-coding-plan" | "zhipuai-coding-plan" | "zhipu" => Provider::Zhipu,
        "deepseek" => Provider::Deepseek,
        "cursor" => Provider::CursorAgent,
        _ => return None,
    })
}
fn merge_counter(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    Some(left? + right?)
}
fn merge(left: &mut TokenBreakdown, right: &TokenBreakdown) {
    left.input = merge_counter(left.input, right.input);
    left.cached = merge_counter(left.cached, right.cached);
    left.cache_write = merge_counter(left.cache_write, right.cache_write);
    left.output = merge_counter(left.output, right.output);
    left.reasoning = merge_counter(left.reasoning, right.reasoning);
}
pub(super) fn rows(
    events: &[Event],
    harness: &str,
    label: &str,
    source: &str,
    now: i64,
) -> Vec<UsageRow> {
    let mut output = vec![];
    for (period, days, period_label) in [
        (UsagePeriod::Day, Some(1), "近 1 天"),
        (UsagePeriod::ThreeDays, Some(3), "近 3 天"),
        (UsagePeriod::Week, Some(7), "近 7 天"),
        (UsagePeriod::Month, Some(30), "近 30 天"),
        (UsagePeriod::All, None, "累计"),
    ] {
        let selected: Vec<_> = events
            .iter()
            .filter(|event| {
                event.at <= now && days.is_none_or(|days| event.at >= now - days * 86400)
            })
            .collect();
        if selected.is_empty() {
            continue;
        }
        let mut models: BTreeMap<String, UsageModel> = BTreeMap::new();
        let mut overall = selected[0].breakdown.clone();
        for (index, event) in selected.iter().enumerate() {
            let entry = models
                .entry(event.model.clone())
                .or_insert_with(|| UsageModel {
                    name: event.model.clone(),
                    total_tokens: Some(0),
                    breakdown: event.breakdown.clone(),
                });
            if entry.total_tokens != Some(0) {
                match (&mut entry.breakdown, &event.breakdown) {
                    (Some(left), Some(right)) => merge(left, right),
                    _ => entry.breakdown = None,
                }
            }
            entry.total_tokens = Some(entry.total_tokens.unwrap_or(0) + event.total);
            if index > 0 {
                match (&mut overall, &event.breakdown) {
                    (Some(left), Some(right)) => merge(left, right),
                    _ => overall = None,
                }
            }
        }
        let mut models: Vec<_> = models.into_values().collect();
        models.sort_by_key(|model| std::cmp::Reverse(model.total_tokens));
        output.push(UsageRow {
            source: source.into(),
            label: format!("{label} · {period_label}"),
            period,
            harness: harness.into(),
            harness_label: label.into(),
            total_tokens: Some(selected.iter().map(|event| event.total).sum()),
            used: None,
            total: None,
            unit: String::new(),
            breakdown: overall,
            models,
        });
    }
    output
}
pub(super) fn collect(
    sources: &[UsageSource],
    account_providers: &HashMap<String, Provider>,
    pin_owners: &HashMap<(String, String), Vec<String>>,
    now: i64,
) -> LocalReport {
    let mut groups: BTreeMap<(String, String), Vec<&UsageSource>> = BTreeMap::new();
    for source in sources {
        groups
            .entry((source.harness.key().into(), source.path.clone()))
            .or_default()
            .push(source);
    }
    let mut report = LocalReport {
        rows: HashMap::new(),
        warnings: HashMap::new(),
        failed: HashMap::new(),
    };
    let mut unique: BTreeMap<(String, String), (Option<String>, Event)> = BTreeMap::new();
    for ((_, path), timeline) in groups {
        let harness = &timeline[0].harness;
        match scan(Path::new(&path), harness) {
            Ok((events, invalid)) => {
                let mut unassigned = 0;
                for event in events {
                    let owner = if let Some(pin) = &event.pin {
                        pin_owners.get(pin).and_then(|accounts| {
                            timeline
                                .iter()
                                .filter(|source| {
                                    source.since <= event.at
                                        && accounts.contains(&source.account_id)
                                })
                                .max_by_key(|source| source.since)
                                .map(|source| source.account_id.clone())
                        })
                    } else {
                        timeline
                            .iter()
                            .filter(|source| {
                                source.since <= event.at
                                    && account_providers.get(&source.account_id)
                                        == Some(&event.provider)
                            })
                            .max_by_key(|source| source.since)
                            .map(|source| source.account_id.clone())
                    };
                    if let Some(owner) = owner {
                        let key = (harness.key().to_owned(), event.id.clone());
                        if let Some((previous_owner, previous_event)) = unique.get_mut(&key) {
                            if previous_owner.as_ref() != Some(&owner) {
                                if let Some(previous) = previous_owner.take() {
                                    report
                                        .warnings
                                        .entry(previous)
                                        .or_default()
                                        .push_str("重叠来源的账号归属冲突，相关记录未计入；");
                                }
                                report
                                    .warnings
                                    .entry(owner)
                                    .or_default()
                                    .push_str("重叠来源的账号归属冲突，相关记录未计入；");
                            } else if event.total > previous_event.total {
                                *previous_event = event;
                            }
                        } else {
                            unique.insert(key, (Some(owner), event));
                        }
                    } else {
                        unassigned += 1;
                    }
                }
                if invalid > 0 || unassigned > 0 {
                    for source in &timeline {
                        report.warnings.entry(source.account_id.clone()).or_default().push_str(&format!("{}：{invalid} 条记录格式异常，{unassigned} 条记录无法按凭据指纹或切号时间归属，未计入账号；", harness.label()));
                    }
                }
            }
            Err(error) => {
                for source in &timeline {
                    report
                        .failed
                        .entry(source.account_id.clone())
                        .or_default()
                        .insert(harness.key().into());
                    report
                        .warnings
                        .entry(source.account_id.clone())
                        .or_default()
                        .push_str(&format!("{}：{error}；", harness.label()));
                }
            }
        }
    }
    let mut grouped: BTreeMap<(String, String), Vec<Event>> = BTreeMap::new();
    for ((harness, _), (owner, event)) in unique {
        if let Some(owner) = owner {
            grouped.entry((owner, harness)).or_default().push(event);
        }
    }
    for ((account, harness), events) in grouped {
        let label = sources
            .iter()
            .find(|source| source.harness.key() == harness)
            .expect("existing source")
            .harness
            .label();
        report
            .rows
            .entry(account)
            .or_default()
            .extend(rows(&events, &harness, label, "local", now));
    }
    report
}
fn files(root: &Path, harness: &LocalHarness, started: Instant) -> Result<Vec<PathBuf>> {
    let mut dirs = vec![root.to_owned()];
    let mut output = vec![];
    while let Some(directory) = dirs.pop() {
        if started.elapsed() > Duration::from_secs(5) {
            return Err(DeckError::Invalid("用量扫描超过 5 秒，请缩小来源目录"));
        }
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                dirs.push(entry.path());
            } else if kind.is_file() {
                let path = entry.path();
                let matches = match harness {
                    LocalHarness::GrokCli => {
                        path.file_name().is_some_and(|name| name == "signals.json")
                    }
                    LocalHarness::KimiCode => {
                        path.file_name().is_some_and(|name| name == "wire.jsonl")
                    }
                    _ => path
                        .extension()
                        .is_some_and(|extension| extension == "jsonl"),
                };
                if matches {
                    output.push(path);
                }
            }
            if output.len() + dirs.len() > 10000 {
                return Err(DeckError::Invalid("来源文件过多，请选择具体会话目录"));
            }
        }
    }
    output.sort();
    if matches!(harness, LocalHarness::Codex) {
        let mut unique: BTreeMap<String, PathBuf> = BTreeMap::new();
        for path in output {
            let name = path
                .file_name()
                .expect("file")
                .to_string_lossy()
                .into_owned();
            let replace = match unique.get(&name) {
                Some(previous) => path.metadata()?.len() > previous.metadata()?.len(),
                None => true,
            };
            if replace {
                unique.insert(name, path);
            }
        }
        output = unique.into_values().collect();
    }
    Ok(output)
}
fn scan(path: &Path, harness: &LocalHarness) -> Result<(Vec<Event>, usize)> {
    if matches!(harness, LocalHarness::Opencode) {
        return scan_opencode(path);
    }
    let started = Instant::now();
    let mut invalid = 0;
    let mut observed: BTreeMap<String, Event> = BTreeMap::new();
    let mut read_bytes = 0;
    for path in files(path, harness, started)? {
        let metadata = path.metadata()?;
        read_bytes += metadata.len();
        if metadata.len() > 128 * 1024 * 1024 || read_bytes > 512 * 1024 * 1024 {
            return Err(DeckError::Invalid("扫描数据超过单轮限制，请缩小来源目录"));
        }
        if matches!(harness, LocalHarness::GrokCli) {
            let value: Value = serde_json::from_reader(File::open(&path)?)?;
            let total = count(&value["totalTokensBeforeCompaction"]).unwrap_or(0)
                + count(&value["contextTokensUsed"]).unwrap_or(0);
            if total > 0 {
                let at = metadata
                    .modified()?
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| DeckError::Invalid("会话文件时间无效"))?
                    .as_secs() as i64;
                let model = value["primaryModelId"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        if value["modelsUsed"]
                            .as_array()
                            .is_some_and(|models| models.len() == 1)
                        {
                            text(&value["modelsUsed"][0])
                        } else {
                            "多个模型".into()
                        }
                    });
                let id = path.to_string_lossy().into_owned();
                observed.insert(
                    id.clone(),
                    Event {
                        at,
                        provider: Provider::Grok,
                        model,
                        total,
                        breakdown: None,
                        id,
                        pin: None,
                    },
                );
            }
            continue;
        }
        let mut previous: Option<TokenBreakdown> = None;
        let mut model = String::new();
        let mut active_pins = HashMap::<String, String>::new();
        for (index, line) in BufReader::new(File::open(&path)?).lines().enumerate() {
            if started.elapsed() > Duration::from_secs(5) {
                return Err(DeckError::Invalid("用量扫描超过 5 秒，请缩小来源目录"));
            }
            let line = line?;
            if line.len() > 8 * 1024 * 1024 {
                return Err(DeckError::Invalid("会话记录超过 8MB"));
            }
            let value: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(_) => {
                    invalid += 1;
                    continue;
                }
            };
            if matches!(harness, LocalHarness::Omp) && value["type"] == "credential_pin" {
                if let (Some(provider), Some(hash)) =
                    (value["provider"].as_str(), value["hash"].as_str())
                {
                    active_pins.insert(provider.into(), hash.into());
                }
                continue;
            }
            let parsed = match harness {
                LocalHarness::Codex => {
                    if value["type"] == "turn_context" {
                        model = text(
                            value["payload"]
                                .get("model")
                                .unwrap_or(&value["payload"]["info"]["model"]),
                        );
                        continue;
                    }
                    if value["type"] != "event_msg" || value["payload"]["type"] != "token_count" {
                        continue;
                    }
                    let info = &value["payload"]["info"];
                    let cumulative = info
                        .get("total_token_usage")
                        .filter(|value| value.is_object())
                        .map(codex_tokens);
                    let last = info
                        .get("last_token_usage")
                        .filter(|value| value.is_object())
                        .map(codex_tokens);
                    let counters = match (&previous, &cumulative) {
                        (Some(previous), Some(current))
                            if current.input >= previous.input
                                && current.output >= previous.output
                                && current.cached >= previous.cached
                                && current.reasoning >= previous.reasoning =>
                        {
                            Some(TokenBreakdown {
                                input: Some(
                                    current.input.unwrap_or(0) - previous.input.unwrap_or(0),
                                ),
                                cached: Some(
                                    current.cached.unwrap_or(0) - previous.cached.unwrap_or(0),
                                ),
                                cache_write: None,
                                output: Some(
                                    current.output.unwrap_or(0) - previous.output.unwrap_or(0),
                                ),
                                reasoning: Some(
                                    current.reasoning.unwrap_or(0)
                                        - previous.reasoning.unwrap_or(0),
                                ),
                            })
                        }
                        (None, _) => last.or(cumulative.clone()),
                        _ => last,
                    };
                    previous = cumulative.clone();
                    let Some(counters) = counters else {
                        continue;
                    };
                    let total = counters.input.unwrap_or(0) + counters.output.unwrap_or(0);
                    let id = format!(
                        "codex:{}:{}:{}",
                        path.file_name().expect("file").to_string_lossy(),
                        text(&value["timestamp"]),
                        cumulative
                            .map(|value| format!("{:?}", value))
                            .unwrap_or_else(|| index.to_string())
                    );
                    Some((
                        Provider::Openai,
                        timestamp(&value["timestamp"]),
                        model.clone(),
                        total,
                        Some(counters),
                        id,
                    ))
                }
                LocalHarness::ClaudeCode => {
                    let message = &value["message"];
                    if value["type"] != "assistant" && message["role"] != "assistant" {
                        continue;
                    }
                    let usage = &message["usage"];
                    if !usage.is_object() {
                        continue;
                    }
                    let counters = tokens(
                        usage,
                        [
                            "input_tokens",
                            "cache_read_input_tokens",
                            "cache_creation_input_tokens",
                            "output_tokens",
                            "reasoning_tokens",
                        ],
                    );
                    let total = total(&counters);
                    let model = text(&message["model"]);
                    let message_id = text(message.get("id").unwrap_or(&value["messageId"]));
                    let request_id = text(value.get("requestId").unwrap_or(&value["request_id"]));
                    let id = if message_id.is_empty() && request_id.is_empty() {
                        format!("{}:{index}", path.display())
                    } else {
                        format!("claude:{message_id}:{request_id}")
                    };
                    Some((
                        Provider::Claude,
                        timestamp(&value["timestamp"]),
                        model,
                        total,
                        Some(counters),
                        id,
                    ))
                }
                LocalHarness::Omp => {
                    if value["type"] != "message" {
                        continue;
                    }
                    let message = &value["message"];
                    let usage = &message["usage"];
                    if !usage.is_object() {
                        continue;
                    }
                    let Some(provider) = provider(message["provider"].as_str().unwrap_or(""))
                    else {
                        invalid += 1;
                        continue;
                    };
                    let counters = tokens(
                        usage,
                        ["input", "cacheRead", "cacheWrite", "output", "reasoning"],
                    );
                    let total = count(&usage["totalTokens"]).unwrap_or_else(|| total(&counters));
                    let at = timestamp(value.get("timestamp").unwrap_or(&message["timestamp"]));
                    let id = format!(
                        "omp:{}:{}",
                        path.display(),
                        value["id"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| index.to_string())
                    );
                    Some((
                        provider,
                        at,
                        text(&message["model"]),
                        total,
                        Some(counters),
                        id,
                    ))
                }
                LocalHarness::KimiCode => {
                    if value["type"] != "usage.record" {
                        continue;
                    }
                    let counters = tokens(
                        &value["usage"],
                        [
                            "inputOther",
                            "inputCacheRead",
                            "inputCacheCreation",
                            "output",
                            "reasoning",
                        ],
                    );
                    let total = total(&counters);
                    let id = format!("kimi:{}:{index}", path.display());
                    Some((
                        Provider::Kimi,
                        timestamp(&value["time"]),
                        text(&value["model"]),
                        total,
                        Some(counters),
                        id,
                    ))
                }
                _ => unreachable!(),
            };
            if let Some((provider, at, model, total, breakdown, id)) = parsed {
                if let Some(at) = at {
                    if total > 0 {
                        let raw_provider = value["message"]["provider"].as_str().unwrap_or("");
                        let pin = active_pins
                            .get(raw_provider)
                            .map(|hash| (raw_provider.to_owned(), hash.clone()));
                        let event = Event {
                            at,
                            provider,
                            model: if model.is_empty() {
                                "未标记模型".into()
                            } else {
                                model
                            },
                            total,
                            breakdown,
                            id: id.clone(),
                            pin,
                        };
                        if observed
                            .get(&id)
                            .is_none_or(|previous| event.total > previous.total)
                        {
                            observed.insert(id, event);
                        }
                    }
                } else {
                    invalid += 1;
                }
            }
        }
    }
    Ok((observed.into_values().collect(), invalid))
}
fn codex_tokens(value: &Value) -> TokenBreakdown {
    let mut result = tokens(
        value,
        [
            "input_tokens",
            "cached_input_tokens",
            "cache_creation_input_tokens",
            "output_tokens",
            "reasoning_output_tokens",
        ],
    );
    result.cached = Some(
        result
            .cached
            .unwrap_or(0)
            .max(count(&value["cache_read_input_tokens"]).unwrap_or(0)),
    );
    result.reasoning = Some(
        result
            .reasoning
            .or_else(|| count(&value["output_tokens_details"]["reasoning_tokens"]))
            .unwrap_or(0)
            .min(result.output.unwrap_or(0)),
    );
    result
}
fn scan_opencode(path: &Path) -> Result<(Vec<Event>, usize)> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(Duration::from_secs(2))?;
    let mut statement =
        db.prepare("SELECT id, time_created, data FROM message WHERE data IS NOT NULL")?;
    let mut records = statement.query([])?;
    let mut output = vec![];
    let mut invalid = 0;
    let started = Instant::now();
    while let Some(row) = records.next()? {
        if started.elapsed() > Duration::from_secs(5) || output.len() > 1_000_000 {
            return Err(DeckError::Invalid("OpenCode 扫描超过单轮限制"));
        }
        let id: String = row.get(0)?;
        let created: i64 = row.get(1)?;
        let raw: String = row.get(2)?;
        let value: Value = match serde_json::from_str(&raw) {
            Ok(value) => value,
            Err(_) => {
                invalid += 1;
                continue;
            }
        };
        let tokens = &value["tokens"];
        if !tokens.is_object() {
            continue;
        }
        let Some(provider) = provider(
            value["model"]["providerID"]
                .as_str()
                .or_else(|| value["providerID"].as_str())
                .unwrap_or(""),
        ) else {
            invalid += 1;
            continue;
        };
        let breakdown = TokenBreakdown {
            input: count(&tokens["input"]),
            cached: count(&tokens["cache"]["read"]),
            cache_write: count(&tokens["cache"]["write"]),
            output: count(&tokens["output"]),
            reasoning: count(&tokens["reasoning"]),
        };
        let total = count(&tokens["total"]).unwrap_or_else(|| total(&breakdown));
        if total == 0 {
            continue;
        }
        let model = text(
            value["model"]
                .get("modelID")
                .or_else(|| value["model"].get("id"))
                .unwrap_or(&value["modelID"]),
        );
        output.push(Event {
            at: if created > 10_000_000_000 {
                created / 1000
            } else {
                created
            },
            provider,
            model,
            total,
            breakdown: Some(breakdown),
            id,
            pin: None,
        });
    }
    Ok((output, invalid))
}
