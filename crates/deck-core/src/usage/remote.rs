use super::{
    credentials::Credential,
    http::{QueryError, QuotaClient},
    local::{self, Event},
    providers::{root, text, timestamp},
    types::{TokenBreakdown, UsageModel, UsageRow},
};
use crate::{catalog::Provider, settings::floating::UsagePeriod};
use chrono::{Duration, Local, TimeZone};
use reqwest::Method;
use serde_json::Value;

pub(super) async fn fetch(
    client: &QuotaClient,
    credential: &Credential,
) -> Result<Vec<UsageRow>, QueryError> {
    match credential.provider {
        Provider::Openai if !credential.access().is_empty() => {
            let mut headers = vec![
                ("Authorization", format!("Bearer {}", credential.access())),
                ("User-Agent", "codex-cli".into()),
            ];
            let account_id = credential.account_claim();
            if !account_id.is_empty() {
                headers.push(("ChatGPT-Account-Id", account_id));
            }
            // This is the read-only endpoint used by OpenAI's BackendClient::get_token_usage_profile.
            let data = client
                .request(
                    Method::GET,
                    "https://chatgpt.com/backend-api/wham/profiles/me",
                    &headers,
                    None,
                    false,
                )
                .await?;
            Ok(openai_rows(&data))
        }
        Provider::Claude if !credential.api_key().is_empty() => {
            let now = Local::now();
            let start = (now - Duration::days(30))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .expect("midnight");
            let mut url = reqwest::Url::parse(
                "https://api.anthropic.com/v1/organizations/usage_report/messages",
            )
            .expect("constant URL");
            url.query_pairs_mut()
                .append_pair(
                    "starting_at",
                    &Local
                        .from_local_datetime(&start)
                        .earliest()
                        .expect("local midnight")
                        .to_rfc3339(),
                )
                .append_pair("ending_at", &now.to_rfc3339())
                .append_pair("bucket_width", "1d")
                .append_pair("group_by[]", "model")
                .append_pair("limit", "31");
            let mut events = Vec::new();
            let mut pages = 0;
            loop {
                let data = client
                    .request(
                        Method::GET,
                        url.as_str(),
                        &[
                            ("x-api-key", credential.api_key().into()),
                            ("anthropic-version", "2023-06-01".into()),
                        ],
                        None,
                        false,
                    )
                    .await?;
                events.extend(anthropic_events(&data));
                pages += 1;
                if data["has_more"] != true {
                    break;
                }
                if pages >= 100 {
                    return Err(QueryError::invalid("Admin 用量分页超过 100 页"));
                }
                let page = data["next_page"]
                    .as_str()
                    .ok_or_else(|| QueryError::invalid("Admin 用量分页缺少 next_page"))?;
                let pairs: Vec<(String, String)> = url
                    .query_pairs()
                    .filter(|(key, _)| key != "page")
                    .map(|(key, value)| (key.into_owned(), value.into_owned()))
                    .collect();
                url.query_pairs_mut()
                    .clear()
                    .extend_pairs(pairs)
                    .append_pair("page", page);
            }
            Ok(local::rows(
                &events,
                "anthropic_admin",
                "Anthropic Admin",
                "remote",
                now.timestamp(),
            )
            .into_iter()
            .filter(|row| row.period != UsagePeriod::All)
            .collect())
        }
        Provider::Zai | Provider::Zhipu if !credential.api_key().is_empty() => {
            let host = if credential.provider == Provider::Zhipu
                || credential.field("variant").contains("zhipu")
            {
                "bigmodel.cn"
            } else {
                "api.z.ai"
            };
            let now = Local::now();
            let mut url =
                reqwest::Url::parse(&format!("https://{host}/api/monitor/usage/model-usage"))
                    .expect("constant URL");
            url.query_pairs_mut()
                .append_pair(
                    "startTime",
                    &(now - Duration::days(30))
                        .format("%Y-%m-%d 00:00:00")
                        .to_string(),
                )
                .append_pair("endTime", &now.format("%Y-%m-%d %H:59:59").to_string());
            let data = client
                .request(
                    Method::GET,
                    url.as_str(),
                    &[("Authorization", format!("Bearer {}", credential.api_key()))],
                    None,
                    false,
                )
                .await?;
            let mut models = Vec::new();
            for value in root(&data)["modelDataList"]
                .as_array()
                .ok_or_else(|| QueryError::invalid("远端用量缺少 modelDataList"))?
            {
                let name = text(value.get("modelName").unwrap_or(&value["modelCode"]));
                let Some(values) = value["tokensUsage"].as_array() else {
                    continue;
                };
                let total = values.iter().filter_map(Value::as_u64).sum();
                models.push(UsageModel {
                    name,
                    total_tokens: Some(total),
                    breakdown: None,
                });
            }
            if models.is_empty() {
                return Ok(vec![]);
            }
            Ok(vec![UsageRow {
                source: "remote".into(),
                label: "近 30 天".into(),
                period: UsagePeriod::Month,
                harness: "remote".into(),
                harness_label: "远端".into(),
                total_tokens: Some(models.iter().filter_map(|model| model.total_tokens).sum()),
                used: None,
                total: None,
                unit: String::new(),
                breakdown: None,
                models,
            }])
        }
        _ => Ok(vec![]),
    }
}
pub(super) fn openai_rows(data: &Value) -> Vec<UsageRow> {
    let stats = &data["stats"];
    let today = Local::now().date_naive();
    let mut rows = vec![];
    for (period, days) in [
        (UsagePeriod::Day, 1),
        (UsagePeriod::ThreeDays, 3),
        (UsagePeriod::Week, 7),
        (UsagePeriod::Month, 30),
        (UsagePeriod::All, 0),
    ] {
        let total = if days == 0 {
            stats["lifetime_tokens"].as_u64()
        } else {
            let values: Vec<_> = stats["daily_usage_buckets"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .filter_map(|bucket| {
                    let date = chrono::NaiveDate::parse_from_str(
                        bucket["start_date"].as_str()?,
                        "%Y-%m-%d",
                    )
                    .ok()?;
                    (date <= today && date >= today - Duration::days(days - 1))
                        .then(|| bucket["tokens"].as_u64())
                        .flatten()
                })
                .collect();
            (!values.is_empty()).then(|| values.into_iter().sum())
        };
        if let Some(total) = total {
            rows.push(UsageRow {
                source: "remote".into(),
                label: if days == 0 {
                    "累计".into()
                } else if days == 1 {
                    "今日".into()
                } else {
                    format!("近 {days} 天")
                },
                period,
                harness: "remote".into(),
                harness_label: "远端".into(),
                total_tokens: Some(total),
                used: None,
                total: None,
                unit: String::new(),
                breakdown: None,
                models: vec![],
            });
        }
    }
    rows
}
fn anthropic_events(data: &Value) -> Vec<Event> {
    let mut events = Vec::new();
    for bucket in data["data"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let Some(at) = timestamp(&bucket["starting_at"]) else {
            continue;
        };
        for item in bucket["results"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            let cache_write = item["cache_creation"]
                .as_object()
                .map(|values| values.values().filter_map(Value::as_u64).sum());
            let breakdown = TokenBreakdown {
                input: item["uncached_input_tokens"].as_u64(),
                cached: item["cache_read_input_tokens"].as_u64(),
                cache_write,
                output: item["output_tokens"].as_u64(),
                reasoning: None,
            };
            let total = [
                breakdown.input,
                breakdown.cached,
                breakdown.cache_write,
                breakdown.output,
            ]
            .into_iter()
            .flatten()
            .sum();
            let model = text(&item["model"]);
            events.push(Event {
                at,
                provider: Provider::Claude,
                model: model.clone(),
                total,
                breakdown: Some(breakdown),
                id: format!("admin:{at}:{model}"),
                pin: None,
            });
        }
    }
    events
}
