use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductScope {
    #[default]
    Included,
    ExcludedOtherTitlePromotion,
}

impl ProductScope {
    pub fn is_included(self) -> bool {
        self == Self::Included
    }
}
