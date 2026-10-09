use std::collections::HashMap;

use otelo_indexed_storage::AttributeValue;
use otelo_query::{Expression, Field, Query};
use rusqlite::params;

use super::compile::{EncodedRecordAliases, equal_sql_values};
use crate::Reader;
use crate::attribute_encoding::{AttributeEncodings, AttributeKeyId, InternedValueId};
use crate::catalog::AttributeOwner;
use crate::indexes::attribute_json_path;
use crate::series::hash_fields;

#[derive(Clone, Copy, Debug, Default)]
pub struct StoredKey {
    pub key_id: Option<AttributeKeyId>,
    pub encodings: AttributeEncodings,
}

#[derive(Debug, Default)]
pub struct StoredAttributes {
    keys: HashMap<String, StoredKey>,
    interned_value_ids_by_json: HashMap<String, InternedValueId>,
}

impl StoredAttributes {
    pub fn stored_key(&self, key: &str) -> StoredKey {
        self.keys.get(key).copied().unwrap_or_default()
    }

    pub fn find_interned_value_id(&self, value_json: &str) -> Option<InternedValueId> {
        self.interned_value_ids_by_json.get(value_json).copied()
    }
}

pub fn format_text_value_json(text: &str) -> String {
    AttributeValue::String(text.to_owned()).to_string()
}

fn collect_compared_texts(expression: &Expression, texts: &mut Vec<String>) {
    match expression {
        Expression::And(terms) | Expression::Or(terms) => {
            for term in terms {
                collect_compared_texts(term, texts);
            }
        }
        Expression::Not(term) => collect_compared_texts(term, texts),
        Expression::Compare {
            field: Field::Attribute(_),
            value,
            ..
        } => texts.extend(list_equal_sql_texts(std::slice::from_ref(value))),
        Expression::In {
            field: Field::Attribute(_),
            values,
        } => texts.extend(list_equal_sql_texts(values)),
        Expression::Compare { .. }
        | Expression::In { .. }
        | Expression::Contains { .. }
        | Expression::Has(_) => {}
    }
}

fn list_equal_sql_texts(values: &[otelo_query::Value]) -> Vec<String> {
    values
        .iter()
        .flat_map(equal_sql_values)
        .filter_map(|value| match value {
            rusqlite::types::Value::Text(text) => Some(text),
            _ => None,
        })
        .collect()
}

impl Reader {
    pub(super) fn read_stored_attributes_of_query(
        &self,
        owner: AttributeOwner,
        query: &Query,
    ) -> anyhow::Result<StoredAttributes> {
        let keys: Vec<&str> = query
            .fields()
            .into_iter()
            .filter_map(|field| match field {
                Field::Attribute(key) => Some(key.as_str()),
                _ => None,
            })
            .collect();
        let mut compared_texts = Vec::new();
        if let Some(expression) = &query.expression {
            collect_compared_texts(expression, &mut compared_texts);
        }
        self.read_stored_attributes(owner, &keys, &compared_texts)
    }

    pub(super) fn read_stored_attributes(
        &self,
        owner: AttributeOwner,
        keys: &[&str],
        compared_texts: &[String],
    ) -> anyhow::Result<StoredAttributes> {
        let mut stored_attributes = StoredAttributes::default();
        if keys.is_empty() {
            return Ok(stored_attributes);
        }
        let keys_json = serde_json::to_string(keys)?;
        let mut key_ids_statement = self.connection().prepare_cached(
            "SELECT key, id
             FROM attribute_keys
             WHERE key IN (SELECT value FROM json_each(?1))",
        )?;
        let mut rows = key_ids_statement.query([&keys_json])?;
        while let Some(row) = rows.next()? {
            stored_attributes
                .keys
                .entry(row.get(0)?)
                .or_default()
                .key_id = Some(row.get(1)?);
        }
        let (first_day, last_day) = self.days_of_range();
        let mut encodings_statement = self.connection().prepare_cached(
            "SELECT key, max(has_stable_values), max(has_interned_values),
                    max(has_literal_values)
             FROM attribute_key_counts
             WHERE attribute_owner = ?1 AND day >= ?2 AND day <= ?3
               AND key IN (SELECT value FROM json_each(?4))
             GROUP BY key",
        )?;
        let mut rows =
            encodings_statement.query(params![owner.name(), first_day, last_day, keys_json])?;
        while let Some(row) = rows.next()? {
            stored_attributes
                .keys
                .entry(row.get(0)?)
                .or_default()
                .encodings = AttributeEncodings {
                has_stable_values: row.get(1)?,
                has_interned_values: row.get(2)?,
                has_literal_values: row.get(3)?,
            };
        }
        let value_jsons: Vec<String> = compared_texts
            .iter()
            .map(|text| format_text_value_json(text))
            .collect();
        if !value_jsons.is_empty() {
            let identity_hashes: Vec<i64> = value_jsons
                .iter()
                .map(|value_json| hash_fields(&[value_json]))
                .collect();
            let mut values_statement = self.connection().prepare_cached(
                "SELECT id, value
                 FROM interned_attribute_values
                 WHERE identity_hash IN (SELECT value FROM json_each(?1))",
            )?;
            let mut rows = values_statement.query([serde_json::to_string(&identity_hashes)?])?;
            while let Some(row) = rows.next()? {
                let value_json: String = row.get(1)?;
                if value_jsons.contains(&value_json) {
                    stored_attributes
                        .interned_value_ids_by_json
                        .insert(value_json, row.get(0)?);
                }
            }
        }
        Ok(stored_attributes)
    }
}

impl EncodedRecordAliases {
    pub fn encoded_records_sql(self, table: &str) -> String {
        let Self {
            record,
            stable_attribute_set,
            record_group,
            resource,
        } = self;
        format!(
            "{table} {record}
             JOIN stable_attribute_sets {stable_attribute_set}
               ON {stable_attribute_set}.id = {record}.stable_attribute_set_id
             JOIN record_groups {record_group} ON {record_group}.id = {stable_attribute_set}.record_group_id
             JOIN resources {resource} ON {resource}.id = {record_group}.resource_id"
        )
    }

    // CROSS JOIN keeps the stable sets the outer loop, so a query of every service seeks the
    // records of each set in its index instead of reading every record of the range.
    pub fn encoded_records_by_stable_attribute_set_sql(self, table: &str) -> String {
        let Self {
            record,
            stable_attribute_set,
            record_group,
            resource,
        } = self;
        format!(
            "stable_attribute_sets {stable_attribute_set}
             CROSS JOIN {table} {record}
               ON {record}.stable_attribute_set_id = {stable_attribute_set}.id
             JOIN record_groups {record_group} ON {record_group}.id = {stable_attribute_set}.record_group_id
             JOIN resources {resource} ON {resource}.id = {record_group}.resource_id"
        )
    }

    pub fn record_attributes_json_sql(self) -> String {
        let Self {
            record,
            stable_attribute_set,
            ..
        } = self;
        format!(
            "(SELECT '{{'
                     || coalesce(group_concat(json_quote(attribute.key) || ':' || attribute.value_json,
                                              ','), '')
                     || '}}'
              FROM (SELECT stable_entry.key AS key,
                           {stable_attribute_set}.attributes -> stable_entry.fullkey AS value_json
                    FROM json_each({stable_attribute_set}.attributes) stable_entry
                    UNION ALL
                    SELECT attribute_key.key, interned_attribute_value.value
                    FROM json_each({record}.interned_attributes) interned_entry
                    JOIN attribute_keys attribute_key
                      ON attribute_key.id = CAST(interned_entry.key AS INTEGER)
                    JOIN interned_attribute_values interned_attribute_value
                      ON interned_attribute_value.id = interned_entry.value
                    UNION ALL
                    SELECT attribute_key.key,
                           {record}.literal_attributes -> literal_entry.fullkey
                    FROM json_each({record}.literal_attributes) literal_entry
                    JOIN attribute_keys attribute_key
                      ON attribute_key.id = CAST(literal_entry.key AS INTEGER)) attribute)"
        )
    }

    pub fn attribute_json_sql(self, key: &str, stored_key: StoredKey) -> String {
        let Self {
            record,
            stable_attribute_set,
            ..
        } = self;
        let stable_json = format!(
            "{stable_attribute_set}.attributes -> {}",
            attribute_json_path(key)
        );
        let Some(key_id) = stored_key.key_id else {
            return stable_json;
        };
        let key_path = key_id.json_path();
        format!(
            "coalesce({stable_json},
                      (SELECT interned_attribute_value.value
                       FROM interned_attribute_values interned_attribute_value
                       WHERE interned_attribute_value.id = json_extract({record}.interned_attributes, {key_path})),
                      {record}.literal_attributes -> {key_path})"
        )
    }

    pub fn attribute_text_sql(self, key: &str, stored_key: StoredKey) -> String {
        format!("({}) ->> '$'", self.attribute_json_sql(key, stored_key))
    }
}
