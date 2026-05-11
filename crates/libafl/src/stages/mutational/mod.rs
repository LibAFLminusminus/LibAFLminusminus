//! Mutational stages and friends take one [`Input`] from the [`Corpus`] and apply mutations for a bunch of times then run against the targets

pub mod std;
pub use std::*;

pub mod power;
use libafl_core::illegal_state;
pub use power::*;

use crate::{
    Result,
    common::PowerScheduleData,
    corpus::TestcaseId,
    states::{FlatState, unnamed_metadata_mut},
};

/// This is for power scheduling. It returns a "score" to decide how many times you want to mutate and test the [`Input`](crate::inputs::Input)  during one [`Stage`](crate::stages::Stage) .
pub trait Power<S> {
    /// The score of this testcase. Usually the score quantifies the "Goodness" of a [`Testcase`](crate::corpus::testcase::Testcase)
    fn score(state: &mut S, testcase_id: TestcaseId) -> Result<usize>;
}

const HAVOC_MAX_MULT: f64 = 16.0;

#[derive(Debug)]
/// Of course! the port of `calculate_score` from AFL
pub struct AFLPower {}

impl<S> Power<S> for AFLPower
where
    S: FlatState,
{
    #[expect(clippy::cast_precision_loss, clippy::cast_sign_loss)]
    fn score(state: &mut S, testcase_id: TestcaseId) -> Result<usize> {
        let psmeta = unnamed_metadata_mut::<PowerScheduleData>(state.named_metadata_map_mut())?;

        let avg_exec_us = psmeta.exec_time().as_nanos() as f64 / psmeta.cycles() as f64; // cycles is just a counter yet it is called cycles. don't blame me. i didn't name it.
        let avg_bitmap_size = if psmeta.bitmap_entries() == 0 {
            1
        } else {
            psmeta.bitmap_size() / psmeta.bitmap_entries() // bad name too but i didn't name it. 
        };

        let mut perf_score = 100.0;
        let meta = psmeta
            .per_testcase_data_mut(testcase_id)
            .ok_or(illegal_state!(
                "Cannot find per testcase metadata. Something is wrong.."
            ))?;
        let q_exec_us = meta.exec_time().as_nanos() as f64;
        if q_exec_us * 0.1 > avg_exec_us {
            perf_score = 10.0;
        } else if q_exec_us * 0.2 > avg_exec_us {
            perf_score = 25.0;
        } else if q_exec_us * 0.5 > avg_exec_us {
            perf_score = 50.0;
        } else if q_exec_us * 0.75 > avg_exec_us {
            perf_score = 75.0;
        } else if q_exec_us * 4.0 < avg_exec_us {
            perf_score = 300.0;
        } else if q_exec_us * 3.0 < avg_exec_us {
            perf_score = 200.0;
        } else if q_exec_us * 2.0 < avg_exec_us {
            perf_score = 150.0;
        }

        let q_bitmap_size = meta.bitmap_size() as f64;
        if q_bitmap_size * 0.3 > avg_bitmap_size as f64 {
            perf_score *= 3.0;
        } else if q_bitmap_size * 0.5 > avg_bitmap_size as f64 {
            perf_score *= 2.0;
        } else if q_bitmap_size * 0.75 > avg_bitmap_size as f64 {
            perf_score *= 1.5;
        } else if q_bitmap_size * 3.0 < avg_bitmap_size as f64 {
            perf_score *= 0.25;
        } else if q_bitmap_size * 2.0 < avg_bitmap_size as f64 {
            perf_score *= 0.5;
        } else if q_bitmap_size * 1.5 < avg_bitmap_size as f64 {
            perf_score *= 0.75;
        }

        if meta.handicap() >= 4 {
            perf_score *= 4.0;
            meta.set_handicap(meta.handicap() - 4);
        } else if meta.handicap() > 0 {
            perf_score *= 2.0;
            meta.set_handicap(meta.handicap() - 1);
        }

        match meta.depth() {
            0_u64..=3_u64 => (),
            4_u64..=7_u64 => perf_score *= 2.0,
            8_u64..=13_u64 => perf_score *= 3.0,
            14_u64..=25_u64 => perf_score *= 4.0,
            _ => perf_score *= 5.0,
        }

        if perf_score > HAVOC_MAX_MULT * 100.0 {
            perf_score = HAVOC_MAX_MULT * 100.0;
        }

        // lastly commit back changes
        Ok(perf_score.floor() as usize)
    }
}
