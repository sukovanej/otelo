use std::collections::HashMap;

use otelo_storage::{MetricKind, Temporality};
use rusqlite::{OptionalExtension, Transaction, params};
use twox_hash::XxHash3_64;

// One label that holds a user ID would otherwise make a series per user.
pub const MAX_SERIES_PER_METRIC: i64 = 1000;

#[derive(Clone, Copy)]
pub enum StoredResource {
    Found(i64),
    Inserted(i64),
}

impl StoredResource {
    pub const fn id(self) -> i64 {
        match self {
            Self::Found(id) | Self::Inserted(id) => id,
        }
    }
}

#[derive(Clone, Copy)]
pub enum StoredSeries {
    Found(i64),
    Inserted(i64),
    PastTheMostOfItsMetric,
}

#[derive(Default)]
pub struct SeriesCache {
    ids_by_hash: HashMap<i64, i64>,
    counts_by_metric: HashMap<String, i64>,
}

pub struct SeriesIdentity<'a> {
    pub name: &'a str,
    pub kind: MetricKind,
    pub unit: &'a str,
    pub labels_json: &'a str,
}

// The JSON has sorted keys, so equal attributes hash equal.
pub fn find_or_insert_resource_id(
    tx: &Transaction,
    cache: &mut HashMap<i64, i64>,
    service: &str,
    attributes_json: &str,
) -> anyhow::Result<StoredResource> {
    let hash = hash_fields(&[service, attributes_json]);
    if let Some(&id) = cache.get(&hash) {
        return Ok(StoredResource::Found(id));
    }
    let inserted = tx
        .prepare_cached(
            "INSERT INTO resources (hash, service, attributes) VALUES (?1, ?2, ?3)
             ON CONFLICT (hash) DO NOTHING",
        )?
        .execute(params![hash, service, attributes_json])?;
    let id = tx
        .prepare_cached("SELECT id FROM resources WHERE hash = ?1")?
        .query_row([hash], |row| row.get(0))?;
    cache.insert(hash, id);
    Ok(if inserted > 0 {
        StoredResource::Inserted(id)
    } else {
        StoredResource::Found(id)
    })
}

pub fn find_or_insert_series_id(
    tx: &Transaction,
    cache: &mut SeriesCache,
    resource_id: i64,
    series: &SeriesIdentity,
) -> anyhow::Result<StoredSeries> {
    let temporality = series.kind.temporality().map(Temporality::name);
    let hash = hash_fields(&[
        &resource_id.to_string(),
        series.name,
        series.kind.name(),
        temporality.unwrap_or_default(),
        series.unit,
        series.labels_json,
    ]);
    if let Some(&id) = cache.ids_by_hash.get(&hash) {
        return Ok(StoredSeries::Found(id));
    }
    let stored = tx
        .prepare_cached("SELECT id FROM series WHERE hash = ?1")?
        .query_row([hash], |row| row.get(0))
        .optional()?;
    if let Some(id) = stored {
        cache.ids_by_hash.insert(hash, id);
        return Ok(StoredSeries::Found(id));
    }
    let count = if let Some(&count) = cache.counts_by_metric.get(series.name) {
        count
    } else {
        let stored: i64 = tx
            .prepare_cached("SELECT count(*) FROM series WHERE name = ?1")?
            .query_row([series.name], |row| row.get(0))?;
        cache
            .counts_by_metric
            .insert(series.name.to_owned(), stored);
        stored
    };
    if count >= MAX_SERIES_PER_METRIC {
        return Ok(StoredSeries::PastTheMostOfItsMetric);
    }
    tx.prepare_cached(
        "INSERT INTO series (hash, resource_id, name, kind, temporality, unit, labels)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(params![
        hash,
        resource_id,
        series.name,
        series.kind.name(),
        temporality,
        series.unit,
        series.labels_json
    ])?;
    if let Some(count) = cache.counts_by_metric.get_mut(series.name) {
        *count += 1;
    }
    let id = tx.last_insert_rowid();
    cache.ids_by_hash.insert(hash, id);
    Ok(StoredSeries::Inserted(id))
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
