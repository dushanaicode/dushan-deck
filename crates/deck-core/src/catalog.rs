use serde::{Deserialize, Serialize};

use crate::error::{DeckError, Result};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Claude,
    Openai,
    Grok,
    Zai,
    Zhipu,
    Kimi,
    Deepseek,
    Antigravity,
    Cursor,
    CursorAgent,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Openai => "openai",
            Self::Grok => "grok",
            Self::Zai => "zai",
            Self::Zhipu => "zhipu",
            Self::Kimi => "kimi",
            Self::Deepseek => "deepseek",
            Self::Antigravity => "antigravity",
            Self::Cursor => "cursor",
            Self::CursorAgent => "cursor_agent",
        }
    }
}

impl Provider {
    pub const ALL: [Self; 10] = [
        Self::Claude,
        Self::Openai,
        Self::Grok,
        Self::Zai,
        Self::Zhipu,
        Self::Kimi,
        Self::Deepseek,
        Self::Antigravity,
        Self::Cursor,
        Self::CursorAgent,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Openai => "OpenAI",
            Self::Grok => "xAI",
            Self::Zai => "Z.ai",
            Self::Zhipu => "Zhipu",
            Self::Kimi => "Kimi Code",
            Self::Deepseek => "DeepSeek",
            Self::Antigravity => "Antigravity",
            Self::Cursor => "Cursor",
            Self::CursorAgent => "Cursor Agent",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: Provider,
    pub name: &'static str,
    pub description: &'static str,
    pub import_formats: &'static [&'static str],
    pub quota_available: bool,
    pub refresh_available: bool,
    pub client_write_available: bool,
}

pub fn providers() -> Vec<ProviderInfo> {
    Provider::ALL
        .into_iter()
        .map(|id| ProviderInfo {
            id,
            name: id.title(),
            description: match id {
                Provider::Claude => "Anthropic · Claude Code",
                Provider::Openai => "OpenAI · Codex",
                Provider::Grok => "xAI · Grok CLI",
                Provider::Zai => "Z.ai Coding Plan",
                Provider::Zhipu => "智谱 Coding Plan",
                Provider::Kimi => "Kimi Code",
                Provider::Deepseek => "DeepSeek API",
                Provider::Antigravity => "Google · Antigravity",
                Provider::Cursor => "Cursor IDE",
                Provider::CursorAgent => "Cursor Agent CLI",
            },
            import_formats: match id {
                Provider::Claude => &["api_key", "claude_code", "quota_json"],
                Provider::Openai => &["api_key", "codex", "quota_json"],
                Provider::Grok | Provider::Cursor | Provider::Antigravity => &["quota_json"],
                _ => &["api_key", "quota_json"],
            },
            quota_available: true,
            refresh_available: matches!(
                id,
                Provider::Claude
                    | Provider::Openai
                    | Provider::Grok
                    | Provider::Cursor
                    | Provider::Antigravity
            ),
            client_write_available: false,
        })
        .collect()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateConnection {
    pub credential_set_id: String,
    pub label: String,
    pub model: String,
    pub base_url: String,
}

impl CreateConnection {
    pub fn validate(&self) -> Result<()> {
        crate::validate_label(&self.label)?;
        if self.model.trim().is_empty()
            || self.model.len() > 160
            || self.model.chars().any(char::is_control)
        {
            return Err(DeckError::Invalid("请输入有效的模型标识（最多 160 字节）"));
        }
        let url = url::Url::parse(&self.base_url)
            .map_err(|_| DeckError::Invalid("请输入完整的服务地址"))?;
        let local_http = url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        if (url.scheme() != "https" && !local_http)
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(DeckError::Invalid(
                "服务地址须为 HTTPS 或本机 HTTP，且不含用户名、密码、查询参数或片段",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConnection {
    pub id: String,
    pub account_id: String,
    pub credential_set_id: String,
    pub label: String,
    pub model: String,
    pub base_url: String,
    pub verified: bool,
}
