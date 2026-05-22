use alloc::{borrow::Cow, string::String, vec::Vec};
use core::{fmt, time::Duration};

use hashbrown::HashMap;
use libaflmm_bolts::current_time;
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeMap};

/// class for performance analytics
#[derive(Debug, Clone, Default)]
pub struct PerfStats {
    /// Total wall time accumulated across all completed iterations.
    total_elapsed: Duration,
    /// Per-stage timer.
    stages: HashMap<Cow<'static, str>, Duration>,
    /// Wall-clock timer for this run.
    iter_start: Duration,
}

const TOTAL_ELAPSED_KEY: &str = "__total_elapsed_ns__";

/// basically tell `serde_json` how to serialize it, because we have to dump it once in a while into a json
impl Serialize for PerfStats {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.stages.len() + 1))?;
        map.serialize_entry(TOTAL_ELAPSED_KEY, &(self.total_elapsed.as_nanos() as u64))?;
        for (k, v) in &self.stages {
            map.serialize_entry(k.as_ref(), &(v.as_nanos() as u64))?;
        }
        map.end()
    }
}

/// the opposite to above
impl<'de> Deserialize<'de> for PerfStats {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut raw = <HashMap<String, u64>>::deserialize(d)?;
        let total_elapsed = raw
            .remove(TOTAL_ELAPSED_KEY)
            .map(Duration::from_nanos)
            .ok_or_else(|| serde::de::Error::missing_field(TOTAL_ELAPSED_KEY))?;
        let stages = raw
            .into_iter()
            .map(|(k, v)| (Cow::Owned(k), Duration::from_nanos(v)))
            .collect();
        Ok(Self {
            total_elapsed,
            stages,
            iter_start: Duration::ZERO,
        })
    }
}

impl PerfStats {
    /// construct a [`struct@PerfStats`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// start of a fuzz iteration.
    #[inline]
    pub fn iter_begin(&mut self) {
        self.iter_start = current_time();
    }

    /// end of a fuzz iteration and accumulate its elapsed time.
    #[inline]
    pub fn iter_end(&mut self) {
        let elapsed = current_time().saturating_sub(self.iter_start);
        self.total_elapsed = self.total_elapsed.saturating_add(elapsed);
    }

    /// Add `elapsed` to the bucket for the stage named `name`.
    #[inline]
    pub fn record_stage(&mut self, name: Cow<'static, str>, elapsed: Duration) {
        let bucket = self.stages.entry(name).or_insert(Duration::ZERO);
        *bucket = bucket.saturating_add(elapsed);
    }

    /// Total wall time across all iterations.
    #[must_use]
    pub fn total_elapsed(&self) -> Duration {
        self.total_elapsed
    }

    /// Per-stage accumulated durations, keyed by stage name.
    #[must_use]
    pub fn stages(&self) -> &HashMap<Cow<'static, str>, Duration> {
        &self.stages
    }

    /// Time spent outside any stage: `total_elapsed - sum(stages)`.
    #[must_use]
    pub fn others(&self) -> Duration {
        let stage_sum: Duration = self.stages.values().copied().sum();
        self.total_elapsed.saturating_sub(stage_sum)
    }
}

impl fmt::Display for PerfStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let total = self.total_elapsed.as_secs_f64();

        writeln!(f, "  total: {total:8.4}s")?;

        let mut rows: Vec<(&Cow<'static, str>, &Duration)> = self.stages.iter().collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        for (name, time) in rows {
            let secs = time.as_secs_f64();
            let pct = if total > 0.0 { secs / total } else { 0.0 };
            writeln!(f, "  {name}: {secs:8.4}s ({pct:6.2}%)")?;
        }
        let others = self.others().as_secs_f64();
        let pct = if total > 0.0 { others / total } else { 0.0 };
        write!(f, "  OTHERS:  {others:8.4}s ({pct:6.2}%)")
    }
}
