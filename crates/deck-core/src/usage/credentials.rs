use crate::{
    accounts::{ImportAccount, ImportFormat},
    catalog::Provider,
    error::{DeckError, Result},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::Value;
use zeroize::Zeroizing;

pub(crate) struct Credential {
    pub account_id: String,
    pub credential_id: String,
    pub provider: Provider,
    pub label: String,
    pub version: i64,
    pub source: String,
    pub expires_at: Option<i64>,
    pub document: Value,
    pub fingerprint: String,
    pub key: Zeroizing<[u8; 32]>,
}

pub(super) fn quota_google_client() -> (String, String) {
    // Shared installed-client registration used by Dushan Quota, not a user's account tokens.
    // Keep custom client_id/client_secret together in the imported credential group.
    let client = [
        107, 106, 109, 107, 106, 106, 108, 106, 108, 106, 111, 99, 107, 119, 46, 55, 50, 41, 41,
        51, 52, 104, 50, 104, 107, 54, 57, 40, 63, 104, 105, 111, 44, 46, 53, 54, 53, 48, 50, 110,
        61, 110, 106, 105, 63, 42, 116, 59, 42, 42, 41, 116, 61, 53, 53, 61, 54, 63, 47, 41, 63,
        40, 57, 53, 52, 46, 63, 52, 46, 116, 57, 53, 55,
    ];
    let secret = [
        29, 21, 25, 9, 10, 2, 119, 17, 111, 98, 28, 13, 8, 110, 98, 108, 22, 62, 22, 16, 107, 55,
        22, 24, 98, 41, 2, 25, 110, 32, 108, 43, 30, 27, 60,
    ];
    let decode = |bytes: &[u8]| {
        String::from_utf8(bytes.iter().map(|byte| byte ^ 0x5a).collect())
            .expect("fixed ASCII registration")
    };
    (decode(&client), decode(&secret))
}
impl Credential {
    pub fn usage_provider(&self) -> Provider {
        if self.provider == Provider::Zai && self.field("variant").contains("zhipu") {
            Provider::Zhipu
        } else {
            self.provider
        }
    }
    pub fn omp_pins(&self) -> Vec<(String, String)> {
        use sha2::{Digest, Sha256};
        let claims = jwt(self.access());
        let account_id = self.account_claim();
        let account_id = if account_id.is_empty() {
            self.field("user_id")
        } else {
            &account_id
        };
        let email = if self.field("email").is_empty() {
            claims["https://api.openai.com/profile"]["email"]
                .as_str()
                .unwrap_or("")
        } else {
            self.field("email")
        };
        if (account_id.is_empty() && email.is_empty()) || account_id.contains('*') {
            return vec![];
        }
        let providers: &[&str] = match self.usage_provider() {
            Provider::Openai => &["openai", "openai-codex", "codex"],
            Provider::Claude => &["claude", "anthropic"],
            Provider::Grok => &["xai", "xai-oauth"],
            Provider::Kimi => &["kimi", "kimi-code"],
            Provider::Zai => &["zai"],
            Provider::Zhipu => &["zhipu-coding-plan", "zhipuai-coding-plan"],
            Provider::Deepseek => &["deepseek"],
            Provider::CursorAgent => &["cursor"],
            _ => &[],
        };
        providers
            .iter()
            .map(|provider| {
                let value = [
                    *provider,
                    account_id,
                    email,
                    self.field("organization_id"),
                    self.field("project_id"),
                ]
                .join("\0");
                (
                    provider.to_string(),
                    format!("{:x}", Sha256::digest(value.as_bytes())),
                )
            })
            .collect()
    }
    pub fn fields(&self) -> &Value {
        match self.source.as_str() {
            "claude_code_json" => &self.document["claudeAiOauth"],
            "codex_json" => &self.document["tokens"],
            "quota_json" => &self.document["secret"],
            "manual_key" => &self.document,
            _ => unreachable!("stored source is created by accounts parser"),
        }
    }
    pub fn access(&self) -> &str {
        self.fields()[match self.source.as_str() {
            "claude_code_json" => "accessToken",
            "codex_json" => "access_token",
            _ => "access",
        }]
        .as_str()
        .unwrap_or("")
    }
    pub fn refresh(&self) -> &str {
        self.fields()[match self.source.as_str() {
            "claude_code_json" => "refreshToken",
            "codex_json" => "refresh_token",
            _ => "refresh",
        }]
        .as_str()
        .unwrap_or("")
    }
    pub fn api_key(&self) -> &str {
        self.fields()["api_key"].as_str().unwrap_or("")
    }
    pub fn account_claim(&self) -> String {
        self.fields()["account_id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                jwt(self.access())["https://api.openai.com/auth"]["chatgpt_account_id"]
                    .as_str()
                    .unwrap_or("")
                    .into()
            })
    }
    pub fn field(&self, key: &str) -> &str {
        self.fields()[key].as_str().unwrap_or("")
    }
    pub fn rotated(&self, response: &Value) -> Result<(Value, String, Option<i64>)> {
        let access = response["access_token"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or(DeckError::Invalid("续期响应缺少有效 access_token"))?;
        if response["shouldLogout"] == true {
            return Err(DeckError::Invalid("登录会话已失效，请重新授权"));
        }
        let refresh = match response.get("refresh_token") {
            None => self.refresh(),
            Some(value) => value
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or(DeckError::Invalid("续期响应中的 refresh_token 无效"))?,
        };
        let expires = response["expires_in"]
            .as_u64()
            .filter(|seconds| *seconds > 0 && *seconds <= 366 * 86400)
            .map(|seconds| crate::now() + seconds as i64);
        if self.provider == Provider::Claude && expires.is_none() {
            return Err(DeckError::Invalid("Claude 续期响应缺少有效有效期"));
        }
        if self.provider == Provider::Openai {
            let expected = self.account_claim();
            let actual = jwt(access)["https://api.openai.com/auth"]["chatgpt_account_id"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            if !expected.is_empty() && !actual.is_empty() && expected != actual {
                return Err(DeckError::Invalid("续期返回的账号与原凭据组不一致"));
            }
        }
        let mut document = self.document.clone();
        let fields = match self.source.as_str() {
            "claude_code_json" => &mut document["claudeAiOauth"],
            "codex_json" => &mut document["tokens"],
            "quota_json" => &mut document["secret"],
            _ => return Err(DeckError::Invalid("API Key 无需 OAuth 续期")),
        };
        let (access_key, refresh_key) = match self.source.as_str() {
            "claude_code_json" => ("accessToken", "refreshToken"),
            "codex_json" => ("access_token", "refresh_token"),
            _ => ("access", "refresh"),
        };
        fields[access_key] = access.into();
        fields[refresh_key] = refresh.into();
        if let Some(id) = response.get("id_token") {
            let token = id
                .as_str()
                .ok_or(DeckError::Invalid("续期响应中的 id_token 无效"))?;
            let claims = jwt(token);
            let id_account = claims["https://api.openai.com/auth"]["chatgpt_account_id"]
                .as_str()
                .unwrap_or("");
            if self.provider == Provider::Openai
                && !id_account.is_empty()
                && !self.account_claim().is_empty()
                && id_account != self.account_claim()
            {
                return Err(DeckError::Invalid("id_token 不属于当前账号"));
            }
            fields["id_token"] = token.into();
        }
        if let Some(expires) = expires {
            fields[if self.source == "claude_code_json" {
                "expiresAt"
            } else {
                "expiry"
            }] = if self.source == "claude_code_json" {
                expires * 1000
            } else {
                expires
            }
            .into();
        }
        let format = match self.source.as_str() {
            "claude_code_json" => ImportFormat::ClaudeCode,
            "codex_json" => ImportFormat::Codex,
            "quota_json" => ImportFormat::QuotaJson,
            _ => unreachable!(),
        };
        let mut input = ImportAccount {
            provider: self.provider,
            label: self.label.clone(),
            format,
            content: serde_json::to_string(&document)?,
        };
        let fingerprint = input.parse()?.fingerprint;
        Ok((document, fingerprint, expires))
    }
}
pub(super) fn jwt(token: &str) -> Value {
    // Claims are hints for provider requests/display only, never proof used to merge accounts.
    let Some(payload) = token.split('.').nth(1) else {
        return Value::Null;
    };
    URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null)
}
