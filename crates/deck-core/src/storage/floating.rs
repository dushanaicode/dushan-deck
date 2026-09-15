use super::Storage;
use crate::{
    error::{DeckError, Result},
    settings::floating::{FloatPreferences, FloatState, validate_background},
};
use rusqlite::params;

impl Storage {
    pub(crate) fn float_state(&self) -> Result<FloatState> {
        let db = self.connect()?;
        let (preferences, width, height, background): (String, u32, u32, Option<String>) = db
            .query_row(
                "SELECT preferences, width, height, background FROM float_settings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        Ok(FloatState {
            preferences: serde_json::from_str(&preferences)?,
            width,
            height,
            background,
        })
    }
    pub(crate) fn save_float_preferences(&self, preferences: FloatPreferences) -> Result<()> {
        preferences.validate()?;
        self.connect()?.execute(
            "UPDATE float_settings SET preferences = ?1 WHERE id = 1",
            [serde_json::to_string(&preferences)?],
        )?;
        Ok(())
    }
    pub(crate) fn save_float_size(&self, width: u32, height: u32) -> Result<()> {
        if !(220..=7680).contains(&width) || !(260..=4320).contains(&height) {
            return Err(DeckError::Invalid("悬浮窗尺寸超出允许范围"));
        }
        self.connect()?.execute(
            "UPDATE float_settings SET width = ?1, height = ?2 WHERE id = 1",
            params![width, height],
        )?;
        Ok(())
    }
    pub(crate) fn save_float_background(&self, background: Option<String>) -> Result<()> {
        if let Some(data) = &background {
            validate_background(data)?;
        }
        self.connect()?.execute(
            "UPDATE float_settings SET background = ?1 WHERE id = 1",
            [background],
        )?;
        Ok(())
    }
}
