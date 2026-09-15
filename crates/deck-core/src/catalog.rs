use serde::{Deserialize, Serialize};

use crate::error::{DeckError, Result};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Claude,
    Openai,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Openai => "openai",
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
    vec![
        ProviderInfo {
            id: Provider::Claude,
            name: "Claude",
            description: "Anthropic · Claude Code",
            import_formats: &["api_key", "claude_code"],
            quota_available: false,
            refresh_available: false,
            client_write_available: false,
        },
        ProviderInfo {
            id: Provider::Openai,
            name: "OpenAI",
            description: "OpenAI · Codex",
            import_formats: &["api_key", "codex"],
            quota_available: false,
            refresh_available: false,
            client_write_available: false,
        },
    ]
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
