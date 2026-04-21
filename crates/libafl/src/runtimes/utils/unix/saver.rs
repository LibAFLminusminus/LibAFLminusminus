use std::os::fd::{FromRawFd, IntoRawFd};
use std::{fs::File, os::fd::OwnedFd};

pub struct OsSaver {
    writer: File,
}

impl OsSaver {
    pub unsafe fn new(write_fd: OwnedFd) -> Self {
        let writer = unsafe { File::from_raw_fd(write_fd.into_raw_fd()) };

        Self { writer }
    }
}
