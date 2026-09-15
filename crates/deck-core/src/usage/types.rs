use crate::{catalog::Provider, settings::floating::UsagePeriod};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub name: String,
    pub remaining_percent: Option<f64>,
    pub reset_ts: Option<i64>,
    pub reset: String,
    pub text: Option<String>,
}
impl QuotaWindow {
    pub fn percent(name: impl Into<String>, remaining: f64, reset_ts: Option<i64>) -> Self {
        Self {
            name: name.into(),
            remaining_percent: Some(remaining.clamp(0., 100.)),
            reset_ts,
            ..Self::default()
        }
    }
    pub fn text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: Some(text.into()),
            ..Self::default()
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBreakdown {
    pub input: Option<u64>,
    pub cached: Option<u64>,
    pub cache_write: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageModel {
    pub name: String,
    pub total_tokens: Option<u64>,
    pub breakdown: Option<TokenBreakdown>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    pub source: String,
    pub label: String,
    pub period: UsagePeriod,
    pub harness: String,
    pub harness_label: String,
    pub total_tokens: Option<u64>,
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub unit: String,
    pub breakdown: Option<TokenBreakdown>,
    pub models: Vec<UsageModel>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Harness {
    pub key: String,
    pub label: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaAccount {
    pub account_id: String,
    pub provider: Provider,
    pub title: String,
    pub email: String,
    pub plan: String,
    pub plan_detail: String,
    pub sub_end: String,
    pub sub_status: String,
    pub windows: Vec<QuotaWindow>,
    pub usage: Vec<UsageRow>,
    pub harnesses: Vec<Harness>,
    pub ok: bool,
    pub error: String,
    pub notice: String,
    pub retry_at: i64,
    pub pending: String,
}
impl QuotaAccount {
    pub fn empty(account_id: String, provider: Provider, label: String) -> Self {
        Self {
            account_id,
            provider,
            title: provider.title().into(),
            email: label,
            plan: String::new(),
            plan_detail: String::new(),
            sub_end: String::new(),
            sub_status: String::new(),
            windows: vec![],
            usage: vec![],
            harnesses: vec![],
            ok: false,
            error: String::new(),
            notice: String::new(),
            retry_at: 0,
            pending: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot {
    pub results: Vec<QuotaAccount>,
    pub state: &'static str,
    pub fetched_at: Option<String>,
    pub error: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalHarness {
    Codex,
    ClaudeCode,
    Opencode,
    Omp,
    KimiCode,
    GrokCli,
}
impl LocalHarness {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
            Self::Opencode => "opencode",
            Self::Omp => "omp",
            Self::KimiCode => "kimi_code",
            Self::GrokCli => "grok_cli",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Opencode => "OpenCode",
            Self::Omp => "OMP",
            Self::KimiCode => "Kimi Code CLI",
            Self::GrokCli => "Grok CLI",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageSource {
    pub id: String,
    pub account_id: String,
    pub harness: LocalHarness,
    pub path: String,
    pub since: i64,
}
