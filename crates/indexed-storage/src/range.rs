use anyhow::ensure;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeRange {
    start_at: i64,
    end_at: i64,
}

impl TimeRange {
    pub fn new(start_at: i64, end_at: i64) -> anyhow::Result<Self> {
        ensure!(start_at < end_at, "the start has to be before the end");
        Ok(Self { start_at, end_at })
    }

    #[must_use]
    pub const fn start_at(self) -> i64 {
        self.start_at
    }

    #[must_use]
    pub const fn end_at(self) -> i64 {
        self.end_at
    }

    #[must_use]
    pub const fn length_ns(self) -> i64 {
        self.end_at - self.start_at
    }
}
