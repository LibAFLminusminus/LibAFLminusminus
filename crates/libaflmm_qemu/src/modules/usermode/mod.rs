#[cfg(feature = "injections")]
pub mod injections;
#[cfg(feature = "injections")]
pub use injections::InjectionModule;

#[cfg(not(cpu_target = "hexagon"))]
pub mod snapshot;
#[cfg(not(cpu_target = "hexagon"))]
pub use snapshot::{
    IntervalSnapshotFilter, IntervalSnapshotFilters, SnapshotModule, get_snapshot_module_mut,
};

#[cfg(all(feature = "asan_host", not(cpu_target = "hexagon")))]
pub mod asan_host;
#[cfg(all(feature = "asan_host", not(cpu_target = "hexagon")))]
pub use asan_host::AsanHostModule;

#[cfg(all(feature = "asan_guest", not(cpu_target = "hexagon")))]
pub mod asan_guest;
#[cfg(all(feature = "asan_guest", not(cpu_target = "hexagon")))]
pub use asan_guest::AsanGuestModule;

pub mod redirect_stdin;
pub use redirect_stdin::RedirectStdinModule;

pub mod redirect_stdout;
pub use redirect_stdout::RedirectStdoutModule;
