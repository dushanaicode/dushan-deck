use crate::error::{DeckError, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FloatTheme {
    Dark,
    Ocean,
    Forest,
    Violet,
    Paper,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum UsagePeriod {
    #[serde(rename = "1d")]
    Day,
    #[serde(rename = "3d")]
    ThreeDays,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
    #[serde(rename = "all")]
    All,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FloatPreferences {
    pub alpha: u8,
    pub on_top: bool,
    pub rounded: bool,
    pub theme: FloatTheme,
    pub animations: bool,
    pub bg_dim: u8,
    pub bg_blur: u8,
    pub glass_blur: u8,
    pub show: BTreeMap<String, bool>,
    pub usage_period: UsagePeriod,
    pub usage_harnesses: BTreeMap<String, String>,
    pub reset_modes: BTreeMap<String, bool>,
    pub card_order: Vec<String>,
}

impl Default for FloatPreferences {
    fn default() -> Self {
        Self {
            alpha: 82,
            on_top: true,
            rounded: true,
            theme: FloatTheme::Dark,
            animations: false,
            bg_dim: 55,
            bg_blur: 8,
            glass_blur: 16,
            show: BTreeMap::new(),
            usage_period: UsagePeriod::Month,
            usage_harnesses: BTreeMap::new(),
            reset_modes: BTreeMap::new(),
            card_order: vec![],
        }
    }
}

impl FloatPreferences {
    pub fn validate(&self) -> Result<()> {
        validate_alpha(self.alpha)?;
        if self.bg_dim > 90 || self.bg_blur > 30 || self.glass_blur > 30 {
            return Err(DeckError::Invalid("背景参数超出允许范围"));
        }
        if self.show.len() > 1000
            || self.usage_harnesses.len() > 1000
            || self.reset_modes.len() > 5000
            || self.card_order.len() > 1000
        {
            return Err(DeckError::Invalid("悬浮窗设置条目过多"));
        }
        if self
            .show
            .keys()
            .chain(self.usage_harnesses.keys())
            .chain(self.reset_modes.keys())
            .chain(self.card_order.iter())
            .any(|key| key.is_empty() || key.len() > 512 || key.chars().any(char::is_control))
        {
            return Err(DeckError::Invalid("悬浮窗设置标识无效"));
        }
        if self.usage_harnesses.values().any(|value| value.len() > 100)
            || self.card_order.iter().collect::<BTreeSet<_>>().len() != self.card_order.len()
        {
            return Err(DeckError::Invalid("客户端筛选或卡片顺序无效"));
        }
        Ok(())
    }
}

pub fn validate_alpha(alpha: u8) -> Result<()> {
    if !(20..=100).contains(&alpha) {
        return Err(DeckError::Invalid("不透明度须为 20–100%"));
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FloatState {
    pub preferences: FloatPreferences,
    pub width: u32,
    pub height: u32,
    pub background: Option<String>,
}

pub fn validate_background(data_url: &str) -> Result<()> {
    if data_url.len() > 40 * 1024 * 1024 + 64 {
        return Err(DeckError::Invalid("图片超过 30MB"));
    }
    let (prefix, encoded) = data_url
        .split_once(",")
        .ok_or(DeckError::Invalid("图片数据格式无效"))?;
    let data = STANDARD
        .decode(encoded)
        .map_err(|_| DeckError::Invalid("图片数据格式无效"))?;
    let matches = match prefix {
        "data:image/png;base64" => data.starts_with(b"\x89PNG\r\n\x1a\n"),
        "data:image/jpeg;base64" => data.starts_with(b"\xff\xd8\xff"),
        "data:image/gif;base64" => data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"),
        "data:image/bmp;base64" => data.starts_with(b"BM"),
        "data:image/webp;base64" => data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP"),
        _ => false,
    };
    if !matches {
        return Err(DeckError::Invalid(
            "请选择 PNG、JPEG、WebP、GIF 或 BMP 图片",
        ));
    }
    Ok(())
}
