use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The value of an attribute: the `AnyValue` of OpenTelemetry.
///
/// Bytes arrive as a base64 string, as OTLP/JSON writes them, so they read
/// back as a string. A double that JSON cannot hold, such as NaN, is a string
/// too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
#[schema(no_recursion)]
pub enum AttributeValue {
    /// An empty value.
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    String(String),
    Array(Vec<Self>),
    Map(Attributes),
}

impl AttributeValue {
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Double(_) => "float",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Map(_) => "object",
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }
}

impl fmt::Display for AttributeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(&json)
    }
}

impl From<&str> for AttributeValue {
    fn from(text: &str) -> Self {
        Self::String(text.to_owned())
    }
}

impl From<String> for AttributeValue {
    fn from(text: String) -> Self {
        Self::String(text)
    }
}

impl From<bool> for AttributeValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for AttributeValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for AttributeValue {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

/// Attributes by key, in the order of their keys.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
#[schema(value_type = HashMap<String, AttributeValue>)]
pub struct Attributes(BTreeMap<String, AttributeValue>);

impl Attributes {
    #[must_use]
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&AttributeValue> {
        self.0.get(key)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<AttributeValue>) {
        self.0.insert(key.into(), value.into());
    }

    pub fn remove(&mut self, key: &str) -> Option<AttributeValue> {
        self.0.remove(key)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &AttributeValue)> {
        self.0.iter()
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("attributes serialize to JSON")
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

impl<K: Into<String>, V: Into<AttributeValue>> FromIterator<(K, V)> for Attributes {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        )
    }
}

impl<K: Into<String>, V: Into<AttributeValue>> Extend<(K, V)> for Attributes {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        self.0.extend(
            iter.into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
    }
}

impl IntoIterator for Attributes {
    type Item = (String, AttributeValue);
    type IntoIter = std::collections::btree_map::IntoIter<String, AttributeValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Attributes {
    type Item = (&'a String, &'a AttributeValue);
    type IntoIter = std::collections::btree_map::Iter<'a, String, AttributeValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl std::ops::Index<&str> for Attributes {
    type Output = AttributeValue;

    fn index(&self, key: &str) -> &AttributeValue {
        self.0.get(key).unwrap_or(&AttributeValue::Null)
    }
}

impl PartialEq<&str> for AttributeValue {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == Some(*other)
    }
}

/// Something that happened at one time in a span, such as an exception.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SpanEvent {
    /// Nanoseconds since the Unix epoch.
    #[serde(alias = "ts")]
    pub occurred_at: i64,
    pub name: String,
    pub attributes: Attributes,
}
