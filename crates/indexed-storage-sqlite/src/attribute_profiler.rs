use std::collections::HashMap;

use crate::attribute_encoding::{AttributeEncoding, AttributeKeyId, RecordGroupId};
use crate::distinct_values::{DistinctValueCounter, MAX_EXACTLY_COUNTED_VALUES};

const FIRST_CLASSIFICATION_AT_RECORD: u64 = 32;
const MAX_RECORDS_BETWEEN_CLASSIFICATIONS: u64 = 4_096;
const MAX_NANOS_BETWEEN_CLASSIFICATIONS: i64 = 3_600 * 1_000_000_000;
const MAX_DISTINCT_VALUES_OF_NEW_STABLE_KEY: u64 = MAX_EXACTLY_COUNTED_VALUES as u64;
// Between the two limits a stable key stays stable, so a key near one does not flip back and forth.
const MAX_DISTINCT_VALUES_OF_STABLE_KEY: u64 = 2 * MAX_DISTINCT_VALUES_OF_NEW_STABLE_KEY;
const MIN_RECORDS_PER_VALUE_OF_NEW_STABLE_KEY: u64 = 20;
const MIN_RECORDS_PER_INTERNED_VALUE: u64 = 2;
// The product of the value counts of the stable keys of a group, which bounds its stable sets.
pub const MAX_STABLE_SETS_PER_CLASSIFICATION: u64 = 4_096;
// Past this many groups the profiler forgets the windows, and loads the encodings again.
const MAX_PROFILED_GROUPS: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyProfile {
    pub encoding: AttributeEncoding,
    pub record_count: u64,
    pub distinct_value_count: u64,
    pub classified_at: i64,
}

#[derive(Default)]
struct KeyWindow {
    record_count: u64,
    distinct_values: DistinctValueCounter,
}

impl KeyWindow {
    fn count_distinct_values(&self) -> u64 {
        self.distinct_values.count_distinct_values()
    }
}

#[derive(Default)]
struct GroupWindow {
    first_received_at: Option<i64>,
    new_stable_set_count: u64,
    keys: HashMap<AttributeKeyId, KeyWindow>,
}

pub struct GroupProfile {
    encodings: HashMap<AttributeKeyId, AttributeEncoding>,
    observed_record_count: u64,
    next_classification_at_record: u64,
    window: GroupWindow,
}

impl GroupProfile {
    pub fn new(encodings: HashMap<AttributeKeyId, AttributeEncoding>) -> Self {
        Self {
            encodings,
            observed_record_count: 0,
            next_classification_at_record: FIRST_CLASSIFICATION_AT_RECORD,
            window: GroupWindow::default(),
        }
    }

    // A key without a profile goes into the stable set, so a constant key never needs another
    // encoding, and a varying one is classified once it passes the values of a stable key.
    pub fn encoding_of_key(&self, key_id: AttributeKeyId) -> AttributeEncoding {
        self.encodings
            .get(&key_id)
            .copied()
            .unwrap_or(AttributeEncoding::Stable)
    }

    pub const fn note_new_stable_set(&mut self) {
        self.window.new_stable_set_count += 1;
    }

    pub fn observe_record(
        &mut self,
        received_at: i64,
        value_hashes_by_key: &[(AttributeKeyId, u64)],
    ) -> Option<Vec<(AttributeKeyId, KeyProfile)>> {
        self.observed_record_count += 1;
        let first_received_at = *self.window.first_received_at.get_or_insert(received_at);
        let mut stable_key_varies = false;
        for &(key_id, value_hash) in value_hashes_by_key {
            let is_stable = self.encoding_of_key(key_id) == AttributeEncoding::Stable;
            let key_window = self.window.keys.entry(key_id).or_default();
            key_window.record_count += 1;
            key_window.distinct_values.add_value_hash(value_hash);
            if is_stable && key_window.count_distinct_values() > MAX_DISTINCT_VALUES_OF_STABLE_KEY {
                stable_key_varies = true;
            }
        }
        let is_due = self.observed_record_count >= self.next_classification_at_record
            || stable_key_varies
            || self.window.new_stable_set_count > MAX_STABLE_SETS_PER_CLASSIFICATION
            || received_at - first_received_at >= MAX_NANOS_BETWEEN_CLASSIFICATIONS;
        is_due.then(|| self.classify_keys(received_at))
    }

    fn classify_keys(&mut self, classified_at: i64) -> Vec<(AttributeKeyId, KeyProfile)> {
        let window = std::mem::take(&mut self.window);
        self.next_classification_at_record =
            if self.observed_record_count < MAX_RECORDS_BETWEEN_CLASSIFICATIONS {
                self.observed_record_count * 2
            } else {
                self.observed_record_count + MAX_RECORDS_BETWEEN_CLASSIFICATIONS
            };
        let counted_keys: Vec<(AttributeKeyId, u64, u64)> = window
            .keys
            .iter()
            .map(|(&key_id, key_window)| {
                (
                    key_id,
                    key_window.record_count,
                    key_window.count_distinct_values(),
                )
            })
            .collect();
        let mut stable_candidates: Vec<(bool, u64, AttributeKeyId)> = counted_keys
            .iter()
            .filter(|&&(key_id, record_count, distinct_value_count)| {
                if self.encodings.get(&key_id) == Some(&AttributeEncoding::Stable) {
                    distinct_value_count <= MAX_DISTINCT_VALUES_OF_STABLE_KEY
                } else {
                    distinct_value_count <= MAX_DISTINCT_VALUES_OF_NEW_STABLE_KEY
                        && record_count
                            >= MIN_RECORDS_PER_VALUE_OF_NEW_STABLE_KEY * distinct_value_count
                }
            })
            .map(|&(key_id, _, distinct_value_count)| {
                let is_new = self.encodings.get(&key_id) != Some(&AttributeEncoding::Stable);
                (is_new, distinct_value_count, key_id)
            })
            .collect();
        stable_candidates.sort_unstable();
        let mut stable_set_bound = 1_u64;
        let mut stable_keys = Vec::new();
        for (_, distinct_value_count, key_id) in stable_candidates {
            let bound = stable_set_bound.saturating_mul(distinct_value_count);
            if bound <= MAX_STABLE_SETS_PER_CLASSIFICATION {
                stable_set_bound = bound;
                stable_keys.push(key_id);
            }
        }
        let mut profiles: Vec<(AttributeKeyId, KeyProfile)> = counted_keys
            .into_iter()
            .map(|(key_id, record_count, distinct_value_count)| {
                let encoding = if stable_keys.contains(&key_id) {
                    AttributeEncoding::Stable
                } else if distinct_value_count * MIN_RECORDS_PER_INTERNED_VALUE <= record_count {
                    AttributeEncoding::Interned
                } else {
                    AttributeEncoding::Literal
                };
                self.encodings.insert(key_id, encoding);
                (
                    key_id,
                    KeyProfile {
                        encoding,
                        record_count,
                        distinct_value_count,
                        classified_at,
                    },
                )
            })
            .collect();
        profiles.sort_unstable_by_key(|&(key_id, _)| key_id);
        profiles
    }
}

#[derive(Default)]
pub struct AttributeProfiler {
    groups: HashMap<RecordGroupId, GroupProfile>,
}

impl AttributeProfiler {
    pub fn find_group(&mut self, group_id: RecordGroupId) -> Option<&mut GroupProfile> {
        self.groups.get_mut(&group_id)
    }

    pub fn insert_group(&mut self, group_id: RecordGroupId, group: GroupProfile) {
        if self.groups.len() >= MAX_PROFILED_GROUPS {
            self.groups.clear();
        }
        self.groups.insert(group_id, group);
    }

    pub fn forget_group(&mut self, group_id: RecordGroupId) {
        self.groups.remove(&group_id);
    }
}
