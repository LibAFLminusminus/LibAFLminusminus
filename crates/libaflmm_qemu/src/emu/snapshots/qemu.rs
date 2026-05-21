use crate::{
    emu::{QemuSnapshotCheckResult, SnapshotId, SnapshotManager, SnapshotManagerError},
    qemu::Qemu,
};

#[derive(Debug, Clone)]
pub struct QemuSnapshotManager {
    is_sync: bool,
}

impl Default for QemuSnapshotManager {
    fn default() -> Self {
        QemuSnapshotManager::new(true)
    }
}

impl QemuSnapshotManager {
    #[must_use]
    pub fn new(is_sync: bool) -> Self {
        Self { is_sync }
    }

    #[must_use]
    pub fn snapshot_id_to_name(&self, snapshot_id: &SnapshotId) -> String {
        format!("__libaflmm_qemu_snapshot_{}", snapshot_id.inner())
    }
}

impl SnapshotManager for QemuSnapshotManager {
    fn save(&mut self, qemu: Qemu) -> SnapshotId {
        let snapshot_id = SnapshotId::gen_unique_id();
        qemu.save_snapshot(
            self.snapshot_id_to_name(&snapshot_id).as_str(),
            self.is_sync,
        );
        snapshot_id
    }

    fn restore(
        &mut self,
        qemu: Qemu,
        snapshot_id: &SnapshotId,
    ) -> Result<(), SnapshotManagerError> {
        qemu.load_snapshot(self.snapshot_id_to_name(snapshot_id).as_str(), self.is_sync);
        Ok(())
    }

    fn do_check(
        &self,
        _qemu: Qemu,
        _reference_snapshot_id: &SnapshotId,
    ) -> Result<QemuSnapshotCheckResult, SnapshotManagerError> {
        // We consider the qemu implementation to be 'ideal' for now.
        Ok(QemuSnapshotCheckResult::default())
    }
}
