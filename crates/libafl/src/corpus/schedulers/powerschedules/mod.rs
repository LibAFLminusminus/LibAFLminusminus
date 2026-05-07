//! Powerschedule-related modules

libafl_bolts::impl_serdeany!(PowerScheduleData);
use crate::corpus::testcase::TestcaseId;
use serde::{Deserialize, Serialize};

use alloc::vec::Vec;
use core::time::Duration;
use hashbrown::HashMap;

const N_FUZZ_SIZE: usize = 1 << 21;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum PowerSchedule {
    /// The `explore` power schedule
    EXPLORE,
    /// The `exploit` power schedule
    EXPLOIT,
    /// The `fast` power schedule
    FAST,
    /// The `coe` power schedule
    COE,
    /// The `lin` power schedule
    LIN,
    /// The `quad` power schedule
    QUAD,
}

/// The metadata used for power schedules
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(miri, expect(clippy::unsafe_derive_deserialize))] // for SerdeAny
pub struct PowerScheduleData {
    /// Powerschedule strategy
    strat: Option<PowerSchedule>,
    /// Measured exec time during calibration
    exec_time: Duration,
    /// Calibration cycles
    cycles: u64,
    /// Size of the observer map
    bitmap_size: u64,
    /// Sum of `log(bitmap_size`)
    bitmap_size_log: f64,
    /// Number of filled map entries
    bitmap_entries: u64,
    /// Queue cycles
    queue_cycles: u64,
    /// The vector to contain the frequency of each execution path.
    n_fuzz: Vec<u32>,
    /// per testcase metadata
    per_testcase: HashMap<TestcaseId, TestcasePowerScheduleData>,
}

/// The metadata for runs in the calibration stage.
impl PowerScheduleData {
    /// Creates a new [`struct@PowerScheduleData`]
    #[must_use]
    pub fn new(strat: Option<PowerSchedule>) -> Self {
        Self {
            strat,
            exec_time: Duration::from_millis(0),
            cycles: 0,
            bitmap_size: 0,
            bitmap_size_log: 0.0,
            bitmap_entries: 0,
            queue_cycles: 0,
            n_fuzz: vec![0; N_FUZZ_SIZE],
            per_testcase: HashMap::new(),
        }
    }

    /// The `PowerSchedule`
    #[must_use]
    pub fn strat(&self) -> Option<PowerSchedule> {
        self.strat
    }

    /// Set the `PowerSchedule`
    pub fn set_strat(&mut self, strat: Option<PowerSchedule>) {
        self.strat = strat;
    }

    /// The measured exec time during calibration
    #[must_use]
    pub fn exec_time(&self) -> Duration {
        self.exec_time
    }

    /// Set the measured exec
    pub fn set_exec_time(&mut self, time: Duration) {
        self.exec_time = time;
    }

    /// The cycles
    #[must_use]
    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    /// Sets the cycles
    pub fn set_cycles(&mut self, val: u64) {
        self.cycles = val;
    }

    /// The bitmap size
    #[must_use]
    pub fn bitmap_size(&self) -> u64 {
        self.bitmap_size
    }

    /// Sets the bitmap size
    pub fn set_bitmap_size(&mut self, val: u64) {
        self.bitmap_size = val;
    }

    #[must_use]
    /// The sum of log(`bitmap_size`)
    pub fn bitmap_size_log(&self) -> f64 {
        self.bitmap_size_log
    }

    /// Setts the sum of log(`bitmap_size`)
    pub fn set_bitmap_size_log(&mut self, val: f64) {
        self.bitmap_size_log = val;
    }

    /// The number of filled map entries
    #[must_use]
    pub fn bitmap_entries(&self) -> u64 {
        self.bitmap_entries
    }

    /// Sets the number of filled map entries
    pub fn set_bitmap_entries(&mut self, val: u64) {
        self.bitmap_entries = val;
    }

    /// The amount of queue cycles
    #[must_use]
    pub fn queue_cycles(&self) -> u64 {
        self.queue_cycles
    }

    /// Sets the amount of queue cycles
    pub fn set_queue_cycles(&mut self, val: u64) {
        self.queue_cycles = val;
    }

    /// Gets the `n_fuzz`.
    #[must_use]
    pub fn n_fuzz(&self) -> &[u32] {
        &self.n_fuzz
    }

    /// Sets the `n_fuzz`.
    #[must_use]
    pub fn n_fuzz_mut(&mut self) -> &mut [u32] {
        &mut self.n_fuzz
    }

    pub fn per_testcase_data(&self, testcase_id: TestcaseId) -> Option<&TestcasePowerScheduleData>{
        self.per_testcase.get(&testcase_id)
    }

    pub fn per_testcase_data_mut(&mut self, testcase_id: TestcaseId) -> Option<&mut TestcasePowerScheduleData>{
        self.per_testcase.get_mut(&testcase_id)
    }
}

/// The Metadata for each testcase used in power schedules.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(miri, expect(clippy::unsafe_derive_deserialize))] // for SerdeAny
pub struct TestcasePowerScheduleData {
    /// parent id, none if it was from the seed corpus
    parent: Option<TestcaseId>,
    /// Average execution time measured in calibration
    exec_time: Duration,
    /// Number of bits set in bitmap, updated in `calibrate_case`
    bitmap_size: u64,
    /// Number of queue cycles behind
    handicap: u64,
    /// Path depth, initialized in `on_add`
    depth: u64,
    /// Offset in `n_fuzz`
    n_fuzz_entry: usize,
    /// Cycles used to calibrate this (not really needed if it were not for `on_replace` and `on_remove`)
    cycle_and_time: (Duration, usize),
}

impl TestcasePowerScheduleData {
    /// Create new [`struct@TestcasePowerScheduleData`]
    #[must_use]
    pub fn new(depth: u64) -> Self {
        Self {
            parent: None,
            exec_time: Duration::ZERO,
            bitmap_size: 0,
            handicap: 0,
            depth,
            n_fuzz_entry: 0,
            cycle_and_time: (Duration::default(), 0),
        }
    }

    #[inline]
    #[must_use]
    pub fn parent(&self) -> Option<&TestcaseId> {
        self.parent.as_ref()
    }

    #[inline]
    #[must_use]
    pub fn set_parent(&mut self, parent: TestcaseId) {
        self.parent = Some(parent);
    }

    #[inline]
    #[must_use]
    pub fn exec_time(&self) -> &Duration {
        &self.exec_time
    }

    #[inline]
    #[must_use]
    pub fn set_exec_time(&mut self, exec_time: Duration) {
        self.exec_time = exec_time;
    }

    /// Get the bitmap size
    #[inline]
    #[must_use]
    pub fn bitmap_size(&self) -> u64 {
        self.bitmap_size
    }

    /// Set the bitmap size
    #[inline]
    pub fn set_bitmap_size(&mut self, val: u64) {
        self.bitmap_size = val;
    }

    /// Get the handicap
    #[inline]
    #[must_use]
    pub fn handicap(&self) -> u64 {
        self.handicap
    }

    /// Set the handicap
    #[inline]
    pub fn set_handicap(&mut self, val: u64) {
        self.handicap = val;
    }

    /// Get the depth
    #[inline]
    #[must_use]
    pub fn depth(&self) -> u64 {
        self.depth
    }

    /// Set the depth
    #[inline]
    pub fn set_depth(&mut self, val: u64) {
        self.depth = val;
    }

    /// Get the `n_fuzz_entry`
    #[inline]
    #[must_use]
    pub fn n_fuzz_entry(&self) -> usize {
        self.n_fuzz_entry
    }

    /// Set the `n_fuzz_entry`
    #[inline]
    pub fn set_n_fuzz_entry(&mut self, val: usize) {
        self.n_fuzz_entry = val;
    }

    /// Get the cycles
    #[inline]
    #[must_use]
    pub fn cycle_and_time(&self) -> (Duration, usize) {
        self.cycle_and_time
    }

    #[inline]
    /// Setter for cycles
    pub fn set_cycle_and_time(&mut self, cycle_and_time: (Duration, usize)) {
        self.cycle_and_time = cycle_and_time;
    }
}

libafl_bolts::impl_serdeany!(TestcasePowerScheduleData);
