use std::{thread, time::Duration};

use libafl_core::Error;
use libc::{SIGALRM, SIGSEGV};
use rusty_fork::{rusty_fork_id, rusty_fork_test};

use crate::{
    inputs::NopInput,
    nop::NopWorker,
    runtimes::{
        Runtime, RuntimeHandle, TerminationHandlerData, inprocess::InProcessRuntime,
        restarting::LIBAFL_EXIT_CONTINUE, utils::OsTerminationParams,
    },
    states::NopState,
};

use libafl_bolts::timers::FastTimer;

rusty_fork_test! {
    #[test]
    fn test_runtime_create() {
        let mut state = NopState::<NopInput>::new();
        let worker = NopWorker;

        let task = |_rt_handle: &mut RuntimeHandle<NopState<NopInput>, NopWorker>, _state: &mut NopState<NopInput>| {
            Err(Error::shutting_down())
        };

        let crash_handler = |_data: &mut TerminationHandlerData, _params: &OsTerminationParams| Ok(());

        let timeout_handler = |_data: &mut TerminationHandlerData, _params: &OsTerminationParams| Ok(());

        let fast_timer = FastTimer::new();

        let mut runtime = InProcessRuntime::new(task, crash_handler, TerminationHandlerData::new(), timeout_handler, fast_timer);

        match runtime.run(state, worker).err() {
            Some(Error::ShuttingDown) => {}
            _ => {
                panic!("Task did not run successfully");
            }
        }
    }
}

/// set_input is important:
/// - if it is set, the runner considers we are in fuzzing loop, so crash / timeout is expected
/// - if it is not set, the runner considers we are out of the fuzzing loop, so crash / timeout is unexpected (fuzzer bug)
fn run_runtime<CH, T, TH>(task: T, crash_handler: CH, timeout_handler: TH, set_input: bool)
where
    T: FnMut(
            &mut RuntimeHandle<NopState<NopInput>, NopWorker>,
            &mut NopState<NopInput>,
        ) -> Result<(), Error>
        + 'static,
    for<'a> CH: FnMut(&mut TerminationHandlerData, &OsTerminationParams<'a>) -> Result<(), Error>
        + Send
        + Sync
        + Unpin
        + 'static,
    for<'a> TH: FnMut(&mut TerminationHandlerData, &OsTerminationParams<'a>) -> Result<(), Error>
        + Send
        + Sync
        + Unpin
        + 'static,
{
    let state = NopState::<NopInput>::new();
    let mut worker = NopWorker;
    
    let fast_timer = FastTimer::new();

    let mut runtime = InProcessRuntime::new(
        task,
        crash_handler,
        TerminationHandlerData::new(),
        timeout_handler,
        fast_timer
    );

    if set_input {
        runtime
            .termination_handler
            .inner_mut()
            .termination_data_mut()
            .set_input(&NopInput);
    }

    runtime.run(state, worker).unwrap();
}

#[test]
fn test_runtime_timeout() {
    // The timeout handler calls exit(55), so we use rusty_fork::fork
    // directly to check the child's exit code.
    let status = rusty_fork::fork(
        "runtimes::inprocess::tests::test_runtime_timeout",
        rusty_fork_id!(),
        |_| (),
        |child, _| child.wait().unwrap(),
        || {
            let task = |rt_handle: &mut RuntimeHandle<NopState<NopInput>, NopWorker>,
                        _state: &mut NopState<NopInput>| {
                rt_handle.set_timeout(Duration::from_millis(10));

                rt_handle.arm_timeout();
                thread::sleep(Duration::from_millis(50));

                panic!("Did not timeout!");

                #[allow(unreachable_code)]
                Ok::<(), Error>(())
            };

            let crash_handler =
                |_data: &mut TerminationHandlerData, _params: &OsTerminationParams| Ok(());

            let timeout_handler =
                |_data: &mut TerminationHandlerData, _params: &OsTerminationParams| Ok(());

            run_runtime(task, crash_handler, timeout_handler, false)
        },
    )
    .unwrap();

    assert_eq!(
        status.code(),
        Some(128 + SIGALRM),
        "Expected child to exit with code 128 + SIGALRM (timeout handler), got {:?}",
        status.code()
    );
}

#[test]
fn test_runtime_crash() {
    let status = rusty_fork::fork(
        "runtimes::inprocess::tests::test_runtime_crash",
        rusty_fork_id!(),
        |_| (),
        |child, _| child.wait().unwrap(),
        || {
            let task = |_rt_handle: &mut RuntimeHandle<NopState<NopInput>, NopWorker>,
                        _state: &mut NopState<NopInput>| {
                unsafe {
                    libc::raise(libc::SIGSEGV);
                }

                panic!("Did not crash!");

                #[allow(unreachable_code)]
                Ok::<(), Error>(())
            };

            let crash_handler =
                |_data: &mut TerminationHandlerData, _params: &OsTerminationParams| Ok(());

            let timeout_handler =
                |_data: &mut TerminationHandlerData, _params: &OsTerminationParams| Ok(());

            run_runtime(task, crash_handler, timeout_handler, false)
        },
    )
    .unwrap();

    assert_eq!(
        status.code(),
        Some(128 + libc::SIGSEGV),
        "Expected child to exit with code 128 + SIGSEGV (crash handler), got {:?}",
        status.code()
    );
}

#[test]
fn test_runtime_timeout_handler() {
    // The timeout handler calls exit(55), so we use rusty_fork::fork
    // directly to check the child's exit code.
    let status = rusty_fork::fork(
        "runtimes::inprocess::tests::test_runtime_timeout_handler",
        rusty_fork_id!(),
        |_| (),
        |child, _| child.wait().unwrap(),
        || {
            let task = |rt_handle: &mut RuntimeHandle<NopState<NopInput>, NopWorker>,
                        _state: &mut NopState<NopInput>| {
                rt_handle.set_timeout(Duration::from_millis(10));

                rt_handle.arm_timeout()?;

                thread::sleep(Duration::from_millis(50));

                panic!("Did not timeout!");

                #[allow(unreachable_code)]
                Ok::<(), Error>(())
            };

            let crash_handler =
                |_data: &mut TerminationHandlerData, _params: &OsTerminationParams| Ok(());

            let timeout_handler =
                |_data: &mut TerminationHandlerData, _params: &OsTerminationParams| unsafe {
                    libc::exit(114);
                };

            run_runtime(task, crash_handler, timeout_handler, true)
        },
    )
    .unwrap();

    assert_eq!(
        status.code(),
        Some(114),
        "Expected child to exit with code 114 (timeout handler), got {:?}",
        status.code()
    );
}

#[test]
fn test_runtime_crash_handler() {
    let status = rusty_fork::fork(
        "runtimes::inprocess::tests::test_runtime_crash_handler",
        rusty_fork_id!(),
        |_| (),
        |child, _| child.wait().unwrap(),
        || {
            let task = |_rt_handle: &mut RuntimeHandle<NopState<NopInput>, NopWorker>,
                        _state: &mut NopState<NopInput>| {
                unsafe {
                    libc::raise(libc::SIGSEGV);
                }

                panic!("Did not crash!");

                #[allow(unreachable_code)]
                Ok::<(), Error>(())
            };

            let crash_handler = |_data: &mut TerminationHandlerData,
                                 _params: &OsTerminationParams| unsafe {
                libc::exit(114);
            };

            let timeout_handler =
                |_data: &mut TerminationHandlerData, _params: &OsTerminationParams| Ok(());

            run_runtime(task, crash_handler, timeout_handler, true)
        },
    )
    .unwrap();

    assert_eq!(
        status.code(),
        Some(114),
        "Expected child to exit with code 114 (crash handler), got {:?}",
        status.code()
    );
}
