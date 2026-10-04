use std::cmp::Ordering;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Which end of a ranking comes first: the groups with the `highest` number, or
/// the ones with the `lowest`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RankOrder {
    #[default]
    Highest,
    Lowest,
}

impl RankOrder {
    pub const ALL: [Self; 2] = [Self::Highest, Self::Lowest];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Highest => "highest",
            Self::Lowest => "lowest",
        }
    }

    #[must_use]
    pub const fn orient_ordering(self, lowest_first: Ordering) -> Ordering {
        match self {
            Self::Highest => lowest_first.reverse(),
            Self::Lowest => lowest_first,
        }
    }
}

impl FromStr for RankOrder {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|order| order.name() == name)
            .ok_or_else(|| format!("{name:?} is not an order: highest or lowest"))
    }
}
