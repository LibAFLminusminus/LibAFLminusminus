use crate::qemu::CallingConvention;
use libaflmm_qemu_sys::{CPUStatePtr, GuestAddr};
use std::convert::Infallible;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum QemuError {
    #[error(transparent)]
    Init(#[from] QemuInitError),
    #[error(transparent)]
    Exit(#[from] QemuExitError),
    #[error(transparent)]
    RW(#[from] QemuRWError),
}

#[derive(Debug, Clone, Error)]
pub enum QemuInitError {
    #[error("Only one instance of the QEMU Emulator is permitted")]
    MultipleInstances,
    #[error("No parameters were provided to initialize QEMU.")]
    NoParametersProvided,
    #[error("QEMU emulator args cannot be empty")]
    EmptyArgs,
    #[error("Infallible error, should never be reached.")]
    Infallible,
    #[error("Too many arguments passed to QEMU emulator ({0} > i32::MAX)")]
    TooManyArgs(usize),
}

#[derive(Debug, Clone, Error)]
pub enum QemuExitError {
    /// Exit reason was not NULL, but exit kind is unknown. Should never happen.
    #[error("Unknown QEMU exit kind")]
    UnknownKind,
    /// QEMU exited without going through an expected exit point. Can be caused by a crash for example.
    #[error("Unexpected QEMU exit")]
    UnexpectedExit,
}

#[derive(Debug, Clone)]
pub enum QemuRWErrorKind {
    Read,
    Write,
}

#[derive(Debug, Clone)]
pub enum QemuRWErrorCause {
    WrongCallingConvention(CallingConvention, CallingConvention), // expected, given
    WrongArgument(u8),
    CurrentCpuNotFound,
    Reg(i32),
    WrongMemoryLocation(GuestAddr, usize), // addr, size
}

#[derive(Debug, Clone, Error)]
#[error("QEMU {kind:?} error: {cause:?}")]
pub struct QemuRWError {
    kind: QemuRWErrorKind,
    cause: QemuRWErrorCause,
    cpu: Option<CPUStatePtr>, // Only makes sense when cause != CurrentCpuNotFound
}

impl From<Infallible> for QemuInitError {
    fn from(_: Infallible) -> Self {
        QemuInitError::Infallible
    }
}

impl QemuRWError {
    #[must_use]
    pub fn new(kind: QemuRWErrorKind, cause: QemuRWErrorCause, cpu: Option<CPUStatePtr>) -> Self {
        Self { kind, cause, cpu }
    }

    pub fn wrong_reg(kind: QemuRWErrorKind, reg: i32, cpu: Option<CPUStatePtr>) -> Self {
        Self::new(kind, QemuRWErrorCause::Reg(reg.into()), cpu)
    }

    pub fn wrong_mem_location(
        kind: QemuRWErrorKind,
        cpu: CPUStatePtr,
        addr: GuestAddr,
        size: usize,
    ) -> Self {
        Self::new(
            kind,
            QemuRWErrorCause::WrongMemoryLocation(addr, size),
            Some(cpu),
        )
    }

    #[must_use]
    pub fn current_cpu_not_found(kind: QemuRWErrorKind) -> Self {
        Self::new(kind, QemuRWErrorCause::CurrentCpuNotFound, None)
    }

    #[must_use]
    pub fn new_argument_error(kind: QemuRWErrorKind, arg_id: u8) -> Self {
        Self::new(kind, QemuRWErrorCause::WrongArgument(arg_id), None)
    }

    pub fn check_conv(
        kind: QemuRWErrorKind,
        expected_conv: CallingConvention,
        given_conv: CallingConvention,
    ) -> std::result::Result<(), QemuRWError> {
        if expected_conv != given_conv {
            return Err(QemuRWError::new(
                kind,
                QemuRWErrorCause::WrongCallingConvention(expected_conv, given_conv),
                None,
            ));
        }

        Ok(())
    }
}

impl From<QemuError> for libaflmm::Error {
    fn from(qemu_error: QemuError) -> Self {
        libaflmm::Error::runtime(qemu_error)
    }
}

impl From<QemuError> for String {
    fn from(qemu_error: QemuError) -> Self {
        format!("LibAFL QEMU Error: {qemu_error}")
    }
}
