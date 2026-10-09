use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, bail};
use rusqlite::{OptionalExtension, Transaction, params};

use otelo_indexed_storage::AttributeValue;

use crate::attribute_encoding::{AttributeKeyId, RecordGroupId};
use crate::rollup::{HOUR_NS, MINUTE_NS};
use crate::series::hash_fields;

// Buckets that grow by 2^(1/64) keep a percentile, the middle of its bucket, within 0.6% of the
// duration, in memory that grows with the logarithm of the durations. It is scale 6 of an
// OpenTelemetry exponential histogram.
const BUCKETS_PER_DOUBLING: f64 = 64.0;
const MAX_CACHED_KEYS: usize = 20_000;

pub const SPAN_SUMMARY_TABLES: [&str; 2] = ["span_minute_summaries", "span_hour_summaries"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpanSummaryKeyId(pub i64);

impl rusqlite::ToSql for SpanSummaryKeyId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl rusqlite::types::FromSql for SpanSummaryKeyId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DurationHistogram {
    zero_count: u64,
    counts_by_index: BTreeMap<i32, u64>,
}

impl DurationHistogram {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the index of a bucket is below 4100 for any i64"
    )]
    pub fn add_duration(&mut self, duration_ns: i64) {
        if duration_ns <= 0 {
            self.zero_count += 1;
        } else {
            let index = ((duration_ns as f64).log2() * BUCKETS_PER_DOUBLING).ceil() as i32 - 1;
            *self.counts_by_index.entry(index).or_default() += 1;
        }
    }

    pub fn add_histogram(&mut self, more: &Self) {
        self.zero_count += more.zero_count;
        for (&index, &count) in &more.counts_by_index {
            *self.counts_by_index.entry(index).or_default() += count;
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + 3 * self.counts_by_index.len());
        push_varint(&mut bytes, self.zero_count);
        let mut previous_index = 0_i32;
        for (&index, &count) in &self.counts_by_index {
            push_varint(&mut bytes, zigzag(index - previous_index));
            push_varint(&mut bytes, count);
            previous_index = index;
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut position = 0;
        let zero_count = read_varint(bytes, &mut position)?;
        let mut counts_by_index = BTreeMap::new();
        let mut index = 0_i32;
        while position < bytes.len() {
            index += unzigzag(read_varint(bytes, &mut position)?)?;
            counts_by_index.insert(index, read_varint(bytes, &mut position)?);
        }
        Ok(Self {
            zero_count,
            counts_by_index,
        })
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an estimate"
    )]
    pub fn nearest_rank_quantile(&self, quantile: f64, count: u64) -> i64 {
        let rank = ((quantile * count as f64).ceil() as u64).max(1) - 1;
        let mut seen = self.zero_count;
        if rank < seen {
            return 0;
        }
        let base = (1.0 / BUCKETS_PER_DOUBLING).exp2();
        for (&index, &bucket_count) in &self.counts_by_index {
            seen += bucket_count;
            if rank < seen {
                return (2.0 * base.powi(index + 1) / (base + 1.0)).round() as i64;
            }
        }
        0
    }
}

fn zigzag(value: i32) -> u64 {
    u64::from(((value << 1) ^ (value >> 31)).cast_unsigned())
}

fn unzigzag(value: u64) -> anyhow::Result<i32> {
    let value = u32::try_from(value).context("a histogram index past 32 bits")?;
    Ok((value >> 1).cast_signed() ^ -((value & 1).cast_signed()))
}

fn push_varint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        bytes.push(u8::try_from(value & 0x7f).expect("seven bits") | 0x80);
        value >>= 7;
    }
    bytes.push(u8::try_from(value).expect("below 0x80"));
}

fn read_varint(bytes: &[u8], position: &mut usize) -> anyhow::Result<u64> {
    let mut value = 0_u64;
    for shift in (0..64).step_by(7) {
        let Some(&byte) = bytes.get(*position) else {
            bail!("a histogram ends inside a varint");
        };
        *position += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return Ok(value);
        }
    }
    bail!("a varint past 64 bits")
}

#[derive(Clone, Debug)]
pub struct SpanSummary {
    pub span_count: u64,
    pub duration_sum_ns: i64,
    pub duration_min_ns: i64,
    pub duration_max_ns: i64,
    pub duration_histogram: DurationHistogram,
}

impl SpanSummary {
    pub fn of_span(duration_ns: i64) -> Self {
        let mut duration_histogram = DurationHistogram::default();
        duration_histogram.add_duration(duration_ns);
        Self {
            span_count: 1,
            duration_sum_ns: duration_ns,
            duration_min_ns: duration_ns,
            duration_max_ns: duration_ns,
            duration_histogram,
        }
    }

    pub fn add_summary(&mut self, more: &Self) {
        self.span_count += more.span_count;
        self.duration_sum_ns = self.duration_sum_ns.saturating_add(more.duration_sum_ns);
        self.duration_min_ns = self.duration_min_ns.min(more.duration_min_ns);
        self.duration_max_ns = self.duration_max_ns.max(more.duration_max_ns);
        self.duration_histogram
            .add_histogram(&more.duration_histogram);
    }

    pub fn read_from_row(row: &rusqlite::Row, first_column: usize) -> anyhow::Result<Self> {
        Ok(Self {
            span_count: u64::try_from(row.get::<_, i64>(first_column)?)?,
            duration_sum_ns: row.get(first_column + 1)?,
            duration_min_ns: row.get(first_column + 2)?,
            duration_max_ns: row.get(first_column + 3)?,
            duration_histogram: DurationHistogram::decode(
                &row.get::<_, Vec<u8>>(first_column + 4)?,
            )?,
        })
    }
}

pub struct SpanSummaryKey<'a> {
    pub record_group_id: RecordGroupId,
    pub kind: i32,
    pub status_code: i32,
    pub summarized_attributes: &'a BTreeMap<AttributeKeyId, &'a AttributeValue>,
    pub unsummarized_attribute_key_ids: &'a [AttributeKeyId],
}

#[derive(Default)]
pub struct SpanSummaryWriter {
    key_ids_by_identity_hash: HashMap<i64, SpanSummaryKeyId>,
    pending_minutes: HashMap<(SpanSummaryKeyId, i64), SpanSummary>,
}

impl SpanSummaryWriter {
    pub fn find_or_insert_key(
        &mut self,
        transaction: &Transaction,
        key: &SpanSummaryKey,
    ) -> anyhow::Result<SpanSummaryKeyId> {
        let summarized_attributes = serde_json::to_string(key.summarized_attributes)?;
        let mut unsummarized_attribute_key_ids = key.unsummarized_attribute_key_ids.to_vec();
        unsummarized_attribute_key_ids.sort_unstable();
        let unsummarized_attribute_key_ids =
            serde_json::to_string(&unsummarized_attribute_key_ids)?;
        let identity_hash = hash_fields(&[
            &key.record_group_id.0.to_string(),
            &key.kind.to_string(),
            &key.status_code.to_string(),
            &summarized_attributes,
            &unsummarized_attribute_key_ids,
        ]);
        if let Some(&key_id) = self.key_ids_by_identity_hash.get(&identity_hash) {
            return Ok(key_id);
        }
        if self.key_ids_by_identity_hash.len() >= MAX_CACHED_KEYS {
            self.key_ids_by_identity_hash.clear();
        }
        transaction
            .prepare_cached(
                "INSERT INTO span_summary_keys
                   (identity_hash, record_group_id, kind, status_code,
                    summarized_attributes, unsummarized_attribute_key_ids)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (identity_hash) DO NOTHING",
            )?
            .execute(params![
                identity_hash,
                key.record_group_id,
                key.kind,
                key.status_code,
                summarized_attributes,
                unsummarized_attribute_key_ids,
            ])?;
        let key_id = transaction
            .prepare_cached("SELECT id FROM span_summary_keys WHERE identity_hash = ?1")?
            .query_row([identity_hash], |row| row.get(0))?;
        self.key_ids_by_identity_hash.insert(identity_hash, key_id);
        Ok(key_id)
    }

    pub fn add_span(&mut self, key_id: SpanSummaryKeyId, started_at: i64, duration_ns: i64) {
        let minute_start_at = started_at.div_euclid(MINUTE_NS) * MINUTE_NS;
        let span = SpanSummary::of_span(duration_ns);
        self.pending_minutes
            .entry((key_id, minute_start_at))
            .and_modify(|summary| summary.add_summary(&span))
            .or_insert(span);
    }

    pub fn write_pending(&mut self, transaction: &Transaction) -> anyhow::Result<()> {
        let pending_minutes = std::mem::take(&mut self.pending_minutes);
        let mut pending_hours: HashMap<(SpanSummaryKeyId, i64), SpanSummary> = HashMap::new();
        for (&(key_id, minute_start_at), summary) in &pending_minutes {
            let hour_start_at = minute_start_at.div_euclid(HOUR_NS) * HOUR_NS;
            pending_hours
                .entry((key_id, hour_start_at))
                .and_modify(|hour| hour.add_summary(summary))
                .or_insert_with(|| summary.clone());
        }
        let [minute_table, hour_table] = SPAN_SUMMARY_TABLES;
        add_to_summaries(transaction, minute_table, pending_minutes)?;
        add_to_summaries(transaction, hour_table, pending_hours)
    }
}

fn add_to_summaries(
    transaction: &Transaction,
    table: &str,
    summaries: HashMap<(SpanSummaryKeyId, i64), SpanSummary>,
) -> anyhow::Result<()> {
    let mut select = transaction.prepare_cached(&format!(
        "SELECT span_count, duration_sum_ns, duration_min_ns, duration_max_ns, duration_histogram
         FROM {table}
         WHERE span_summary_key_id = ?1 AND start_at = ?2"
    ))?;
    let mut upsert = transaction.prepare_cached(&format!(
        "INSERT OR REPLACE INTO {table}
           (span_summary_key_id, start_at, span_count, duration_sum_ns, duration_min_ns,
            duration_max_ns, duration_histogram)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    ))?;
    for ((key_id, start_at), mut summary) in summaries {
        let stored = select
            .query_row(params![key_id, start_at], |row| {
                Ok(SpanSummary::read_from_row(row, 0))
            })
            .optional()?
            .transpose()?;
        if let Some(stored) = stored {
            summary.add_summary(&stored);
        }
        upsert.execute(params![
            key_id,
            start_at,
            i64::try_from(summary.span_count)?,
            summary.duration_sum_ns,
            summary.duration_min_ns,
            summary.duration_max_ns,
            summary.duration_histogram.encode(),
        ])?;
    }
    Ok(())
}
