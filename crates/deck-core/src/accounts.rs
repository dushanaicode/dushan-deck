use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{
    catalog::Provider,
    error::{DeckError, Result},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportAccount {
    pub provider: Provider,
    pub label: String,
    pub format: ImportFormat,
    pub content: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    ApiKey,
    ClaudeCode,
    Codex,
    QuotaJson,
}

pub(crate) struct ParsedCredential {
    pub payload: Zeroizing<Vec<u8>>,
    pub fingerprint: String,
    pub kind: &'static str,
    pub source: &'static str,
    pub expires_at: Option<i64>,
}

impl ImportAccount {
    pub(crate) fn parse(&mut self) -> Result<ParsedCredential> {
        crate::validate_label(&self.label)?;
        let input = Zeroizing::new(std::mem::take(&mut self.content));
        if input.is_empty() || input.len() > 256 * 1024 {
            return Err(DeckError::Invalid("凭据内容为空或超过 256 KiB"));
        }
        let (payload, anchor, kind, source, expires_at) = match self.format {
            ImportFormat::QuotaJson => {
                let mut document: Value = serde_json::from_str(&input)
                    .map_err(|_| DeckError::Invalid("请输入单个 Quota 账号的完整 JSON"))?;
                if let Some(records) = document.as_array_mut() {
                    if records.len() != 1 {
                        return Err(DeckError::Invalid("请导入单个 Quota 账号记录"));
                    }
                    document = records.remove(0);
                }
                if document.get("provider").and_then(Value::as_str) != Some(self.provider.as_str())
                {
                    return Err(DeckError::Invalid(
                        "Quota JSON 的 provider 与所选专区不一致",
                    ));
                }
                if document.get("secret").is_none() {
                    document =
                        serde_json::json!({"provider": self.provider.as_str(), "secret": document});
                }
                let secret = document
                    .get("secret")
                    .and_then(Value::as_object)
                    .ok_or(DeckError::Invalid("Quota JSON 需要完整 secret 对象"))?;
                let api_key = secret
                    .get("api_key")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty());
                let access = secret
                    .get("access")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty());
                let anchor = api_key
                    .or(access)
                    .ok_or(DeckError::Invalid("secret 缺少 api_key 或 access"))?
                    .to_owned();
                let kind = if api_key.is_some() {
                    "api_key"
                } else {
                    "oauth"
                };
                let expires = secret.get("expiry").and_then(Value::as_i64);
                (document, anchor, kind, "quota_json", expires)
            }
            ImportFormat::ApiKey => {
                let key = input.trim();
                if key.len() < 8 || key.chars().any(char::is_whitespace) {
                    return Err(DeckError::Invalid("API Key 格式无效"));
                }
                (
                    serde_json::json!({"api_key": key}),
                    key.to_owned(),
                    "api_key",
                    "manual_key",
                    None,
                )
            }
            ImportFormat::ClaudeCode | ImportFormat::Codex => {
                let document: Value = serde_json::from_str(&input)
                    .map_err(|_| DeckError::Invalid("请输入完整的 JSON 凭据文件"))?;
                let (tokens, access_field, refresh_field, source) =
                    match (self.provider, self.format) {
                        (Provider::Claude, ImportFormat::ClaudeCode) => (
                            &document["claudeAiOauth"],
                            "accessToken",
                            "refreshToken",
                            "claude_code_json",
                        ),
                        (Provider::Openai, ImportFormat::Codex) => (
                            &document["tokens"],
                            "access_token",
                            "refresh_token",
                            "codex_json",
                        ),
                        _ => return Err(DeckError::Invalid("凭据文件格式与专区不匹配")),
                    };
                let access = required_token(tokens, access_field)?;
                let refresh = required_token(tokens, refresh_field)?;
                let expires_at =
                    if matches!(self.format, ImportFormat::ClaudeCode) {
                        match tokens.get("expiresAt") {
                            Some(value) => Some(
                                value.as_i64().filter(|n| *n > 0).ok_or(DeckError::Invalid(
                                    "expiresAt 必须是正整数毫秒时间戳",
                                ))? / 1000,
                            ),
                            None => None,
                        }
                    } else {
                        None
                    };
                // A session is identified by the entire refresh token, never by its source or suffix.
                let anchor = format!("{access}\0{refresh}");
                (document, anchor, "oauth", source, expires_at)
            }
        };
        let anchor = Zeroizing::new(anchor);
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(format!("{}\0{kind}\0{}", self.provider.as_str(), *anchor))
        );
        Ok(ParsedCredential {
            payload: Zeroizing::new(serde_json::to_vec(&payload)?),
            fingerprint,
            kind,
            source,
            expires_at,
        })
    }
}

fn required_token<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .filter(|token| !token.is_empty() && !token.chars().any(char::is_whitespace))
        .ok_or(DeckError::Invalid(
            "OAuth 凭据必须包含同一会话的完整 access / refresh token",
        ))
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub provider: Provider,
    pub label: String,
    pub identity_status: String,
    pub created_at: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSet {
    pub id: String,
    pub account_id: String,
    pub kind: String,
    pub source: String,
    pub version: i64,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub account_id: String,
    pub credential_set_id: String,
    pub outcome: &'static str,
}
