use otelo_indexed_storage::AttributeValue;
use rusqlite::ToSql;
use rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttributeEncoding {
    Stable,
    Interned,
    Literal,
}

impl AttributeEncoding {
    pub const ALL: [Self; 3] = [Self::Stable, Self::Interned, Self::Literal];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Interned => "interned",
            Self::Literal => "literal",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|encoding| encoding.name() == text)
    }

    // A number, a bool, or a null on the row takes fewer bytes than the id of an interned value.
    pub const fn fit_to_value(self, value: &AttributeValue) -> Self {
        match (self, value) {
            (
                Self::Interned,
                AttributeValue::Null
                | AttributeValue::Bool(_)
                | AttributeValue::Int(_)
                | AttributeValue::Double(_),
            ) => Self::Literal,
            (encoding, _) => encoding,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttributeEncodings {
    pub has_stable_values: bool,
    pub has_interned_values: bool,
    pub has_literal_values: bool,
}

impl AttributeEncodings {
    pub const fn add_encoding(&mut self, encoding: AttributeEncoding) {
        match encoding {
            AttributeEncoding::Stable => self.has_stable_values = true,
            AttributeEncoding::Interned => self.has_interned_values = true,
            AttributeEncoding::Literal => self.has_literal_values = true,
        }
    }

    pub const fn is_only_stable(self) -> bool {
        self.has_stable_values && !self.has_interned_values && !self.has_literal_values
    }
}

macro_rules! row_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(pub i64);

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                self.0.to_sql()
            }
        }

        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                i64::column_result(value).map(Self)
            }
        }
    };
}

row_id!(AttributeKeyId);
row_id!(RecordGroupId);
row_id!(StableAttributeSetId);
row_id!(InternedValueId);

impl AttributeKeyId {
    pub fn json_path(self) -> String {
        format!("'$.\"{}\"'", self.0)
    }
}
