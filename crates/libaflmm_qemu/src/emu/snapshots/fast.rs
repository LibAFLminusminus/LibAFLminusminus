use crate::{
    emu::{IsSnapshotManager, QemuSnapshotCheckResult, SnapshotId, SnapshotManagerError},
    qemu::Qemu,
};
use std::collections::HashMap;

pub type FastSnapshotPtr = *mut libaflmm_qemu_sys::SyxSnapshot;

#[derive(Debug, Clone)]
pub struct FastSnapshotManager {
    snapshots: HashMap<SnapshotId, FastSnapshotPtr>,
}

impl Default for FastSnapshotManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FastSnapshotManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
        }
    }
    #[allow(clippy::missing_safety_doc)]
    #[must_use]
    pub unsafe fn get(&self, id: &SnapshotId) -> FastSnapshotPtr {
        *self.snapshots.get(id).unwrap()
    }
}

impl IsSnapshotManager for FastSnapshotManager {
    fn save(&mut self, qemu: Qemu) -> SnapshotId {
        let snapshot_id = SnapshotId::gen_unique_id();
        self.snapshots
            .insert(snapshot_id, qemu.create_fast_snapshot(true));
        snapshot_id
    }

    fn restore(
        &mut self,
        qemu: Qemu,
        snapshot_id: &SnapshotId,
    ) -> Result<(), SnapshotManagerError> {
        let fast_snapshot_ptr = *self
            .snapshots
            .get(snapshot_id)
            .ok_or(SnapshotManagerError::SnapshotIdNotFound(*snapshot_id))?;

        unsafe {
            qemu.restore_fast_snapshot(fast_snapshot_ptr);
        }

        Ok(())
    }

    fn do_check(
        &self,
        qemu: Qemu,
        reference_snapshot_id: &SnapshotId,
    ) -> Result<QemuSnapshotCheckResult, SnapshotManagerError> {
        let fast_snapshot_ptr = *self.snapshots.get(reference_snapshot_id).ok_or(
            SnapshotManagerError::SnapshotIdNotFound(*reference_snapshot_id),
        )?;

        unsafe { Ok(qemu.check_fast_snapshot(fast_snapshot_ptr)) }
    }
}
