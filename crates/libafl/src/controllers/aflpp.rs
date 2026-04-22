use std::{collections::HashMap, path::PathBuf, vec::Vec};

pub struct MainClientController {
    secondary_queues: Vec<PathBuf>,
    last_evaluated: HashMap<PathBuf, Option<u64>>,
}

pub enum AflppController {
    Main(),
    Secondary(),
}
