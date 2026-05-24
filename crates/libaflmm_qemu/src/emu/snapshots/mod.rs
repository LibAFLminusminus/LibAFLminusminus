use crate::{Result, emu::EmulatorError, qemu::Qemu};
use std::{
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};
use thiserror::Error;

#[cfg(feature = "systemmode")]
pub mod fast;
#[cfg(feature = "systemmode")]
pub use fast::{FastSnapshotManager, FastSnapshotPtr};

#[cfg(feature = "systemmode")]
pub mod qemu;
#[cfg(feature = "systemmode")]
pub use qemu::QemuSnapshotManager;

#[cfg(feature = "usermode")]
pub type StdSnapshotManager = NopSnapshotManager;
#[cfg(feature = "systemmode")]
pub type StdSnapshotManager = FastSnapshotManager;

pub trait SnapshotManager: Clone + Debug {
    fn init(&mut self, _qemu: Qemu) {}

    fn save(&mut self, qemu: Qemu) -> SnapshotId;
    fn restore(&mut self, qemu: Qemu, snapshot_id: &SnapshotId) -> Result<()>;
    fn do_check(
        &self,
        qemu: Qemu,
        reference_snapshot_id: &SnapshotId,
    ) -> Result<QemuSnapshotCheckResult>;

    fn check(&self, qemu: Qemu, reference_snapshot_id: &SnapshotId) -> Result<()> {
        let check_result = self.do_check(qemu, reference_snapshot_id)?;

        if check_result == QemuSnapshotCheckResult::default() {
            Ok(())
        } else {
            Err(SnapshotManagerCheckError::SnapshotCheckError(check_result).into())
        }
    }
}

#[cfg(feature = "systemmode")]
#[derive(Debug, Clone)]
pub enum AllSnapshotManager {
    Qemu(QemuSnapshotManager),
    Fast(FastSnapshotManager),
}

#[cfg(feature = "systemmode")]
impl SnapshotManager for AllSnapshotManager {
    fn save(&mut self, qemu: Qemu) -> SnapshotId {
        match self {
            AllSnapshotManager::Qemu(qemu_sm) => qemu_sm.save(qemu),
            AllSnapshotManager::Fast(fast_sm) => fast_sm.save(qemu),
        }
    }

    fn restore(&mut self, qemu: Qemu, snapshot_id: &SnapshotId) -> Result<()> {
        match self {
            AllSnapshotManager::Qemu(qemu_sm) => qemu_sm.restore(qemu, snapshot_id),
            AllSnapshotManager::Fast(fast_sm) => fast_sm.restore(qemu, snapshot_id),
        }
    }

    fn do_check(
        &self,
        qemu: Qemu,
        reference_snapshot_id: &SnapshotId,
    ) -> Result<QemuSnapshotCheckResult> {
        match self {
            AllSnapshotManager::Qemu(qemu_sm) => qemu_sm.do_check(qemu, reference_snapshot_id),
            AllSnapshotManager::Fast(fast_sm) => fast_sm.do_check(qemu, reference_snapshot_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QemuSnapshotCheckResult {
    pub nb_page_inconsistencies: u64,
}

#[derive(Debug, Clone, Error)]
pub enum SnapshotManagerError {
    #[error("snapshot id not found: {0:?}")]
    SnapshotIdNotFound(SnapshotId),
    #[error("memory inconsistencies: {0}")]
    MemoryInconsistencies(u64),
}

#[derive(Debug, Clone, Error)]
pub enum SnapshotManagerCheckError {
    #[error(transparent)]
    SnapshotManagerError(#[from] SnapshotManagerError),
    #[error("snapshot check failed: {0:?}")]
    SnapshotCheckError(QemuSnapshotCheckResult),
}

impl From<SnapshotManagerError> for crate::Error {
    fn from(error: SnapshotManagerError) -> Self {
        EmulatorError::from(error).into()
    }
}

impl From<SnapshotManagerCheckError> for crate::Error {
    fn from(error: SnapshotManagerCheckError) -> Self {
        EmulatorError::from(error).into()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct NopSnapshotManager;

impl Default for NopSnapshotManager {
    fn default() -> Self {
        NopSnapshotManager
    }
}

impl SnapshotManager for NopSnapshotManager {
    fn save(&mut self, _qemu: Qemu) -> SnapshotId {
        log::debug!("Saving snapshot with the NopSnapshotManager");
        SnapshotId { id: 0 }
    }

    fn restore(&mut self, _qemu: Qemu, _snapshot_id: &SnapshotId) -> Result<()> {
        log::debug!("Restoring snapshot with the NopSnapshotManager");
        Ok(())
    }

    fn do_check(
        &self,
        _qemu: Qemu,
        _reference_snapshot_id: &SnapshotId,
    ) -> Result<QemuSnapshotCheckResult> {
        Ok(QemuSnapshotCheckResult::default())
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct SnapshotId {
    id: u64,
}

/// Represents a QEMU snapshot check result for which no error was detected
impl Default for QemuSnapshotCheckResult {
    fn default() -> Self {
        Self {
            nb_page_inconsistencies: 0,
        }
    }
}

impl QemuSnapshotCheckResult {
    #[must_use]
    pub fn new(nb_page_inconsistencies: u64) -> Self {
        Self {
            nb_page_inconsistencies,
        }
    }
}

impl SnapshotId {
    pub fn gen_unique_id() -> SnapshotId {
        static UNIQUE_ID: AtomicU64 = AtomicU64::new(0);

        let unique_id = UNIQUE_ID.fetch_add(1, Ordering::SeqCst);

        SnapshotId { id: unique_id }
    }

    #[must_use]
    pub fn inner(&self) -> u64 {
        self.id
    }
}
