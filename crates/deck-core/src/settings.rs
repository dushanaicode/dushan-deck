use serde::{Deserialize, Serialize};
pub mod floating;

use crate::{
    catalog::Provider,
    error::{DeckError, Result},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub favorite_providers: Vec<Provider>,
    pub float_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            favorite_providers: vec![Provider::Claude, Provider::Openai],
            float_enabled: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.favorite_providers.len() > Provider::ALL.len()
            || self
                .favorite_providers
                .iter()
                .enumerate()
                .any(|(index, provider)| self.favorite_providers[..index].contains(provider))
        {
            return Err(DeckError::Invalid("收藏专区不能重复"));
        }
        Ok(())
    }
}
