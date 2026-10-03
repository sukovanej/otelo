use std::collections::HashMap;

use otelo_indexed_storage::{MetricKind, Temporality};
use rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};
use rusqlite::{OptionalExtension, ToSql, Transaction, params};
use twox_hash::XxHash3_64;

// One attribute that holds a user ID would otherwise make a series per user.
pub const MAX_SERIES_PER_METRIC: i64 = 1000;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(i64);

impl ToSql for ResourceId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for ResourceId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MetricSeriesId(i64);

impl ToSql for MetricSeriesId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for MetricSeriesId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

#[derive(Clone, Copy)]
pub enum StoredResource {
    Found(ResourceId),
    Inserted(ResourceId),
}

impl StoredResource {
    pub const fn id(self) -> ResourceId {
        match self {
            Self::Found(resource_id) | Self::Inserted(resource_id) => resource_id,
        }
    }
}

#[derive(Clone, Copy)]
pub enum StoredSeries {
    Found(MetricSeriesId),
    Inserted(MetricSeriesId),
    PastSeriesLimit,
}

#[derive(Default)]
pub struct SeriesCache {
    series_ids_by_identity_hash: HashMap<i64, MetricSeriesId>,
    series_counts_by_metric: HashMap<String, i64>,
}

pub struct SeriesIdentity<'a> {
    pub name: &'a str,
    pub kind: MetricKind,
    pub unit: &'a str,
    pub attributes_json: &'a str,
}

// The JSON has sorted keys, so equal attributes hash equal.
pub fn find_or_insert_resource_id(
    transaction: &Transaction,
    resource_ids_by_identity_hash: &mut HashMap<i64, ResourceId>,
    service: &str,
    attributes_json: &str,
) -> anyhow::Result<StoredResource> {
    let identity_hash = hash_fields(&[service, attributes_json]);
    if let Some(&resource_id) = resource_ids_by_identity_hash.get(&identity_hash) {
        return Ok(StoredResource::Found(resource_id));
    }
    let inserted_row_count = transaction
        .prepare_cached(
            "INSERT INTO resources (identity_hash, service, attributes) VALUES (?1, ?2, ?3)
             ON CONFLICT (identity_hash) DO NOTHING",
        )?
        .execute(params![identity_hash, service, attributes_json])?;
    let resource_id = transaction
        .prepare_cached("SELECT id FROM resources WHERE identity_hash = ?1")?
        .query_row([identity_hash], |row| row.get(0))?;
    resource_ids_by_identity_hash.insert(identity_hash, resource_id);
    Ok(if inserted_row_count > 0 {
        StoredResource::Inserted(resource_id)
    } else {
        StoredResource::Found(resource_id)
    })
}

pub fn find_or_insert_series_id(
    transaction: &Transaction,
    series_cache: &mut SeriesCache,
    resource_id: ResourceId,
    series_identity: &SeriesIdentity,
) -> anyhow::Result<StoredSeries> {
    let temporality_name = series_identity.kind.temporality().map(Temporality::name);
    let identity_hash = hash_fields(&[
        &resource_id.0.to_string(),
        series_identity.name,
        series_identity.kind.name(),
        temporality_name.unwrap_or_default(),
        series_identity.unit,
        series_identity.attributes_json,
    ]);
    if let Some(&series_id) = series_cache.series_ids_by_identity_hash.get(&identity_hash) {
        return Ok(StoredSeries::Found(series_id));
    }
    let stored_series_id = transaction
        .prepare_cached("SELECT id FROM metric_series WHERE identity_hash = ?1")?
        .query_row([identity_hash], |row| row.get(0))
        .optional()?;
    if let Some(series_id) = stored_series_id {
        series_cache
            .series_ids_by_identity_hash
            .insert(identity_hash, series_id);
        return Ok(StoredSeries::Found(series_id));
    }
    let series_count = if let Some(&series_count) = series_cache
        .series_counts_by_metric
        .get(series_identity.name)
    {
        series_count
    } else {
        let stored_series_count: i64 = transaction
            .prepare_cached("SELECT count(*) FROM metric_series WHERE name = ?1")?
            .query_row([series_identity.name], |row| row.get(0))?;
        series_cache
            .series_counts_by_metric
            .insert(series_identity.name.to_owned(), stored_series_count);
        stored_series_count
    };
    if series_count >= MAX_SERIES_PER_METRIC {
        return Ok(StoredSeries::PastSeriesLimit);
    }
    transaction
        .prepare_cached(
            "INSERT INTO metric_series (identity_hash, resource_id, name, kind,
                                        aggregation_temporality, unit, attributes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?
        .execute(params![
            identity_hash,
            resource_id,
            series_identity.name,
            series_identity.kind.name(),
            temporality_name,
            series_identity.unit,
            series_identity.attributes_json
        ])?;
    if let Some(series_count) = series_cache
        .series_counts_by_metric
        .get_mut(series_identity.name)
    {
        *series_count += 1;
    }
    let series_id = MetricSeriesId(transaction.last_insert_rowid());
    series_cache
        .series_ids_by_identity_hash
        .insert(identity_hash, series_id);
    Ok(StoredSeries::Inserted(series_id))
}

pub fn hash_fields(fields: &[&str]) -> i64 {
    // A length before each field keeps two lists of fields from hashing the same bytes.
    let mut bytes = Vec::new();
    for field in fields {
        bytes.extend_from_slice(&(field.len() as u64).to_le_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    XxHash3_64::oneshot(&bytes).cast_signed()
}
