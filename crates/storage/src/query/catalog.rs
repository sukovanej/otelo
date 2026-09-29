use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The attribute keys of a signal, over the attached days.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct AttributeKeys {
    /// The attributes of the records: of the logs, the spans, or the labels
    /// of the series.
    pub record: Vec<Attribute>,
    /// The attributes of the resources that sent them.
    pub resource: Vec<Attribute>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Attribute {
    pub key: String,
    /// The JSON type of the values: `string`, `int`, `float`, `bool`,
    /// `array`, `object`, or `mixed`.
    #[serde(rename = "type")]
    pub kind: String,
    /// How many records have the key. For resources and series, how many
    /// resources and series.
    pub count: u64,
    /// Whether the key has an index.
    pub indexed: bool,
}
