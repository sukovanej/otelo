use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub const MAX_PAGE_ROWS: usize = 1000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct PageCursor {
    order_values: Vec<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRequest {
    pub after: Option<PageCursor>,
    pub limit: usize,
}

impl PageCursor {
    #[must_use]
    pub const fn new(order_values: Vec<i64>) -> Self {
        Self { order_values }
    }

    #[must_use]
    pub fn order_values(&self) -> &[i64] {
        &self.order_values
    }
}

impl PageRequest {
    #[must_use]
    pub const fn first(limit: usize) -> Self {
        Self { after: None, limit }
    }
}

impl fmt::Display for PageCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let order_values: Vec<String> = self.order_values.iter().map(i64::to_string).collect();
        formatter.write_str(&order_values.join("."))
    }
}

impl FromStr for PageCursor {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let order_values = text
            .split('.')
            .map(str::parse)
            .collect::<Result<Vec<i64>, _>>()
            .map_err(|_| format!("{text:?} is not the next of a page"))?;
        Ok(Self { order_values })
    }
}

impl From<PageCursor> for String {
    fn from(cursor: PageCursor) -> Self {
        cursor.to_string()
    }
}

impl TryFrom<String> for PageCursor {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}
