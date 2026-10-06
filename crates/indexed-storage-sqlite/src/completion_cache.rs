use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use otelo_indexed_storage::TimeRange;
use otelo_query::{BuiltinField, Field, FieldValues, KeyInfo, Signal};

use crate::catalog::AttributeOwner;
use crate::query::RecordSample;

// The context stays the same while the user types one term, so its sample serves every keystroke.
const MAX_AGE_OF_CACHED_CONTEXT: Duration = Duration::from_secs(30);

const MAX_CACHED_CONTEXTS: usize = 16;

#[derive(Default)]
pub struct CompletionCache {
    contexts: Mutex<Vec<CachedContext>>,
}

#[derive(Clone)]
pub struct ContextOfRange {
    pub signal: Signal,
    pub context_text: String,
    pub range: TimeRange,
}

impl ContextOfRange {
    // A range that ends now moves with each request, and a cached sample of it stays good a while.
    fn is_close_to(&self, other: &Self) -> bool {
        let is_within_max_age =
            |shift_ns: u64| u128::from(shift_ns) <= MAX_AGE_OF_CACHED_CONTEXT.as_nanos();
        self.signal == other.signal
            && self.context_text == other.context_text
            && is_within_max_age(self.range.start_at().abs_diff(other.range.start_at()))
            && is_within_max_age(self.range.end_at().abs_diff(other.range.end_at()))
    }
}

struct CachedContext {
    context_of_range: ContextOfRange,
    cached_at: Instant,
    sample: RecordSample,
    keys_by_owner: HashMap<AttributeOwner, Vec<KeyInfo>>,
    values_by_field_and_prefix: HashMap<(Field, String), FieldValues>,
    presence_by_builtin_field: HashMap<BuiltinField, bool>,
}

impl CompletionCache {
    fn read_cached_context<T>(
        &self,
        context_of_range: &ContextOfRange,
        read: impl FnOnce(&CachedContext) -> Option<T>,
    ) -> Option<T> {
        let contexts = self.contexts.lock().ok()?;
        contexts
            .iter()
            .find(|cached| {
                cached.cached_at.elapsed() < MAX_AGE_OF_CACHED_CONTEXT
                    && cached.context_of_range.is_close_to(context_of_range)
            })
            .and_then(read)
    }

    fn change_cached_context(
        &self,
        context_of_range: &ContextOfRange,
        change: impl FnOnce(&mut CachedContext),
    ) {
        let Ok(mut contexts) = self.contexts.lock() else {
            return;
        };
        if let Some(cached) = contexts
            .iter_mut()
            .find(|cached| cached.context_of_range.is_close_to(context_of_range))
        {
            change(cached);
        }
    }

    pub fn find_sample(&self, context_of_range: &ContextOfRange) -> Option<RecordSample> {
        self.read_cached_context(context_of_range, |cached| Some(cached.sample.clone()))
    }

    pub fn keep_sample(&self, context_of_range: ContextOfRange, sample: RecordSample) {
        let Ok(mut contexts) = self.contexts.lock() else {
            return;
        };
        contexts.retain(|cached| {
            cached.cached_at.elapsed() < MAX_AGE_OF_CACHED_CONTEXT
                && !cached.context_of_range.is_close_to(&context_of_range)
        });
        if contexts.len() >= MAX_CACHED_CONTEXTS {
            contexts.remove(0);
        }
        contexts.push(CachedContext {
            context_of_range,
            cached_at: Instant::now(),
            sample,
            keys_by_owner: HashMap::new(),
            values_by_field_and_prefix: HashMap::new(),
            presence_by_builtin_field: HashMap::new(),
        });
    }

    pub fn find_keys(
        &self,
        context_of_range: &ContextOfRange,
        owner: AttributeOwner,
    ) -> Option<Vec<KeyInfo>> {
        self.read_cached_context(context_of_range, |cached| {
            cached.keys_by_owner.get(&owner).cloned()
        })
    }

    pub fn keep_keys(
        &self,
        context_of_range: &ContextOfRange,
        owner: AttributeOwner,
        keys: Vec<KeyInfo>,
    ) {
        self.change_cached_context(context_of_range, |cached| {
            cached.keys_by_owner.insert(owner, keys);
        });
    }

    pub fn find_values(
        &self,
        context_of_range: &ContextOfRange,
        field: &Field,
        lowercase_value_prefix: &str,
    ) -> Option<FieldValues> {
        self.read_cached_context(context_of_range, |cached| {
            cached
                .values_by_field_and_prefix
                .get(&(field.clone(), lowercase_value_prefix.to_owned()))
                .cloned()
        })
    }

    pub fn keep_values(
        &self,
        context_of_range: &ContextOfRange,
        field: &Field,
        lowercase_value_prefix: &str,
        values: FieldValues,
    ) {
        self.change_cached_context(context_of_range, |cached| {
            cached
                .values_by_field_and_prefix
                .insert((field.clone(), lowercase_value_prefix.to_owned()), values);
        });
    }

    pub fn find_whether_has_builtin_field(
        &self,
        context_of_range: &ContextOfRange,
        builtin_field: BuiltinField,
    ) -> Option<bool> {
        self.read_cached_context(context_of_range, |cached| {
            cached
                .presence_by_builtin_field
                .get(&builtin_field)
                .copied()
        })
    }

    pub fn keep_whether_has_builtin_field(
        &self,
        context_of_range: &ContextOfRange,
        builtin_field: BuiltinField,
        has_field: bool,
    ) {
        self.change_cached_context(context_of_range, |cached| {
            cached
                .presence_by_builtin_field
                .insert(builtin_field, has_field);
        });
    }
}
