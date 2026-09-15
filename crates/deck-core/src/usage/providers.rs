use super::types::{QuotaAccount, QuotaWindow};
use crate::catalog::Provider;
use chrono::{DateTime, Utc};
use serde_json::Value;

pub(super) fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|value| value.is_finite())
}
pub(super) fn text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        _ => String::new(),
    }
}
pub(super) fn timestamp(value: &Value) -> Option<i64> {
    if let Some(value) = number(value) {
        return Some(if value > 10_000_000_000. {
            value / 1000.
        } else {
            value
        } as i64);
    }
    DateTime::parse_from_rfc3339(value.as_str()?)
        .ok()
        .map(|time| time.timestamp())
}
pub(super) fn iso(value: &Value) -> String {
    timestamp(value)
        .and_then(|time| DateTime::<Utc>::from_timestamp(time, 0))
        .map(|time| time.to_rfc3339())
        .unwrap_or_default()
}
pub(super) fn root(value: &Value) -> &Value {
    if value.get("data").is_some_and(Value::is_object) {
        &value["data"]
    } else {
        value
    }
}
fn list(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn quota(
    name: &str,
    used: Option<f64>,
    limit: Option<f64>,
    reset: Option<i64>,
) -> Option<QuotaWindow> {
    match (used, limit) {
        (Some(used), Some(limit)) if limit > 0. => Some(QuotaWindow::percent(
            name,
            (1. - used / limit) * 100.,
            reset,
        )),
        (Some(used), _) => Some(QuotaWindow::text(name, format!("已用 {used} · 上限未知"))),
        _ => None,
    }
}
pub(super) fn title_case(value: &str) -> String {
    value
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().to_string() + &chars.as_str().to_lowercase())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub(super) fn parse_primary(
    provider: Provider,
    data: &Value,
    output: &mut QuotaAccount,
) -> Result<(), &'static str> {
    if !data.is_object() {
        return Err("额度服务返回了无效 JSON 结构");
    }
    match provider {
        Provider::Openai => {
            for key in ["primary_window", "secondary_window"] {
                let value = &data["rate_limit"][key];
                if let (Some(seconds), Some(used)) = (
                    number(&value["limit_window_seconds"]),
                    number(&value["used_percent"]),
                ) {
                    let name = match seconds as i64 {
                        18000 => "5h quota".into(),
                        604800 => "Week quota".into(),
                        2592000 | 2628000 => "Month quota".into(),
                        value => format!("{}h quota", value / 3600),
                    };
                    output.windows.push(QuotaWindow::percent(
                        name,
                        100. - used,
                        timestamp(&value["reset_at"]),
                    ));
                }
            }
            if let Some(remaining) =
                number(&data["spend_control"]["individual_limit"]["remaining_percent"])
            {
                output.windows.push(QuotaWindow::percent(
                    "Quota",
                    remaining,
                    timestamp(&data["spend_control"]["individual_limit"]["reset_at"]),
                ));
            }
            let credits = &data["rate_limit_reset_credits"];
            if let Some(count) = number(&credits["available_count"])
                .or_else(|| number(&credits["applicable_available_count"]))
            {
                output.windows.push(QuotaWindow::text(
                    "重置次数",
                    format!("剩余 {} 次", count.max(0.) as u64),
                ));
            }
            output.plan = openai_plan(&text(&data["plan_type"]));
            output.plan_detail = format!("usage.plan_type={}", text(&data["plan_type"]));
        }
        Provider::Claude => {
            let mut roots = vec![data];
            roots.extend(
                ["quota", "usage", "rate_limits", "rateLimits", "oauth_usage"]
                    .into_iter()
                    .filter_map(|key| data.get(key).filter(|value| value.is_object())),
            );
            for data in roots {
                for (key, value) in data.as_object().expect("validated object") {
                    let label = match key.to_lowercase().replace('_', "").as_str() {
                        "fivehour" => "5h quota",
                        "sevenday" => "Week quota",
                        "sevendayoauth" => "OAuth Week quota",
                        "sevendayoauthapps" => "OAuth Apps Week quota",
                        "sevendayopus" => "Opus Week quota",
                        "sevendaysonnet" => "Sonnet Week quota",
                        "sevendaycowork" => "Cowork Week quota",
                        "extrausage" => "Extra usage",
                        _ => "",
                    };
                    if !label.is_empty() {
                        claude_window(label, value, &mut output.windows);
                    }
                }
                for item in list(&data["limits"]) {
                    let label = match item["kind"].as_str() {
                        Some("session") => "5h quota".into(),
                        Some("weekly_all") => "Week quota".into(),
                        Some(value) if value.starts_with("weekly_") => {
                            format!("{} Week quota", title_case(&value[7..]))
                        }
                        _ => String::new(),
                    };
                    if !label.is_empty() {
                        claude_window(&label, item, &mut output.windows);
                    }
                }
            }
        }
        Provider::Grok => {
            let config = data
                .get("config")
                .filter(|value| value.is_object())
                .unwrap_or(data);
            if let Some(used) = number(&config["creditUsagePercent"]) {
                output.windows.push(QuotaWindow::percent(
                    "Week quota",
                    100. - used,
                    timestamp(
                        config["currentPeriod"]
                            .get("end")
                            .unwrap_or(&config["billingPeriodEnd"]),
                    ),
                ));
            }
            output.sub_end = iso(config["currentPeriod"]
                .get("end")
                .unwrap_or(&config["billingPeriodEnd"]));
        }
        Provider::Zai | Provider::Zhipu => {
            if data["success"] == false || number(&data["code"]).is_some_and(|code| code >= 400.) {
                return Err("额度接口报告失败，请检查账号和服务状态");
            }
            for item in list(&root(data)["limits"]) {
                let name = match (item["type"].as_str(), item["unit"].as_i64()) {
                    (Some("TOKENS_LIMIT" | "CREDIT_LIMIT"), Some(3)) => "5h quota",
                    (Some("TOKENS_LIMIT" | "CREDIT_LIMIT"), Some(6)) => "Week quota",
                    (Some("TIME_LIMIT"), _) => "Quota",
                    _ => continue,
                };
                if let Some(used) = number(&item["percentage"]) {
                    output.windows.push(QuotaWindow::percent(
                        name,
                        100. - used,
                        timestamp(&item["nextResetTime"]),
                    ));
                }
            }
        }
        Provider::Kimi => {
            let data = root(data);
            kimi_window("Week quota", &data["usage"], &mut output.windows);
            for (index, item) in list(&data["limits"]).iter().enumerate() {
                let detail = item
                    .get("detail")
                    .filter(|value| value.is_object())
                    .unwrap_or(item);
                let name = item
                    .get("name")
                    .or_else(|| item.get("title"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        let window = item
                            .get("window")
                            .filter(|value| value.is_object())
                            .unwrap_or(item);
                        match (number(&window["duration"]), window["timeUnit"].as_str()) {
                            (Some(value), Some(unit))
                                if unit.contains("MINUTE") && value % 60. == 0. =>
                            {
                                format!("{}h quota", value / 60.)
                            }
                            (Some(value), Some(unit)) if unit.contains("MINUTE") => {
                                format!("{value}m quota")
                            }
                            (Some(value), Some(unit)) if unit.contains("HOUR") => {
                                format!("{value}h quota")
                            }
                            (Some(value), Some(unit)) if unit.contains("DAY") => {
                                format!("{value}d quota")
                            }
                            _ => format!("Limit #{}", index + 1),
                        }
                    });
                kimi_window(&name, detail, &mut output.windows);
            }
            let level = text(&data["user"]["membership"]["level"]);
            if !level.is_empty() {
                output.plan = format!(
                    "Kimi Code {}{}",
                    title_case(level.strip_prefix("LEVEL_").unwrap_or(&level)),
                    if text(&data["subType"]).contains("TRIAL") {
                        " 试用"
                    } else {
                        ""
                    }
                );
            }
            output.plan_detail = format!(
                "membership.level={level} · subType={}",
                text(&data["subType"])
            );
            if let Some(email) = data["user"]["email"].as_str() {
                output.email = email.into();
            }
        }
        Provider::Deepseek => {
            for balance in list(&data["balance_infos"]) {
                let currency = text(&balance["currency"]);
                let symbol = match currency.as_str() {
                    "CNY" => "¥",
                    "USD" => "$",
                    _ => &currency,
                };
                let mut label = format!("{symbol}{}", text(&balance["total_balance"]));
                if balance.get("granted_balance").is_some()
                    && balance.get("topped_up_balance").is_some()
                {
                    label += &format!(
                        " · 赠送 {symbol}{} / 充值 {symbol}{}",
                        text(&balance["granted_balance"]),
                        text(&balance["topped_up_balance"])
                    );
                }
                output.windows.push(QuotaWindow::text("Balance", label));
            }
            output.plan = match data["is_available"].as_bool() {
                Some(true) => "可用",
                Some(false) => "不可用（余额不足或欠费）",
                None => "可用性未知",
            }
            .into();
        }
        Provider::Cursor => {
            let billing = data
                .get("billing")
                .filter(|value| value.is_object())
                .unwrap_or(data);
            for (name, used, limit) in [
                ("Total", "totalSpend", "totalLimit"),
                ("Auto + Composer", "autoSpend", "autoLimit"),
                ("API", "apiSpend", "apiLimit"),
            ] {
                if let Some(value) =
                    quota(name, number(&billing[used]), number(&billing[limit]), None)
                {
                    output.windows.push(value);
                }
            }
            if let Some(usage) = data["usage"].as_object() {
                for (name, item) in usage {
                    let reset = timestamp(item.get("resetAt").unwrap_or(&item["reset_at"]));
                    if let Some(used) =
                        number(&item["usedPercent"]).or_else(|| number(&item["percentage"]))
                    {
                        output
                            .windows
                            .push(QuotaWindow::percent(name, 100. - used, reset));
                    } else if let Some(value) = quota(
                        name,
                        number(&item["used"]),
                        number(item.get("limit").unwrap_or(&item["total"])),
                        reset,
                    ) {
                        output.windows.push(value);
                    }
                }
            }
            output.plan = text(
                data.get("membershipType")
                    .or_else(|| data.get("plan"))
                    .unwrap_or(&data["individualMembershipType"]),
            );
        }
        Provider::CursorAgent => {
            let usage = &data["planUsage"];
            for (name, field) in [
                ("Included", "totalPercentUsed"),
                ("Auto", "autoPercentUsed"),
                ("API", "apiPercentUsed"),
            ] {
                if let Some(used) = number(&usage[field]) {
                    output.windows.push(QuotaWindow::percent(
                        name,
                        100. - used,
                        timestamp(&data["billingCycleEnd"]),
                    ));
                }
            }
            output.sub_end = iso(&data["billingCycleEnd"]);
        }
        Provider::Antigravity => {
            for group in list(&data["groups"]) {
                for bucket in list(&group["buckets"]) {
                    if bucket["disabled"] == true {
                        continue;
                    }
                    let label = match bucket["bucketId"].as_str() {
                        Some("gemini-weekly") => "Gemini Week",
                        Some("gemini-5h") => "Gemini 5h",
                        Some("3p-weekly") => "Claude/GPT Week",
                        Some("3p-5h") => "Claude/GPT 5h",
                        _ => continue,
                    };
                    if let Some(remaining) = number(&bucket["remainingFraction"]) {
                        output.windows.push(QuotaWindow::percent(
                            label,
                            (remaining * 1000.).round() / 10.,
                            timestamp(&bucket["resetTime"]),
                        ));
                    }
                }
            }
            if output.windows.is_empty() {
                for (model, label) in [
                    ("gemini-2.5-pro", "Gemini 5h"),
                    ("claude-sonnet-4-6", "Claude/GPT 5h"),
                    ("claude-opus-4-6-thinking", "Claude/GPT 5h"),
                ] {
                    let quota = &data["models"][model]["quotaInfo"];
                    if let Some(remaining) = number(&quota["remainingFraction"])
                        && !output.windows.iter().any(|window| window.name == label)
                    {
                        output.windows.push(QuotaWindow::percent(
                            label,
                            remaining * 100.,
                            timestamp(&quota["resetTime"]),
                        ));
                    }
                }
            }
        }
    }
    if output.windows.is_empty() {
        return Err("接口未返回可识别的额度数据");
    }
    output.ok = true;
    Ok(())
}
fn claude_window(name: &str, value: &Value, windows: &mut Vec<QuotaWindow>) {
    if windows.iter().any(|window| window.name == name) {
        return;
    }
    let used = [
        "utilization",
        "used_percentage",
        "usedPercentage",
        "used_percent",
        "usedPercent",
        "percent_used",
        "percentUsed",
        "percent",
    ]
    .iter()
    .find_map(|field| number(&value[field]));
    let reset = ["resets_at", "resetsAt", "reset_at", "resetAt"]
        .iter()
        .find_map(|field| timestamp(&value[field]));
    if let Some(used) = used {
        windows.push(QuotaWindow::percent(name, 100. - used, reset));
    }
}
fn kimi_window(name: &str, value: &Value, windows: &mut Vec<QuotaWindow>) {
    let limit = number(&value["limit"]);
    let used = number(&value["used"]).or_else(|| Some(limit? - number(&value["remaining"])?));
    let reset = ["reset_at", "resetAt", "reset_time", "resetTime"]
        .iter()
        .find_map(|field| timestamp(&value[field]));
    let name = value
        .get("name")
        .or_else(|| value.get("title"))
        .and_then(Value::as_str)
        .unwrap_or(name);
    if let Some(window) = quota(name, used, limit, reset) {
        windows.push(window);
    }
}
pub(super) fn openai_plan(value: &str) -> String {
    let key = value.to_lowercase().replace(['_', '-', ' '], "");
    let label = match key.as_str() {
        "free" => "Free",
        "go" => "Go",
        "plus" => "Plus",
        "pro" | "promax" | "pro20x" | "codexpro20x" => "Pro 20x",
        "prolite" | "pro5x" | "codexpro5x" => "Pro 5x",
        "team" => "Team",
        "business" => "Business",
        "enterprise" => "Enterprise",
        "edu" => "Edu",
        _ => value,
    };
    if value.is_empty() {
        "OpenAI".into()
    } else {
        format!("OpenAI ({label})")
    }
}
pub(super) fn claude_profile(data: &Value, output: &mut QuotaAccount) {
    let (account, organization) = (&data["account"], &data["organization"]);
    if let Some(email) = account["email"].as_str() {
        output.email = email.into();
    }
    let kind = organization["organization_type"].as_str().unwrap_or(
        if account["has_claude_max"] == true {
            "claude_max"
        } else if account["has_claude_pro"] == true {
            "claude_pro"
        } else {
            ""
        },
    );
    output.plan = title_case(kind);
    let tier = text(&organization["rate_limit_tier"]);
    for component in tier.split('_') {
        if let Some(value) = component.strip_suffix('x')
            && value.parse::<u32>().is_ok()
            && !output.plan.contains(component)
        {
            output.plan += &format!(" {component}");
        }
    }
    output.plan_detail = format!("organization_type={kind} · rate_limit_tier={tier}");
}
pub(super) fn grok_details(
    task: Option<&Value>,
    user: Option<&Value>,
    subscriptions: Option<&Value>,
    output: &mut QuotaAccount,
) {
    if let Some(task) = task {
        for (label, used, total) in [
            ("高频任务", "frequentUsage", "frequentLimit"),
            ("普通任务", "occasionalUsage", "occasionalLimit"),
        ] {
            if let Some(window) = quota(label, number(&task[used]), number(&task[total]), None) {
                output.windows.push(window);
            }
        }
    }
    let mut reported = String::new();
    if let Some(user) = user {
        let user = user.get("user").unwrap_or(user);
        if let Some(email) = user["email"].as_str() {
            output.email = email.into();
        }
        reported = text(&user["subscriptionTier"]);
    }
    let active = subscriptions.and_then(|value| {
        list(&value["subscriptions"])
            .iter()
            .find(|item| item["status"] == "SUBSCRIPTION_STATUS_ACTIVE")
    });
    let tier = active
        .map(|item| text(&item["tier"]))
        .filter(|value| !value.is_empty())
        .unwrap_or(reported);
    output.plan = match tier
        .to_lowercase()
        .replace(['_', ' '], "")
        .replace("subscriptiontier", "")
        .as_str()
    {
        "supergrokheavy" => "xAI Heavy".into(),
        "supergrokpro" => "xAI SuperGrok Pro".into(),
        "supergroklite" => "xAI Lite".into(),
        "supergrok" => "xAI SuperGrok".into(),
        "" => String::new(),
        _ => format!("xAI {tier}"),
    };
    output.plan_detail = format!("subscriptionTier={tier}");
}
