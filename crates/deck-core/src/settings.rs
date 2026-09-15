use serde::{Deserialize, Serialize};

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
        if self.favorite_providers.len() > 2
            || (self.favorite_providers.len() == 2
                && self.favorite_providers[0] == self.favorite_providers[1])
        {
            return Err(DeckError::Invalid("收藏专区不能重复"));
        }
        Ok(())
    }
}
