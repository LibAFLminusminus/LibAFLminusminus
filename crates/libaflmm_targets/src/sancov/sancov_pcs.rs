//! PC-table callbacks necessary for libfuzzer shimming

static mut PC_TABLES: Vec<&'static [PcTableEntry]> = Vec::new();

/// An entry to the `sanitizer_cov` `pc_table`
#[repr(C, packed)]
#[derive(Debug, PartialEq, Eq)]
pub struct PcTableEntry {
    addr: usize,
    flags: usize,
}

impl PcTableEntry {
    /// Returns whether the PC corresponds to a function entry point.
    #[must_use]
    pub fn is_function_entry(&self) -> bool {
        self.flags == 0x1
    }

    /// Returns the address associated with this PC.
    #[must_use]
    pub fn addr(&self) -> usize {
        self.addr
    }
}

/// Returns an iterator over the PC tables. If no tables were registered, this will be empty.
pub fn sanitizer_cov_pc_table<'a>() -> impl Iterator<Item = &'a [PcTableEntry]> {
    // SAFETY: Once PCS_BEG and PCS_END have been initialized, will not be written to again. So
    // there's no TOCTOU issue.
    unsafe {
        let pc_tables_ptr = &raw const PC_TABLES;
        let pc_tables = &*pc_tables_ptr;
        pc_tables.iter().copied()
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn __sanitizer_cov_pcs_init(pcs_beg: *const usize, pcs_end: *const usize) {
    unsafe {
        let len = pcs_end.offset_from(pcs_beg);
        let Ok(len) = usize::try_from(len) else {
            panic!("Invalid PC Table bounds - start: {pcs_beg:x?} end: {pcs_end:x?}")
        };
        assert_eq!(
            len % 2,
            0,
            "PC Table size is not evens - start: {pcs_beg:x?} end: {pcs_end:x?}"
        );
        assert_eq!(
            (pcs_beg as usize) % align_of::<PcTableEntry>(),
            0,
            "Unaligned PC Table - start: {pcs_beg:x?} end: {pcs_end:x?}"
        );

        let pc_tables_ptr = &raw mut PC_TABLES;
        let pc_tables = &mut *pc_tables_ptr;
        pc_tables.push(core::slice::from_raw_parts(
            pcs_beg as *const PcTableEntry,
            len,
        ));
    }
}
