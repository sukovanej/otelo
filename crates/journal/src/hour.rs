const NANOS_PER_HOUR: i64 = 3_600 * 1_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hour {
    hours_since_epoch: i64,
}

impl Hour {
    #[must_use]
    pub const fn containing(unix_nanos: i64) -> Self {
        Self {
            hours_since_epoch: unix_nanos.div_euclid(NANOS_PER_HOUR),
        }
    }

    #[must_use]
    pub const fn from_hours_since_epoch(hours_since_epoch: i64) -> Self {
        Self { hours_since_epoch }
    }

    #[must_use]
    pub const fn hours_since_epoch(self) -> i64 {
        self.hours_since_epoch
    }

    #[must_use]
    pub const fn started_at(self) -> i64 {
        self.hours_since_epoch * NANOS_PER_HOUR
    }

    #[must_use]
    pub const fn ended_at(self) -> i64 {
        self.next().started_at()
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self {
            hours_since_epoch: self.hours_since_epoch + 1,
        }
    }
}
