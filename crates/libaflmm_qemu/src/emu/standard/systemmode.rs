use crate::{
    emu::{StdEmulator, snapshots::FastSnapshotPtr},
    qemu::DeviceSnapshotFilter,
};
use libaflmm_qemu_sys::GuestPhysAddr;

impl<C, CM, ED, ET, I, S, SM> StdEmulator<C, CM, ED, ET, I, S, SM> {
    /// Write a value to a phsical guest address, including ROM areas.
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn write_phys_mem(&self, paddr: GuestPhysAddr, buf: &[u8]) {
        unsafe {
            self.qemu.write_phys_mem(paddr, buf);
        }
    }

    /// Read a value from a physical guest address.
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn read_phys_mem(&self, paddr: GuestPhysAddr, buf: &mut [u8]) {
        unsafe {
            self.qemu.read_phys_mem(paddr, buf);
        }
    }

    pub fn save_snapshot(&self, name: &str, sync: bool) {
        self.qemu.save_snapshot(name, sync);
    }

    pub fn load_snapshot(&self, name: &str, sync: bool) {
        self.qemu.load_snapshot(name, sync);
    }

    #[must_use]
    pub fn create_fast_snapshot(&self, track: bool) -> FastSnapshotPtr {
        self.qemu.create_fast_snapshot(track)
    }

    #[must_use]
    pub fn create_fast_snapshot_filter(
        &self,
        track: bool,
        device_filter: &DeviceSnapshotFilter,
    ) -> FastSnapshotPtr {
        self.qemu.create_fast_snapshot_filter(track, device_filter)
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn restore_fast_snapshot(&self, snapshot: FastSnapshotPtr) {
        unsafe {
            self.qemu.restore_fast_snapshot(snapshot);
        }
    }

    pub fn list_devices(&self) -> Vec<String> {
        self.qemu.list_devices()
    }
}
