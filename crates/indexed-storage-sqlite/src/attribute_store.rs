use std::collections::{BTreeMap, HashMap, HashSet};

use otelo_indexed_storage::{AttributeValue, Attributes};
use rusqlite::{Connection, Transaction, params};

use crate::attribute_encoding::{
    AttributeEncoding, AttributeKeyId, InternedValueId, RecordGroupId, StableAttributeSetId,
};
use crate::attribute_profiler::{AttributeProfiler, GroupProfile, KeyProfile};
use crate::catalog::{AttributeOwner, CatalogCache, CatalogDelta};
use crate::day::Day;
use crate::series::{ResourceId, hash_fields};

// Past this many, a cache of sets or values starts over, and the indexer reads them from the file.
const MAX_CACHED_ROWS: usize = 20_000;

pub struct RecordOfGroup<'a> {
    pub attribute_owner: AttributeOwner,
    pub resource_id: ResourceId,
    pub name: &'a str,
    pub day: Day,
    pub received_at: i64,
}

pub struct EncodedAttributes {
    pub stable_attribute_set_id: StableAttributeSetId,
    pub interned_attributes: String,
    pub literal_attributes: String,
}

#[derive(Clone, Copy)]
struct CachedRow<Id> {
    id: Id,
    newest_record_day: Day,
}

#[derive(Default)]
pub struct AttributeStore {
    key_ids: HashMap<String, AttributeKeyId>,
    group_ids: HashMap<(AttributeOwner, ResourceId, String), RecordGroupId>,
    stable_sets_by_identity_hash: HashMap<i64, CachedRow<StableAttributeSetId>>,
    interned_values_by_identity_hash: HashMap<i64, CachedRow<InternedValueId>>,
    profiler: AttributeProfiler,
}

struct ProfiledValue<'a> {
    key: &'a str,
    key_id: AttributeKeyId,
    value: &'a AttributeValue,
    value_json: String,
    value_hash: i64,
}

impl AttributeStore {
    pub fn forget_rows_past_retention(&mut self, deleted_group_ids: &HashSet<RecordGroupId>) {
        self.group_ids.clear();
        self.stable_sets_by_identity_hash.clear();
        self.interned_values_by_identity_hash.clear();
        for &group_id in deleted_group_ids {
            self.profiler.forget_group(group_id);
        }
    }

    pub fn encode_attributes(
        &mut self,
        transaction: &Transaction,
        catalog: &mut CatalogCache,
        catalog_delta: &mut CatalogDelta,
        record: &RecordOfGroup,
        attributes: &Attributes,
    ) -> anyhow::Result<EncodedAttributes> {
        let group_id = self.find_or_insert_group(transaction, record)?;
        let mut profiled_values = Vec::with_capacity(attributes.len());
        for (key, value) in attributes {
            let value_json = value.to_string();
            profiled_values.push(ProfiledValue {
                key,
                key_id: find_or_insert_key_id(transaction, &mut self.key_ids, key)?,
                value,
                value_hash: hash_fields(&[&value_json]),
                value_json,
            });
        }
        let group = self.load_group_profile(transaction, group_id)?;
        let value_hashes_by_key: Vec<(AttributeKeyId, u64)> = profiled_values
            .iter()
            .map(|profiled| (profiled.key_id, profiled.value_hash.cast_unsigned()))
            .collect();
        if let Some(key_profiles) = group.observe_record(record.received_at, &value_hashes_by_key) {
            write_key_profiles(transaction, group_id, &key_profiles)?;
        }
        let encodings: Vec<AttributeEncoding> = profiled_values
            .iter()
            .map(|profiled| {
                group
                    .encoding_of_key(profiled.key_id)
                    .fit_to_value(profiled.value)
            })
            .collect();
        let mut stable_attributes = Attributes::new();
        let mut interned_value_ids: BTreeMap<AttributeKeyId, InternedValueId> = BTreeMap::new();
        let mut literal_values: BTreeMap<AttributeKeyId, &AttributeValue> = BTreeMap::new();
        for (profiled, encoding) in profiled_values.iter().zip(encodings) {
            catalog.count_encoded_value(
                catalog_delta,
                record.day,
                record.attribute_owner,
                profiled.key,
                profiled.value,
                encoding,
            );
            match encoding {
                AttributeEncoding::Stable => {
                    stable_attributes.insert(profiled.key, profiled.value.clone());
                }
                AttributeEncoding::Interned => {
                    let value_id = self.find_or_insert_interned_value(
                        transaction,
                        profiled.value_hash,
                        &profiled.value_json,
                        record.day,
                    )?;
                    interned_value_ids.insert(profiled.key_id, value_id);
                }
                AttributeEncoding::Literal => {
                    literal_values.insert(profiled.key_id, profiled.value);
                }
            }
        }
        let stable_attribute_set_id = self.find_or_insert_stable_set(
            transaction,
            group_id,
            &stable_attributes.to_json(),
            record.day,
        )?;
        Ok(EncodedAttributes {
            stable_attribute_set_id,
            interned_attributes: serde_json::to_string(&interned_value_ids)?,
            literal_attributes: serde_json::to_string(&literal_values)?,
        })
    }

    fn find_or_insert_group(
        &mut self,
        transaction: &Transaction,
        record: &RecordOfGroup,
    ) -> anyhow::Result<RecordGroupId> {
        let group_key = (
            record.attribute_owner,
            record.resource_id,
            record.name.to_owned(),
        );
        if let Some(&group_id) = self.group_ids.get(&group_key) {
            return Ok(group_id);
        }
        transaction
            .prepare_cached(
                "INSERT INTO record_groups (attribute_owner, resource_id, name) VALUES (?1, ?2, ?3)
                 ON CONFLICT (resource_id, attribute_owner, name) DO NOTHING",
            )?
            .execute(params![
                record.attribute_owner.name(),
                record.resource_id,
                record.name
            ])?;
        let group_id = transaction
            .prepare_cached(
                "SELECT id
                 FROM record_groups
                 WHERE resource_id = ?1 AND attribute_owner = ?2 AND name = ?3",
            )?
            .query_row(
                params![
                    record.resource_id,
                    record.attribute_owner.name(),
                    record.name
                ],
                |row| row.get(0),
            )?;
        self.group_ids.insert(group_key, group_id);
        Ok(group_id)
    }

    fn load_group_profile(
        &mut self,
        connection: &Connection,
        group_id: RecordGroupId,
    ) -> anyhow::Result<&mut GroupProfile> {
        if self.profiler.find_group(group_id).is_none() {
            let encodings = read_key_encodings(connection, group_id)?;
            self.profiler
                .insert_group(group_id, GroupProfile::new(encodings));
        }
        Ok(self
            .profiler
            .find_group(group_id)
            .expect("the group was just inserted"))
    }

    fn find_or_insert_interned_value(
        &mut self,
        transaction: &Transaction,
        identity_hash: i64,
        value_json: &str,
        day: Day,
    ) -> anyhow::Result<InternedValueId> {
        if self.interned_values_by_identity_hash.len() >= MAX_CACHED_ROWS {
            self.interned_values_by_identity_hash.clear();
        }
        let (row, _) = find_or_insert_row(
            transaction,
            &mut self.interned_values_by_identity_hash,
            "interned_attribute_values",
            identity_hash,
            day,
            |transaction| {
                Ok(transaction
                    .prepare_cached(
                        "INSERT INTO interned_attribute_values
                           (identity_hash, value, newest_record_day)
                         VALUES (?1, ?2, ?3)
                         ON CONFLICT (identity_hash) DO NOTHING",
                    )?
                    .execute(params![identity_hash, value_json, day])?)
            },
        )?;
        Ok(row)
    }

    fn find_or_insert_stable_set(
        &mut self,
        transaction: &Transaction,
        group_id: RecordGroupId,
        attributes_json: &str,
        day: Day,
    ) -> anyhow::Result<StableAttributeSetId> {
        if self.stable_sets_by_identity_hash.len() >= MAX_CACHED_ROWS {
            self.stable_sets_by_identity_hash.clear();
        }
        let identity_hash = hash_fields(&[&group_id.0.to_string(), attributes_json]);
        let (set_id, inserted) = find_or_insert_row(
            transaction,
            &mut self.stable_sets_by_identity_hash,
            "stable_attribute_sets",
            identity_hash,
            day,
            |transaction| {
                Ok(transaction
                    .prepare_cached(
                        "INSERT INTO stable_attribute_sets
                           (identity_hash, record_group_id, attributes, newest_record_day)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT (identity_hash) DO NOTHING",
                    )?
                    .execute(params![identity_hash, group_id, attributes_json, day])?)
            },
        )?;
        if inserted && let Some(group) = self.profiler.find_group(group_id) {
            group.note_new_stable_set();
        }
        Ok(set_id)
    }
}

// The table has id, identity_hash, and newest_record_day, and the cache keeps the newest day
// written, so a row is updated once a day at most.
fn find_or_insert_row<Id: Copy + rusqlite::types::FromSql + rusqlite::ToSql>(
    transaction: &Transaction,
    cache: &mut HashMap<i64, CachedRow<Id>>,
    table: &str,
    identity_hash: i64,
    day: Day,
    insert_row: impl FnOnce(&Transaction) -> anyhow::Result<usize>,
) -> anyhow::Result<(Id, bool)> {
    if let Some(cached) = cache.get_mut(&identity_hash) {
        if cached.newest_record_day < day {
            transaction
                .prepare_cached(&format!(
                    "UPDATE {table} SET newest_record_day = ?2
                     WHERE id = ?1 AND newest_record_day < ?2"
                ))?
                .execute(params![cached.id, day])?;
            cached.newest_record_day = day;
        }
        return Ok((cached.id, false));
    }
    let inserted = insert_row(transaction)? > 0;
    let id: Id = transaction
        .prepare_cached(&format!("SELECT id FROM {table} WHERE identity_hash = ?1"))?
        .query_row([identity_hash], |row| row.get(0))?;
    if !inserted {
        transaction
            .prepare_cached(&format!(
                "UPDATE {table} SET newest_record_day = ?2
                 WHERE id = ?1 AND newest_record_day < ?2"
            ))?
            .execute(params![id, day])?;
    }
    cache.insert(
        identity_hash,
        CachedRow {
            id,
            newest_record_day: day,
        },
    );
    Ok((id, inserted))
}

pub fn find_or_insert_key_id(
    connection: &Connection,
    key_ids: &mut HashMap<String, AttributeKeyId>,
    key: &str,
) -> anyhow::Result<AttributeKeyId> {
    if let Some(&key_id) = key_ids.get(key) {
        return Ok(key_id);
    }
    connection
        .prepare_cached(
            "INSERT INTO attribute_keys (key) VALUES (?1) ON CONFLICT (key) DO NOTHING",
        )?
        .execute([key])?;
    let key_id = connection
        .prepare_cached("SELECT id FROM attribute_keys WHERE key = ?1")?
        .query_row([key], |row| row.get(0))?;
    key_ids.insert(key.to_owned(), key_id);
    Ok(key_id)
}

fn read_key_encodings(
    connection: &Connection,
    group_id: RecordGroupId,
) -> anyhow::Result<HashMap<AttributeKeyId, AttributeEncoding>> {
    let mut statement = connection.prepare_cached(
        "SELECT attribute_key_id, encoding
         FROM attribute_key_profiles
         WHERE record_group_id = ?1",
    )?;
    let mut rows = statement.query([group_id])?;
    let mut encodings = HashMap::new();
    while let Some(row) = rows.next()? {
        if let Some(encoding) = AttributeEncoding::parse(&row.get::<_, String>(1)?) {
            encodings.insert(row.get(0)?, encoding);
        }
    }
    Ok(encodings)
}

fn write_key_profiles(
    transaction: &Transaction,
    group_id: RecordGroupId,
    key_profiles: &[(AttributeKeyId, KeyProfile)],
) -> anyhow::Result<()> {
    let mut upsert = transaction.prepare_cached(
        "INSERT INTO attribute_key_profiles
           (record_group_id, attribute_key_id, encoding, record_count, distinct_value_count,
            classified_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (record_group_id, attribute_key_id) DO UPDATE SET
           encoding = excluded.encoding,
           record_count = excluded.record_count,
           distinct_value_count = excluded.distinct_value_count,
           classified_at = excluded.classified_at",
    )?;
    for (key_id, profile) in key_profiles {
        upsert.execute(params![
            group_id,
            key_id,
            profile.encoding.name(),
            i64::try_from(profile.record_count)?,
            i64::try_from(profile.distinct_value_count)?,
            profile.classified_at,
        ])?;
    }
    Ok(())
}
