//! Exception handling for Windows

use alloc::vec::Vec;
use core::{
    cell::UnsafeCell,
    fmt::{self, Display, Formatter},
    ptr::{self, write_volatile},
    sync::atomic::{Ordering, compiler_fence},
};
use libaflmm_core::Error;
use num_enum::FromPrimitive;
pub use windows::Win32::{
    Foundation::NTSTATUS,
    System::{
        Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, PHANDLER_ROUTINE, SetConsoleCtrlHandler},
        Diagnostics::Debug::{
            AddVectoredExceptionHandler, EXCEPTION_POINTERS, UnhandledExceptionFilter,
        },
        Threading::{IsProcessorFeaturePresent, PROCESSOR_FEATURE_ID},
    },
};
use windows::Win32::{
    Foundation::{
        DBG_COMMAND_EXCEPTION, DBG_CONTROL_BREAK, DBG_CONTROL_C, DBG_EXCEPTION_NOT_HANDLED,
        DBG_PRINTEXCEPTION_C, DBG_PRINTEXCEPTION_WIDE_C, DBG_RIPEXCEPTION, DBG_TERMINATE_PROCESS,
        DBG_TERMINATE_THREAD, EXCEPTION_POSSIBLE_DEADLOCK, STATUS_ABANDONED_WAIT_0,
        STATUS_ACCESS_VIOLATION, STATUS_ARRAY_BOUNDS_EXCEEDED, STATUS_ASSERTION_FAILURE,
        STATUS_BREAKPOINT, STATUS_CONTROL_C_EXIT, STATUS_DATATYPE_MISALIGNMENT,
        STATUS_DATATYPE_MISALIGNMENT_ERROR, STATUS_DLL_INIT_FAILED, STATUS_DLL_NOT_FOUND,
        STATUS_ENTRYPOINT_NOT_FOUND, STATUS_FATAL_APP_EXIT, STATUS_FATAL_USER_CALLBACK_EXCEPTION,
        STATUS_FLOAT_DENORMAL_OPERAND, STATUS_FLOAT_DIVIDE_BY_ZERO, STATUS_FLOAT_INEXACT_RESULT,
        STATUS_FLOAT_INVALID_OPERATION, STATUS_FLOAT_MULTIPLE_FAULTS, STATUS_FLOAT_MULTIPLE_TRAPS,
        STATUS_FLOAT_OVERFLOW, STATUS_FLOAT_STACK_CHECK, STATUS_FLOAT_UNDERFLOW,
        STATUS_GUARD_PAGE_VIOLATION, STATUS_HEAP_CORRUPTION, STATUS_ILLEGAL_FLOAT_CONTEXT,
        STATUS_ILLEGAL_INSTRUCTION, STATUS_IN_PAGE_ERROR, STATUS_INTEGER_DIVIDE_BY_ZERO,
        STATUS_INTEGER_OVERFLOW, STATUS_INVALID_CRUNTIME_PARAMETER, STATUS_INVALID_DISPOSITION,
        STATUS_INVALID_EXCEPTION_HANDLER, STATUS_INVALID_HANDLE, STATUS_INVALID_PARAMETER,
        STATUS_LONGJUMP, STATUS_NO_MEMORY, STATUS_NONCONTINUABLE_EXCEPTION, STATUS_NOT_IMPLEMENTED,
        STATUS_ORDINAL_NOT_FOUND, STATUS_PENDING, STATUS_PRIVILEGED_INSTRUCTION,
        STATUS_REG_NAT_CONSUMPTION, STATUS_SEGMENT_NOTIFICATION, STATUS_SINGLE_STEP,
        STATUS_STACK_BUFFER_OVERRUN, STATUS_STACK_OVERFLOW, STATUS_SXS_EARLY_DEACTIVATION,
        STATUS_SXS_INVALID_DEACTIVATION, STATUS_TIMEOUT, STATUS_UNWIND_CONSOLIDATE,
        STATUS_USER_APC, STATUS_WAIT_0, STATUS_WX86_BREAKPOINT, STATUS_WX86_CONTINUE,
        STATUS_WX86_CREATEWX86TIB, STATUS_WX86_EXCEPTION_CHAIN, STATUS_WX86_EXCEPTION_CONTINUE,
        STATUS_WX86_EXCEPTION_LASTCHANCE, STATUS_WX86_SINGLE_STEP, STATUS_WX86_UNSIMULATE,
    },
    System::Diagnostics::Debug::{EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH},
};
pub use windows::core::BOOL;

/// The special exit code when the target exited through ctrl-c
pub const CTRL_C_EXIT: i32 = STATUS_CONTROL_C_EXIT.0;

pub use libc::{SIGABRT, SIGFPE, SIGILL, SIGINT, SIGSEGV, SIGTERM};

// not exposed by libc for windows
pub const SIGABRT_COMPAT: i32 = 6;
pub const SIGBREAK: i32 = 21;
pub const SIGABRT2: i32 = SIGABRT;

// not part of the windows crate
const EXCEPTION_RO_ORIGINATEERROR: NTSTATUS = NTSTATUS(0x4008_0201_u32.cast_signed());
const EXCEPTION_RO_TRANSFORMERROR: NTSTATUS = NTSTATUS(0x4008_0202_u32.cast_signed());
const MS_VC_EXCEPTION: NTSTATUS = NTSTATUS(0x406D_1388_u32.cast_signed());
const VCPP_EXCEPTION_ERROR_INVALID_PARAMETER: NTSTATUS = NTSTATUS(0xC06D_0057_u32.cast_signed());
const VCPP_EXCEPTION_ERROR_MOD_NOT_FOUND: NTSTATUS = NTSTATUS(0xC06D_007E_u32.cast_signed());
const VCPP_EXCEPTION_ERROR_PROC_NOT_FOUND: NTSTATUS = NTSTATUS(0xC06D_007F_u32.cast_signed());
const CLR_EXCEPTION: NTSTATUS = NTSTATUS(0xE043_4352_u32.cast_signed());
const CPP_EH_EXCEPTION: NTSTATUS = NTSTATUS(0xE06D_7363_u32.cast_signed());

#[derive(Debug, FromPrimitive, Copy, Clone)]
#[repr(i32)]
pub enum ExceptionCode {
    WaitZero = STATUS_WAIT_0.0,
    AbandonedWaitZero = STATUS_ABANDONED_WAIT_0.0,
    UserApc = STATUS_USER_APC.0,
    Timeout = STATUS_TIMEOUT.0,
    Pending = STATUS_PENDING.0,
    SegmentNotification = STATUS_SEGMENT_NOTIFICATION.0,
    FatalAppExit = STATUS_FATAL_APP_EXIT.0,
    GuardPageViolation = STATUS_GUARD_PAGE_VIOLATION.0,
    DatatypeMisalignment = STATUS_DATATYPE_MISALIGNMENT.0,
    Breakpoint = STATUS_BREAKPOINT.0,
    SingleStep = STATUS_SINGLE_STEP.0,
    Longjump = STATUS_LONGJUMP.0,
    UnwindConsolidate = STATUS_UNWIND_CONSOLIDATE.0,
    AccessViolation = STATUS_ACCESS_VIOLATION.0,
    InPageError = STATUS_IN_PAGE_ERROR.0,
    InvalidHandle = STATUS_INVALID_HANDLE.0,
    NoMemory = STATUS_NO_MEMORY.0,
    IllegalInstruction = STATUS_ILLEGAL_INSTRUCTION.0,
    NoncontinuableException = STATUS_NONCONTINUABLE_EXCEPTION.0,
    InvalidDisposition = STATUS_INVALID_DISPOSITION.0,
    ArrayBoundsExceeded = STATUS_ARRAY_BOUNDS_EXCEEDED.0,
    FloatDenormalOperand = STATUS_FLOAT_DENORMAL_OPERAND.0,
    FloatDivideByZero = STATUS_FLOAT_DIVIDE_BY_ZERO.0,
    FloatInexactResult = STATUS_FLOAT_INEXACT_RESULT.0,
    FloatInvalidOperation = STATUS_FLOAT_INVALID_OPERATION.0,
    FloatOverflow = STATUS_FLOAT_OVERFLOW.0,
    FloatStackCheck = STATUS_FLOAT_STACK_CHECK.0,
    FloatUnderflow = STATUS_FLOAT_UNDERFLOW.0,
    IntegerDivideByZero = STATUS_INTEGER_DIVIDE_BY_ZERO.0,
    IntegerOverflow = STATUS_INTEGER_OVERFLOW.0,
    PrivilegedInstruction = STATUS_PRIVILEGED_INSTRUCTION.0,
    StackOverflow = STATUS_STACK_OVERFLOW.0,
    DllNotFound = STATUS_DLL_NOT_FOUND.0,
    OrdinalNotFound = STATUS_ORDINAL_NOT_FOUND.0,
    EntrypointNotFound = STATUS_ENTRYPOINT_NOT_FOUND.0,
    ControlCExit = STATUS_CONTROL_C_EXIT.0,
    DllInitFailed = STATUS_DLL_INIT_FAILED.0,
    FloatMultipleFaults = STATUS_FLOAT_MULTIPLE_FAULTS.0,
    FloatMultipleTraps = STATUS_FLOAT_MULTIPLE_TRAPS.0,
    RegNatConsumption = STATUS_REG_NAT_CONSUMPTION.0,
    HeapCorruption = STATUS_HEAP_CORRUPTION.0,
    StackBufferOverrun = STATUS_STACK_BUFFER_OVERRUN.0,
    InvalidCruntimeParameter = STATUS_INVALID_CRUNTIME_PARAMETER.0,
    AssertionFailure = STATUS_ASSERTION_FAILURE.0,
    SxsEarlyDeactivation = STATUS_SXS_EARLY_DEACTIVATION.0,
    SxsInvalidDeactivation = STATUS_SXS_INVALID_DEACTIVATION.0,
    NotImplemented = STATUS_NOT_IMPLEMENTED.0,

    Wx86Unsimulate = STATUS_WX86_UNSIMULATE.0,
    Wx86Continue = STATUS_WX86_CONTINUE.0,
    Wx86SingleStep = STATUS_WX86_SINGLE_STEP.0,
    Wx86Breakpoint = STATUS_WX86_BREAKPOINT.0,
    Wx86ExceptionContinue = STATUS_WX86_EXCEPTION_CONTINUE.0,
    Wx86ExceptionLastchance = STATUS_WX86_EXCEPTION_LASTCHANCE.0,
    Wx86ExceptionChain = STATUS_WX86_EXCEPTION_CHAIN.0,
    Wx86Createwx86Tib = STATUS_WX86_CREATEWX86TIB.0,
    DbgTerminateThread = DBG_TERMINATE_THREAD.0,
    DbgTerminateProcess = DBG_TERMINATE_PROCESS.0,
    DbgControlC = DBG_CONTROL_C.0,
    DbgPrintexceptionC = DBG_PRINTEXCEPTION_C.0,
    DbgRipexception = DBG_RIPEXCEPTION.0,
    DbgControlBreak = DBG_CONTROL_BREAK.0,
    DbgCommandException = DBG_COMMAND_EXCEPTION.0,
    DbgPrintexceptionWideC = DBG_PRINTEXCEPTION_WIDE_C.0,
    ExceptionRoOriginateError = EXCEPTION_RO_ORIGINATEERROR.0,
    ExceptionRoTransformError = EXCEPTION_RO_TRANSFORMERROR.0,
    MsVcException = MS_VC_EXCEPTION.0,
    DbgExceptionNotHandled = DBG_EXCEPTION_NOT_HANDLED.0,
    InvalidParameter = STATUS_INVALID_PARAMETER.0,
    IllegalFloatContext = STATUS_ILLEGAL_FLOAT_CONTEXT.0,
    ExceptionPossibleDeadlock = EXCEPTION_POSSIBLE_DEADLOCK.0,
    InvalidExceptionHandler = STATUS_INVALID_EXCEPTION_HANDLER.0,
    DatatypeMisalignmentError = STATUS_DATATYPE_MISALIGNMENT_ERROR.0,
    UserCallback = STATUS_FATAL_USER_CALLBACK_EXCEPTION.0,
    ClrException = CLR_EXCEPTION.0,
    CppEhException = CPP_EH_EXCEPTION.0,
    VcppExceptionErrorInvalidParameter = VCPP_EXCEPTION_ERROR_INVALID_PARAMETER.0,
    VcppExceptionErrorModNotFound = VCPP_EXCEPTION_ERROR_MOD_NOT_FOUND.0,
    VcppExceptionErrorProcNotFound = VCPP_EXCEPTION_ERROR_PROC_NOT_FOUND.0,
    #[default]
    Others,
}

pub static CRASH_EXCEPTIONS: &[ExceptionCode] = &[
    ExceptionCode::AccessViolation,
    ExceptionCode::ArrayBoundsExceeded,
    ExceptionCode::FloatDivideByZero,
    ExceptionCode::GuardPageViolation,
    ExceptionCode::IllegalInstruction,
    ExceptionCode::InPageError,
    ExceptionCode::IntegerDivideByZero,
    ExceptionCode::InvalidHandle,
    ExceptionCode::NoncontinuableException,
    ExceptionCode::PrivilegedInstruction,
    ExceptionCode::StackOverflow,
    ExceptionCode::HeapCorruption,
    ExceptionCode::StackBufferOverrun,
    ExceptionCode::AssertionFailure,
];

impl PartialEq for ExceptionCode {
    fn eq(&self, other: &Self) -> bool {
        *self as i32 == *other as i32
    }
}

impl Eq for ExceptionCode {}

impl Display for ExceptionCode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ExceptionCode::WaitZero => write!(f, "STATUS_WAIT_0"),
            ExceptionCode::AbandonedWaitZero => write!(f, "STATUS_ABANDONED_WAIT_0"),
            ExceptionCode::UserApc => write!(f, "STATUS_USER_APC"),
            ExceptionCode::Timeout => write!(f, "STATUS_TIMEOUT"),
            ExceptionCode::Pending => write!(f, "STATUS_PENDING"),
            ExceptionCode::SegmentNotification => write!(f, "STATUS_SEGMENT_NOTIFICATION"),
            ExceptionCode::FatalAppExit => write!(f, "STATUS_FATAL_APP_EXIT"),
            ExceptionCode::GuardPageViolation => write!(f, "STATUS_GUARD_PAGE_VIOLATION"),
            ExceptionCode::DatatypeMisalignment => write!(f, "STATUS_DATATYPE_MISALIGNMENT"),
            ExceptionCode::Breakpoint => write!(f, "STATUS_BREAKPOINT"),
            ExceptionCode::SingleStep => write!(f, "STATUS_SINGLE_STEP"),
            ExceptionCode::Longjump => write!(f, "STATUS_LONGJUMP"),
            ExceptionCode::UnwindConsolidate => write!(f, "STATUS_UNWIND_CONSOLIDATE"),
            ExceptionCode::AccessViolation => write!(f, "STATUS_ACCESS_VIOLATION"),
            ExceptionCode::InPageError => write!(f, "STATUS_IN_PAGE_ERROR"),
            ExceptionCode::InvalidHandle => write!(f, "STATUS_INVALID_HANDLE"),
            ExceptionCode::NoMemory => write!(f, "STATUS_NO_MEMORY"),
            ExceptionCode::IllegalInstruction => write!(f, "STATUS_ILLEGAL_INSTRUCTION"),
            ExceptionCode::NoncontinuableException => write!(f, "STATUS_NONCONTINUABLE_EXCEPTION"),
            ExceptionCode::InvalidDisposition => write!(f, "STATUS_INVALID_DISPOSITION"),
            ExceptionCode::ArrayBoundsExceeded => write!(f, "STATUS_ARRAY_BOUNDS_EXCEEDED"),
            ExceptionCode::FloatDenormalOperand => write!(f, "STATUS_FLOAT_DENORMAL_OPERAND"),
            ExceptionCode::FloatDivideByZero => write!(f, "STATUS_FLOAT_DIVIDE_BY_ZERO"),
            ExceptionCode::FloatInexactResult => write!(f, "STATUS_FLOAT_INEXACT_RESULT"),
            ExceptionCode::FloatInvalidOperation => write!(f, "STATUS_FLOAT_INVALID_OPERATION"),
            ExceptionCode::FloatOverflow => write!(f, "STATUS_FLOAT_OVERFLOW"),
            ExceptionCode::FloatStackCheck => write!(f, "STATUS_FLOAT_STACK_CHECK"),
            ExceptionCode::FloatUnderflow => write!(f, "STATUS_FLOAT_UNDERFLOW"),
            ExceptionCode::IntegerDivideByZero => write!(f, "STATUS_INTEGER_DIVIDE_BY_ZERO"),
            ExceptionCode::IntegerOverflow => write!(f, "STATUS_INTEGER_OVERFLOW"),
            ExceptionCode::PrivilegedInstruction => write!(f, "STATUS_PRIVILEGED_INSTRUCTION"),
            ExceptionCode::StackOverflow => write!(f, "STATUS_STACK_OVERFLOW"),
            ExceptionCode::DllNotFound => write!(f, "STATUS_DLL_NOT_FOUND"),
            ExceptionCode::OrdinalNotFound => write!(f, "STATUS_ORDINAL_NOT_FOUND"),
            ExceptionCode::EntrypointNotFound => write!(f, "STATUS_ENTRYPOINT_NOT_FOUND"),
            ExceptionCode::ControlCExit => write!(f, "STATUS_CONTROL_C_EXIT"),
            ExceptionCode::DllInitFailed => write!(f, "STATUS_DLL_INIT_FAILED"),
            ExceptionCode::FloatMultipleFaults => write!(f, "STATUS_FLOAT_MULTIPLE_FAULTS"),
            ExceptionCode::FloatMultipleTraps => write!(f, "STATUS_FLOAT_MULTIPLE_TRAPS"),
            ExceptionCode::RegNatConsumption => write!(f, "STATUS_REG_NAT_CONSUMPTION"),
            ExceptionCode::HeapCorruption => write!(f, "STATUS_HEAP_CORRUPTION"),
            ExceptionCode::StackBufferOverrun => write!(f, "STATUS_STACK_BUFFER_OVERRUN"),
            ExceptionCode::InvalidCruntimeParameter => {
                write!(f, "STATUS_INVALID_CRUNTIME_PARAMETER")
            }
            ExceptionCode::AssertionFailure => write!(f, "STATUS_ASSERTION_FAILURE"),
            ExceptionCode::SxsEarlyDeactivation => write!(f, "STATUS_SXS_EARLY_DEACTIVATION"),
            ExceptionCode::SxsInvalidDeactivation => write!(f, "STATUS_SXS_INVALID_DEACTIVATION"),
            ExceptionCode::NotImplemented => write!(f, "STATUS_NOT_IMPLEMENTED"),
            ExceptionCode::Wx86Unsimulate => write!(f, "STATUS_WX86_UNSIMULATE"),
            ExceptionCode::Wx86Continue => write!(f, "STATUS_WX86_CONTINUE"),
            ExceptionCode::Wx86SingleStep => write!(f, "STATUS_WX86_SINGLE_STEP"),
            ExceptionCode::Wx86Breakpoint => write!(f, "STATUS_WX86_BREAKPOINT"),
            ExceptionCode::Wx86ExceptionContinue => write!(f, "STATUS_WX86_EXCEPTION_CONTINUE"),
            ExceptionCode::Wx86ExceptionLastchance => write!(f, "STATUS_WX86_EXCEPTION_LASTCHANCE"),
            ExceptionCode::Wx86ExceptionChain => write!(f, "STATUS_WX86_EXCEPTION_CHAIN"),
            ExceptionCode::Wx86Createwx86Tib => write!(f, "STATUS_WX86_CREATEWX86TIB"),
            ExceptionCode::DbgTerminateThread => write!(f, "DBG_TERMINATE_THREAD"),
            ExceptionCode::DbgTerminateProcess => write!(f, "DBG_TERMINATE_PROCESS"),
            ExceptionCode::DbgControlC => write!(f, "DBG_CONTROL_C"),
            ExceptionCode::DbgPrintexceptionC => write!(f, "DBG_PRINTEXCEPTION_C"),
            ExceptionCode::DbgRipexception => write!(f, "DBG_RIPEXCEPTION"),
            ExceptionCode::DbgControlBreak => write!(f, "DBG_CONTROL_BREAK"),
            ExceptionCode::DbgCommandException => write!(f, "DBG_COMMAND_EXCEPTION"),
            ExceptionCode::DbgPrintexceptionWideC => write!(f, "DBG_PRINTEXCEPTION_WIDE_C"),
            ExceptionCode::ExceptionRoOriginateError => write!(f, "EXCEPTION_RO_ORIGINATEERROR"),
            ExceptionCode::ExceptionRoTransformError => write!(f, "EXCEPTION_RO_TRANSFORMERROR"),
            ExceptionCode::MsVcException => write!(f, "MS_VC_EXCEPTION"),
            ExceptionCode::DbgExceptionNotHandled => write!(f, "DBG_EXCEPTION_NOT_HANDLED"),
            ExceptionCode::InvalidParameter => write!(f, "STATUS_INVALID_PARAMETER"),
            ExceptionCode::IllegalFloatContext => write!(f, "STATUS_ILLEGAL_FLOAT_CONTEXT"),
            ExceptionCode::ExceptionPossibleDeadlock => write!(f, "EXCEPTION_POSSIBLE_DEADLOCK"),
            ExceptionCode::InvalidExceptionHandler => write!(f, "STATUS_INVALID_EXCEPTION_HANDLER"),
            ExceptionCode::DatatypeMisalignmentError => {
                write!(f, "STATUS_DATATYPE_MISALIGNMENT_ERROR")
            }
            ExceptionCode::UserCallback => write!(f, "STATUS_USER_CALLBACK"),
            ExceptionCode::ClrException => write!(f, "CLR_EXCEPTION"),
            ExceptionCode::CppEhException => write!(f, "CPP_EH_EXCEPTION"),
            ExceptionCode::VcppExceptionErrorInvalidParameter => {
                write!(f, "VCPP_EXCEPTION_ERROR_INVALID_PARAMETER")
            }
            ExceptionCode::VcppExceptionErrorModNotFound => {
                write!(f, "VCPP_EXCEPTION_ERROR_MOD_NOT_FOUND")
            }
            ExceptionCode::VcppExceptionErrorProcNotFound => {
                write!(f, "VCPP_EXCEPTION_ERROR_PROC_NOT_FOUND")
            }
            ExceptionCode::Others => write!(f, "Unknown exception code"),
        }
    }
}

pub static EXCEPTION_CODES_MAPPING: [ExceptionCode; 79] = [
    ExceptionCode::WaitZero,
    ExceptionCode::AbandonedWaitZero,
    ExceptionCode::UserApc,
    ExceptionCode::Timeout,
    ExceptionCode::Pending,
    ExceptionCode::SegmentNotification,
    ExceptionCode::FatalAppExit,
    ExceptionCode::GuardPageViolation,
    ExceptionCode::DatatypeMisalignment,
    ExceptionCode::Breakpoint,
    ExceptionCode::SingleStep,
    ExceptionCode::Longjump,
    ExceptionCode::UnwindConsolidate,
    ExceptionCode::AccessViolation,
    ExceptionCode::InPageError,
    ExceptionCode::InvalidHandle,
    ExceptionCode::NoMemory,
    ExceptionCode::IllegalInstruction,
    ExceptionCode::NoncontinuableException,
    ExceptionCode::InvalidDisposition,
    ExceptionCode::ArrayBoundsExceeded,
    ExceptionCode::FloatDenormalOperand,
    ExceptionCode::FloatDivideByZero,
    ExceptionCode::FloatInexactResult,
    ExceptionCode::FloatInvalidOperation,
    ExceptionCode::FloatOverflow,
    ExceptionCode::FloatStackCheck,
    ExceptionCode::FloatUnderflow,
    ExceptionCode::IntegerDivideByZero,
    ExceptionCode::IntegerOverflow,
    ExceptionCode::PrivilegedInstruction,
    ExceptionCode::StackOverflow,
    ExceptionCode::DllNotFound,
    ExceptionCode::OrdinalNotFound,
    ExceptionCode::EntrypointNotFound,
    ExceptionCode::ControlCExit,
    ExceptionCode::DllInitFailed,
    ExceptionCode::FloatMultipleFaults,
    ExceptionCode::FloatMultipleTraps,
    ExceptionCode::RegNatConsumption,
    ExceptionCode::HeapCorruption,
    ExceptionCode::StackBufferOverrun,
    ExceptionCode::InvalidCruntimeParameter,
    ExceptionCode::AssertionFailure,
    ExceptionCode::SxsEarlyDeactivation,
    ExceptionCode::SxsInvalidDeactivation,
    ExceptionCode::NotImplemented,
    ExceptionCode::Wx86Unsimulate,
    ExceptionCode::Wx86Continue,
    ExceptionCode::Wx86SingleStep,
    ExceptionCode::Wx86Breakpoint,
    ExceptionCode::Wx86ExceptionContinue,
    ExceptionCode::Wx86ExceptionLastchance,
    ExceptionCode::Wx86ExceptionChain,
    ExceptionCode::Wx86Createwx86Tib,
    ExceptionCode::DbgTerminateThread,
    ExceptionCode::DbgTerminateProcess,
    ExceptionCode::DbgControlC,
    ExceptionCode::DbgPrintexceptionC,
    ExceptionCode::DbgRipexception,
    ExceptionCode::DbgControlBreak,
    ExceptionCode::DbgCommandException,
    ExceptionCode::DbgPrintexceptionWideC,
    ExceptionCode::ExceptionRoOriginateError,
    ExceptionCode::ExceptionRoTransformError,
    ExceptionCode::MsVcException,
    ExceptionCode::DbgExceptionNotHandled,
    ExceptionCode::InvalidParameter,
    ExceptionCode::IllegalFloatContext,
    ExceptionCode::ExceptionPossibleDeadlock,
    ExceptionCode::InvalidExceptionHandler,
    ExceptionCode::DatatypeMisalignmentError,
    ExceptionCode::UserCallback,
    ExceptionCode::ClrException,
    ExceptionCode::CppEhException,
    ExceptionCode::VcppExceptionErrorInvalidParameter,
    ExceptionCode::VcppExceptionErrorModNotFound,
    ExceptionCode::VcppExceptionErrorProcNotFound,
    ExceptionCode::Others,
];

pub trait ExceptionHandler {
    /// Handle an exception
    ///
    /// # Safety
    /// This is generally not safe to call. It should only be called through the signal it was registered for.
    /// Signal handling is hard, don't mess with it :).
    unsafe fn handle(
        &mut self,
        exception_code: ExceptionCode,
        exception_pointers: *mut EXCEPTION_POINTERS,
    );
    /// Return a list of exceptions to handle
    fn exceptions(&self) -> Vec<ExceptionCode>;
}

struct HandlerHolder {
    handler: UnsafeCell<*mut dyn ExceptionHandler>,
}

pub const EXCEPTION_HANDLERS_SIZE: usize = 96;

unsafe impl Send for HandlerHolder {}

/// Keep track of which handler is registered for which exception
static mut EXCEPTION_HANDLERS: [Option<HandlerHolder>; EXCEPTION_HANDLERS_SIZE] = [
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
];

unsafe fn internal_handle_exception(
    exception_code: ExceptionCode,
    exception_pointers: *mut EXCEPTION_POINTERS,
) -> i32 {
    let index = EXCEPTION_CODES_MAPPING
        .iter()
        .position(|x| *x == exception_code)
        .unwrap();
    if let Some(handler_holder) = unsafe { &EXCEPTION_HANDLERS[index] } {
        log::info!(
            "{:?}: Handling exception {}",
            std::process::id(),
            exception_code
        );
        let handler = unsafe { &mut **handler_holder.handler.get() };
        unsafe {
            handler.handle(exception_code, exception_pointers);
        }
        EXCEPTION_CONTINUE_EXECUTION
    } else {
        log::info!(
            "{:?}: No handler for exception {}",
            std::process::id(),
            exception_code
        );
        // Go to Default one
        if let Some(handler_holder) = unsafe { &EXCEPTION_HANDLERS[EXCEPTION_HANDLERS_SIZE - 1] } {
            let handler = unsafe { &mut **handler_holder.handler.get() };
            unsafe {
                handler.handle(exception_code, exception_pointers);
            }
        }
        EXCEPTION_CONTINUE_SEARCH
    }
}

/// Raise an exception following our exception handlers.
/// Convenient when it's an exception triggered by the library directly.
///
/// # Safety
///
/// `exception_pointers` but be NULL or a valid pointer.
pub unsafe fn raise_exception(
    exception_code: ExceptionCode,
    exception_pointers: *mut EXCEPTION_POINTERS,
) {
    unsafe {
        internal_handle_exception(exception_code, exception_pointers);
    }
}

/// Function that is being called whenever an exception arrives (stdcall).
/// # Safety
/// This function is unsafe because it is called by the OS
pub unsafe extern "system" fn handle_exception(exception_pointers: *mut EXCEPTION_POINTERS) -> i32 {
    let code = unsafe {
        exception_pointers
            .as_mut()
            .unwrap()
            .ExceptionRecord
            .as_mut()
            .unwrap()
            .ExceptionCode
    };
    let exception_code = From::from(code.0);
    log::info!("Received exception; code: {exception_code}");
    unsafe { internal_handle_exception(exception_code, exception_pointers) }
}

unsafe extern "C" fn handle_signal(_signum: i32) {
    // log::info!("Received signal {}", _signum);
    unsafe {
        internal_handle_exception(ExceptionCode::AssertionFailure, ptr::null_mut());
    }
}

/// Setup Win32 exception handlers in a somewhat rusty way.
///
/// # Safety
/// Exception handlers are usually ugly, handle with care!
pub unsafe fn setup_exception_handler<T: 'static + ExceptionHandler>(
    handler: *mut T,
) -> Result<(), Error> {
    let exceptions = unsafe { (*handler).exceptions() };
    let mut catch_assertions = false;
    for exception_code in exceptions {
        if exception_code == ExceptionCode::AssertionFailure {
            catch_assertions = true;
        }
        let index = EXCEPTION_CODES_MAPPING
            .iter()
            .position(|x| *x == exception_code)
            .unwrap();
        unsafe {
            write_volatile(
                &raw mut EXCEPTION_HANDLERS[index],
                Some(HandlerHolder {
                    handler: UnsafeCell::new(handler as *mut dyn ExceptionHandler),
                }),
            );
        }
    }

    unsafe {
        write_volatile(
            &raw mut (EXCEPTION_HANDLERS[EXCEPTION_HANDLERS_SIZE - 1]),
            Some(HandlerHolder {
                handler: UnsafeCell::new(handler as *mut dyn ExceptionHandler),
            }),
        );
    }
    compiler_fence(Ordering::SeqCst);
    if catch_assertions {
        unsafe {
            libc::signal(SIGABRT, handle_signal as *const () as libc::sighandler_t);
        }
    }
    // SetUnhandledFilter does not work with frida since the stack is changed and exception handler is lost with Stalker enabled.
    // See https://github.com/AFLplusplus/LibAFL/pull/403
    unsafe {
        AddVectoredExceptionHandler(0, Some(handle_exception));
    }
    Ok(())
}

pub trait CtrlHandler {
    /// Handle an exception
    fn handle(&mut self, ctrl_type: u32) -> bool;
}

struct CtrlHandlerHolder {
    handler: UnsafeCell<*mut dyn CtrlHandler>,
}

/// Keep track of which handler is registered for which exception
static mut CTRL_HANDLER: Option<CtrlHandlerHolder> = None;

/// Set `ConsoleCtrlHandler` to catch Ctrl-C
///
/// # Safety
/// Same safety considerations as in `setup_exception_handler`
pub unsafe fn setup_ctrl_handler<T: 'static + CtrlHandler>(handler: *mut T) -> Result<(), Error> {
    unsafe {
        write_volatile(
            &raw mut (CTRL_HANDLER),
            Some(CtrlHandlerHolder {
                handler: UnsafeCell::new(handler as *mut dyn CtrlHandler),
            }),
        );
    }
    compiler_fence(Ordering::SeqCst);

    // Log the result of SetConsoleCtrlHandler
    let result = unsafe { SetConsoleCtrlHandler(Some(ctrl_handler), true) };
    match result {
        Ok(()) => {
            log::info!("SetConsoleCtrlHandler succeeded");
            Ok(())
        }
        Err(err) => {
            log::info!("SetConsoleCtrlHandler failed");
            Err(Error::from(err))
        }
    }
}

unsafe extern "system" fn ctrl_handler(ctrl_type: u32) -> BOOL {
    let handler = unsafe { ptr::read_volatile(&raw const (CTRL_HANDLER)) };
    match handler {
        Some(handler_holder) => {
            log::info!("{:?}: Handling ctrl {}", std::process::id(), ctrl_type);
            let handler = unsafe { &mut *handler_holder.handler.get() };
            if let Some(ctrl_handler) = unsafe { handler.as_mut() } {
                (*ctrl_handler).handle(ctrl_type).into()
            } else {
                false.into()
            }
        }
        None => false.into(),
    }
}
