use super::{
    credentials::{Credential, jwt},
    providers::{self, text},
    types::QuotaAccount,
};
use crate::catalog::Provider;
use reqwest::{
    Client, Method,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct QueryError {
    pub code: &'static str,
    pub message: String,
    pub retry_after: i64,
}
impl QueryError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_response",
            message: message.into(),
            retry_after: 60,
        }
    }
    pub fn auth(message: impl Into<String>) -> Self {
        Self {
            code: "authentication",
            message: message.into(),
            retry_after: 300,
        }
    }
}
pub(crate) struct QuotaClient {
    client: Client,
    pub offline: bool,
    #[cfg(test)]
    pub test_origin: Option<String>,
}
impl QuotaClient {
    pub fn new(offline: bool) -> Result<Self, reqwest::Error> {
        let builder = Client::builder();
        #[cfg(test)]
        let builder = builder.no_proxy();
        Ok(Self {
            client: builder
                .timeout(Duration::from_secs(8))
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("Dushan-Deck/0.1")
                .build()?,
            offline,
            #[cfg(test)]
            test_origin: None,
        })
    }
    pub async fn request(
        &self,
        method: Method,
        endpoint: &str,
        headers: &[(&str, String)],
        body: Option<&Value>,
        form: bool,
    ) -> Result<Value, QueryError> {
        if self.offline {
            return Err(QueryError {
                code: "offline",
                message: "离线模式：未访问额度服务".into(),
                retry_after: 60,
            });
        }
        #[cfg(test)]
        let endpoint = if let Some(origin) = &self.test_origin {
            let url = reqwest::Url::parse(endpoint).unwrap();
            format!(
                "{origin}/{}{}{}",
                url.host_str().unwrap(),
                url.path(),
                url.query()
                    .map(|query| format!("?{query}"))
                    .unwrap_or_default()
            )
        } else {
            panic!("Tests must use an explicit loopback transport")
        };
        #[cfg(test)]
        let endpoint = endpoint.as_str();
        let mut parsed_headers = HeaderMap::new();
        for (name, value) in headers {
            parsed_headers.insert(
                HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| QueryError::invalid("请求头名称无效"))?,
                HeaderValue::from_str(value)
                    .map_err(|_| QueryError::auth("凭据包含不允许的字符"))?,
            );
        }
        let url = reqwest::Url::parse(endpoint).map_err(|_| QueryError::invalid("服务地址无效"))?;
        let mut request = self.client.request(method, url).headers(parsed_headers);
        if let Some(body) = body {
            request = if form {
                request.form(body)
            } else {
                request.json(body)
            };
        }
        let mut response = request.send().await.map_err(|error| QueryError {
            code: "network",
            message: if error.is_timeout() {
                "网络请求超时"
            } else {
                "无法连接额度服务，请检查网络或代理"
            }
            .into(),
            retry_after: 60,
        })?;
        let status = response.status().as_u16();
        let retry = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(60)
            .clamp(1, 3600);
        if !(200..300).contains(&status) {
            return Err(QueryError {
                code: match status {
                    401 | 403 => "authentication",
                    429 => "rate_limited",
                    _ => "service",
                },
                message: match status {
                    401 | 403 => format!("登录验证失败（HTTP {status}），请检查此账号授权"),
                    429 => "服务限流（HTTP 429）".into(),
                    _ => format!("额度服务请求失败（HTTP {status}）"),
                },
                retry_after: retry,
            });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| QueryError::invalid("额度响应读取失败"))?
        {
            if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                return Err(QueryError::invalid("额度响应超过 4MB"));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| QueryError::invalid("额度服务没有返回有效 JSON"))
    }
    pub async fn refresh(&self, credential: &Credential) -> Result<Value, QueryError> {
        if credential.refresh().is_empty() {
            return Err(QueryError::auth("缺少续期凭据，请重新授权此账号"));
        }
        let google = match (
            credential.field("client_id"),
            credential.field("client_secret"),
        ) {
            ("", "") => super::credentials::quota_google_client(),
            (id, secret) if !id.is_empty() && !secret.is_empty() => {
                (id.to_owned(), secret.to_owned())
            }
            _ if credential.provider == Provider::Antigravity => {
                return Err(QueryError::auth(
                    "Google client_id 与 client_secret 必须成组提供",
                ));
            }
            _ => (String::new(), String::new()),
        };
        let (url, client_id, form) = match credential.provider {
            Provider::Openai => (
                "https://auth.openai.com/oauth/token",
                "app_EMoamEEZ73f0CkXaXp7hrann",
                false,
            ),
            Provider::Claude => (
                "https://platform.claude.com/v1/oauth/token",
                "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
                false,
            ),
            Provider::Grok => (
                "https://auth.x.ai/oauth2/token",
                "b1a00492-073a-47ea-816f-4c329264a828",
                true,
            ),
            Provider::Cursor => (
                "https://api2.cursor.sh/oauth/token",
                "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB",
                false,
            ),
            Provider::Antigravity => (
                "https://oauth2.googleapis.com/token",
                google.0.as_str(),
                true,
            ),
            _ => return Err(QueryError::auth("此凭据类型不支持 OAuth 续期，请重新导入")),
        };
        if client_id.is_empty() {
            return Err(QueryError::auth("Google OAuth 凭据缺少 client_id"));
        }
        let mut body = json!({"grant_type":"refresh_token", "client_id":client_id, "refresh_token":credential.refresh()});
        if credential.provider == Provider::Antigravity {
            body["client_secret"] = google.1.into();
        }
        self.request(Method::POST, url, &[], Some(&body), form)
            .await
    }
    pub async fn quota(&self, credential: &Credential) -> Result<QuotaAccount, QueryError> {
        let provider = credential.provider;
        let mut output = QuotaAccount::empty(
            credential.account_id.clone(),
            provider,
            credential.label.clone(),
        );
        let access = credential.access();
        let key = credential.api_key();
        let mut headers = vec![("Authorization", format!("Bearer {access}"))];
        let endpoint = match provider {
            Provider::Claude => {
                if access.is_empty() {
                    output.pending =
                        "订阅额度需要 Claude OAuth；API Key 可查询有权限的 Admin 用量".into();
                    return Ok(output);
                }
                headers.push(("anthropic-beta", "oauth-2025-04-20".into()));
                "https://api.anthropic.com/api/oauth/usage"
            }
            Provider::Openai => {
                if access.is_empty() {
                    output.pending = "订阅额度需要 Codex OAuth 登录态".into();
                    return Ok(output);
                }
                let id = credential.account_claim();
                if !id.is_empty() {
                    headers.push(("ChatGPT-Account-Id", id));
                }
                "https://chatgpt.com/backend-api/wham/usage"
            }
            Provider::Grok => {
                headers.extend([
                    ("x-grok-client-surface", "grok-build".into()),
                    ("x-grok-client-version", "1.0.0".into()),
                ]);
                "https://cli-chat-proxy.grok.com/v1/billing?format=credits"
            }
            Provider::Zai | Provider::Zhipu => {
                if key.is_empty() {
                    return Err(QueryError::auth("此来源需要 API Key"));
                }
                headers = vec![("Authorization", key.into())];
                if provider == Provider::Zhipu || credential.field("variant").contains("zhipu") {
                    output.title = "Zhipu".into();
                    "https://bigmodel.cn/api/monitor/usage/quota/limit"
                } else {
                    "https://api.z.ai/api/monitor/usage/quota/limit"
                }
            }
            Provider::Kimi => {
                if key.is_empty() {
                    return Err(QueryError::auth("Kimi Code 需要 API Key"));
                }
                headers = vec![("Authorization", format!("Bearer {key}"))];
                "https://api.kimi.com/coding/v1/usages"
            }
            Provider::Deepseek => {
                if key.is_empty() {
                    return Err(QueryError::auth("DeepSeek 需要 API Key"));
                }
                headers = vec![("Authorization", format!("Bearer {key}"))];
                "https://api.deepseek.com/user/balance"
            }
            Provider::Cursor => {
                let claims = jwt(access);
                let sub = claims["sub"].as_str().unwrap_or("");
                let user_id = sub.rsplit('|').next().unwrap_or("");
                if !user_id.starts_with("user_") {
                    return Err(QueryError::auth("Cursor 需要有效的 IDE 登录凭据"));
                }
                headers = vec![
                    (
                        "Cookie",
                        format!("WorkosCursorSessionToken={user_id}%3A%3A{access}"),
                    ),
                    ("Accept", "application/json".into()),
                ];
                "https://cursor.com/api/usage-summary"
            }
            Provider::CursorAgent => {
                let mut token = access.to_owned();
                if !key.is_empty() {
                    let exchanged = self
                        .request(
                            Method::POST,
                            "https://api2.cursor.sh/auth/exchange_user_api_key",
                            &[("Authorization", format!("Bearer {key}"))],
                            Some(&json!({})),
                            false,
                        )
                        .await?;
                    token = exchanged["accessToken"]
                        .as_str()
                        .or_else(|| exchanged["access_token"].as_str())
                        .ok_or_else(|| QueryError::invalid("Cursor Agent 未返回访问令牌"))?
                        .into();
                }
                if token.is_empty() {
                    return Err(QueryError::auth(
                        "Cursor Agent 需要 API Key 或 access token",
                    ));
                }
                headers = vec![
                    ("Authorization", format!("Bearer {token}")),
                    ("Connect-Protocol-Version", "1".into()),
                ];
                "https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage"
            }
            Provider::Antigravity => return self.antigravity(credential).await,
        };
        if matches!(provider, Provider::Grok) && access.is_empty() {
            return Err(QueryError::auth("Grok 订阅额度需要 OAuth 登录态"));
        }
        let rpc_body = json!({});
        let data = self
            .request(
                if provider == Provider::CursorAgent {
                    Method::POST
                } else {
                    Method::GET
                },
                endpoint,
                &headers,
                (provider == Provider::CursorAgent).then_some(&rpc_body),
                false,
            )
            .await?;
        providers::parse_primary(provider, &data, &mut output).map_err(QueryError::invalid)?;
        match provider {
            Provider::Claude => match self
                .request(
                    Method::GET,
                    "https://api.anthropic.com/api/oauth/profile",
                    &headers,
                    None,
                    false,
                )
                .await
            {
                Ok(profile) => providers::claude_profile(&profile, &mut output),
                Err(error) => optional_error(&mut output, "账号资料", &error),
            },
            Provider::Openai => {
                let claims = jwt(access);
                if let Some(email) = claims["https://api.openai.com/profile"]["email"].as_str() {
                    output.email = email.into();
                }
                let account_id = credential.account_claim();
                if !account_id.is_empty() {
                    let mut url =
                        reqwest::Url::parse("https://chatgpt.com/backend-api/subscriptions")
                            .expect("constant URL");
                    url.query_pairs_mut().append_pair("account_id", &account_id);
                    match self
                        .request(Method::GET, url.as_str(), &headers, None, false)
                        .await
                    {
                        Ok(subscription) => {
                            output.sub_end = providers::iso(
                                subscription
                                    .get("active_until")
                                    .unwrap_or(&subscription["expires_at"]),
                            );
                            let plan = text(
                                subscription
                                    .get("subscription_plan")
                                    .unwrap_or(&subscription["plan_type"]),
                            );
                            if data["plan_type"] == "pro" && !plan.is_empty() {
                                output.plan = providers::openai_plan(&plan);
                            }
                        }
                        Err(error) => optional_error(&mut output, "订阅资料", &error),
                    }
                }
            }
            Provider::Grok => {
                let task_headers = [
                    ("Authorization", format!("Bearer {access}")),
                    ("x-xai-token-auth", "xai-grok-cli".into()),
                    ("User-Agent", "Grok Build".into()),
                ];
                let (task, user, subscriptions) = tokio::join!(
                    self.request(
                        Method::GET,
                        "https://grok.com/rest/tasks/usage",
                        &task_headers,
                        None,
                        false
                    ),
                    self.request(
                        Method::GET,
                        "https://cli-chat-proxy.grok.com/v1/user?include=subscription",
                        &headers,
                        None,
                        false
                    ),
                    self.request(
                        Method::GET,
                        "https://grok.com/rest/subscriptions",
                        &headers,
                        None,
                        false
                    )
                );
                for (label, response) in [
                    ("任务额度", &task),
                    ("账号资料", &user),
                    ("订阅资料", &subscriptions),
                ] {
                    if let Err(error) = response {
                        optional_error(&mut output, label, error);
                    }
                }
                providers::grok_details(
                    task.as_ref().ok(),
                    user.as_ref().ok(),
                    subscriptions.as_ref().ok(),
                    &mut output,
                );
            }
            Provider::CursorAgent => {
                for (method, field) in [("GetMe", "email"), ("GetPlanInfo", "plan")] {
                    match self
                        .request(
                            Method::POST,
                            &format!(
                                "https://api2.cursor.sh/aiserver.v1.DashboardService/{method}"
                            ),
                            &headers,
                            Some(&rpc_body),
                            false,
                        )
                        .await
                    {
                        Ok(value) => {
                            if field == "email" {
                                if let Some(email) = value["email"].as_str() {
                                    output.email = email.into();
                                }
                            } else {
                                output.plan = text(&value["planInfo"]["planName"]);
                            }
                        }
                        Err(error) => optional_error(&mut output, "账号资料", &error),
                    }
                }
            }
            Provider::Zai | Provider::Zhipu => {
                let url = if provider == Provider::Zhipu
                    || credential.field("variant").contains("zhipu")
                {
                    "https://open.bigmodel.cn/api/paas/v4/user"
                } else {
                    "https://api.z.ai/api/paas/v4/user"
                };
                match self
                    .request(
                        Method::GET,
                        url,
                        &[("Authorization", format!("Bearer {key}"))],
                        None,
                        false,
                    )
                    .await
                {
                    Ok(value) => {
                        let profile = providers::root(&value);
                        if let Some(email) = profile["email"].as_str() {
                            output.email = email.into();
                        }
                        output.plan = text(
                            profile
                                .get("plan")
                                .or_else(|| profile.get("level"))
                                .unwrap_or(&profile["package"]),
                        );
                    }
                    Err(error) => optional_error(&mut output, "账号资料", &error),
                }
            }
            _ => {}
        }
        if provider == Provider::Openai {
            self.openai_subscription_check(credential, &headers, &mut output)
                .await;
        }
        if let Some(end) = providers::timestamp(&Value::String(output.sub_end.clone())) {
            output.sub_status = if end < crate::now() {
                "expired"
            } else {
                "known"
            }
            .into();
        }
        Ok(output)
    }
    async fn openai_subscription_check(
        &self,
        credential: &Credential,
        headers: &[(&str, String)],
        output: &mut QuotaAccount,
    ) {
        let account_id = credential.account_claim();
        if account_id.is_empty() {
            return;
        }
        let path = "/backend-api/accounts/check/v4-2023-04-27";
        let mut request_headers = headers.to_vec();
        request_headers.extend([
            ("Accept", "application/json".into()),
            ("Referer", "https://chatgpt.com/".into()),
            ("x-openai-target-path", path.into()),
            ("x-openai-target-route", path.into()),
        ]);
        let offset = -chrono::Local::now().offset().local_minus_utc() / 60;
        match self
            .request(
                Method::GET,
                &format!("https://chatgpt.com{path}?timezone_offset_min={offset}"),
                &request_headers,
                None,
                false,
            )
            .await
        {
            Ok(data) => {
                let records: Vec<&Value> = match &data["accounts"] {
                    Value::Object(records) => records.values().collect(),
                    Value::Array(records) => records.iter().collect(),
                    _ => vec![],
                };
                let selected = records.into_iter().find(|record| {
                    let account = record
                        .get("account")
                        .filter(|value| value.is_object())
                        .unwrap_or(record);
                    ["account_id", "id", "chatgpt_account_id", "workspace_id"]
                        .iter()
                        .any(|key| account[key].as_str() == Some(&account_id))
                });
                if let Some(record) = selected {
                    let entitlement = &record["entitlement"];
                    let end = entitlement
                        .get("expires_at")
                        .unwrap_or(&entitlement["active_until"]);
                    if providers::timestamp(end).is_some_and(|end| end > crate::now()) {
                        output.sub_end = providers::iso(end);
                    }
                    let plan = text(&entitlement["subscription_plan"]);
                    if !plan.is_empty()
                        && (output.plan == "OpenAI" || output.plan == "OpenAI (Pro 20x)")
                    {
                        output.plan = providers::openai_plan(&plan);
                    }
                }
            }
            Err(error) => optional_error(output, "账号订阅资料", &error),
        }
    }
    async fn antigravity(&self, credential: &Credential) -> Result<QuotaAccount, QueryError> {
        if credential.access().is_empty() {
            return Err(QueryError::auth("Antigravity 需要 Google OAuth 登录态"));
        }
        let headers = [
            ("Authorization", format!("Bearer {}", credential.access())),
            (
                "User-Agent",
                "antigravity/1.104.0 (Windows NT 10.0; Win64; x64)".into(),
            ),
        ];
        let profile = self
            .request(
                Method::POST,
                "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist",
                &headers,
                Some(
                    &json!({"mode":"FULL_ELIGIBILITY_CHECK", "metadata":{"ideType":"ANTIGRAVITY"}}),
                ),
                false,
            )
            .await?;
        let project = profile["cloudaicompanionProject"]
            .as_str()
            .or_else(|| profile["cloudaicompanionProject"]["id"].as_str())
            .ok_or_else(|| QueryError::invalid("Antigravity 未返回项目上下文"))?;
        let tier = profile["paidTier"]["id"]
            .as_str()
            .or_else(|| profile["currentTier"]["id"].as_str())
            .unwrap_or("");
        let mut output = QuotaAccount::empty(
            credential.account_id.clone(),
            credential.provider,
            credential.label.clone(),
        );
        output.plan = match tier {
            "g1-pro-tier" => "Google AI Pro",
            "g1-ultra-tier" => "Google AI Ultra",
            "free-tier" => "Free Tier",
            _ => tier,
        }
        .into();
        let mut failure = QueryError::invalid("Antigravity 未返回可识别的额度");
        // These are the two active daily hosts and two read-only response contracts used by Quota.
        for method in ["retrieveUserQuotaSummary", "fetchAvailableModels"] {
            for host in [
                "daily-cloudcode-pa.sandbox.googleapis.com",
                "daily-cloudcode-pa.googleapis.com",
            ] {
                match self
                    .request(
                        Method::POST,
                        &format!("https://{host}/v1internal:{method}"),
                        &headers,
                        Some(&json!({"project":project})),
                        false,
                    )
                    .await
                {
                    Ok(data) => {
                        match providers::parse_primary(Provider::Antigravity, &data, &mut output) {
                            Ok(()) => return Ok(output),
                            Err(error) => failure = QueryError::invalid(error),
                        }
                    }
                    Err(error)
                        if error.code == "authentication" || error.code == "rate_limited" =>
                    {
                        return Err(error);
                    }
                    Err(error) => failure = error,
                }
            }
        }
        Err(failure)
    }
}
pub(super) fn optional_error(output: &mut QuotaAccount, label: &str, error: &QueryError) {
    if !output.notice.is_empty() {
        output.notice += "；";
    }
    output.notice += &format!("{label}：{}", error.message);
    output.retry_at = output.retry_at.max(crate::now() + error.retry_after);
}
