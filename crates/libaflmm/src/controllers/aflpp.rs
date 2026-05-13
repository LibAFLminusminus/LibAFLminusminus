//! `AFL++`-style controller.

use std::{collections::HashMap, path::PathBuf, vec::Vec};

/// [`Controller`] for `AFL++`.
pub struct AflppController {
    secondary_queues: Vec<PathBuf>,
    last_evaluated: HashMap<PathBuf, Option<u64>>,
}

/// [`Worker`] for `AFL++`-style fuzzing.
pub enum AflppWorker {
    Main(),
    Secondary(),
}
