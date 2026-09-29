use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use siner_storage::{Inbox, IndexedAttribute, RangeQueries, Result, Storage, TimeRange};

use crate::day::Day;
use crate::indexes::Indexes;
use crate::state::StateFile;
use crate::{Config, Reader, Writer};

pub struct Sqlite {
    config: Config,
    state: StateFile,
}

impl Sqlite {
    pub fn open(data: &Path) -> anyhow::Result<Self> {
        let state = StateFile::open(data)?;
        let mut config = Config::new(data.join("telemetry"));
        config.indexes = Indexes::new(state.indexed_attributes()?);
        Ok(Self { config, state })
    }

    pub fn spawn_writer(&self, inbox: Inbox) -> anyhow::Result<Writer> {
        Writer::spawn(self.config.clone(), inbox)
    }
}

impl Storage for Sqlite {
    fn oldest_retained_at(&self) -> i64 {
        self.config.oldest_retained_day(Day::today()).start()
    }

    fn open_range(&self, range: TimeRange, time_limit: Duration) -> Result<Box<dyn RangeQueries>> {
        let mut reader = Reader::open(&self.config.dir, range)?;
        reader.set_time_limit(time_limit)?;
        reader.set_indexed_attributes(self.config.indexes.attributes());
        Ok(Box::new(reader))
    }

    fn indexed_attributes(&self) -> BTreeSet<IndexedAttribute> {
        self.config.indexes.attributes()
    }

    fn add_index(&self, attribute: &IndexedAttribute) -> Result<()> {
        self.state.add_indexed_attribute(attribute)?;
        self.config
            .indexes
            .replace_attributes(self.state.indexed_attributes()?);
        Ok(())
    }

    fn remove_index(&self, attribute: &IndexedAttribute) -> Result<bool> {
        let removed = self.state.remove_indexed_attribute(attribute)?;
        self.config
            .indexes
            .replace_attributes(self.state.indexed_attributes()?);
        Ok(removed)
    }
}
