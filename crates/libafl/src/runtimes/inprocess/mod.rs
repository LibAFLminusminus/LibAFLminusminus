use core::{marker::PhantomData, pin::Pin, time::Duration};
use std::boxed::Box;

use libafl_bolts::TimerStruct;
use libafl_core::Error;

use crate::{
    DependencyResolver,
    runtimes::{Runtime, RuntimeHandle},
};

pub mod unix;
pub use unix::OsSignalHandler;

impl<CH, D, S, T, TH> DependencyResolver for InProcessRuntime<CH, D, S, T, TH> {}

/// Hooks the current process to set it up for in-process tasks.
/// It will change signal handlers and "pollute" the current process.
/// It is advised to combine it with the [`RestartingRuntime`], responsible
/// for forking and and state preservation.
///
/// InProcessRuntime runs a task that does NOT return.
/// To exit, simply exit the process.
/// There are special exit codes used to convey what caused the exit.
/// TODO: document these exit code
pub struct InProcessRuntime<CH, D, S, T, TH> {
    state: S,
    task: T,
    signal_handler: Pin<Box<OsSignalHandler<CH, D, TH>>>,
    timer: Option<TimerStruct>,
}

pub struct InProcessSignalHandler<CH, D, TH> {
    signal_handler_depth: usize,
    signal_handler_max_depth: usize,
    crash_handler: CH,
    timeout_handler: TH,
    signal_data: D,
    in_target: bool,
}

unsafe impl<CH, D, TH> Send for InProcessSignalHandler<CH, D, TH>
where
    CH: Send,
    D: Send,
    TH: Send,
{
}

unsafe impl<CH, D, TH> Sync for InProcessSignalHandler<CH, D, TH>
where
    CH: Sync,
    D: Sync,
    TH: Sync,
{
}

impl<CH, D, S, T, TH> InProcessRuntime<CH, D, S, T, TH>
where
    CH: FnMut(&mut D) -> Result<(), Error> + Send + Sync + Unpin + 'static,
    D: Send + Sync + Unpin + 'static,
    TH: FnMut(&mut D) -> Result<(), Error> + Send + Sync + Unpin + 'static,
{
    pub fn new(state: S, task: T, crash_handler: CH, signal_data: D, timeout_handler: TH) -> Self {
        let signal_handler =
            InProcessSignalHandler::new(crash_handler, signal_data, timeout_handler);

        InProcessRuntime {
            state,
            task,
            signal_handler: Box::pin(OsSignalHandler::new(signal_handler)),
            timer: None,
        }
    }
}

impl<CH, D, TH> InProcessSignalHandler<CH, D, TH>
where
    CH: FnMut(&mut D) -> Result<(), Error>,
    TH: FnMut(&mut D) -> Result<(), Error>,
{
    pub fn new(crash_handler: CH, signal_data: D, timeout_handler: TH) -> Self {
        Self {
            crash_handler,
            timeout_handler,
            signal_handler_depth: 0,
            signal_handler_max_depth: 3,
            signal_data,
            in_target: false,
        }
    }

    pub fn enter(&mut self) -> bool {
        self.signal_handler_depth += 1;

        self.signal_handler_depth >= self.signal_handler_max_depth
    }

    pub fn exit(&mut self) {
        self.signal_handler_depth -= 1;
    }

    // pub fn handle_timeout(&mut self) {
    //     (self.timeout_handler)(&mut self.signal_data)
    // }

    // pub fn handle_crash(&mut self) {
    //     (self.crash_handler)(&mut self.signal_data)
    // }

    pub fn max_depth(&self) -> usize {
        self.signal_handler_max_depth
    }

    pub fn enter_target(&mut self) {
        self.in_target = true;
    }

    pub fn exit_target(&mut self) {
        self.in_target = false;
    }

    pub fn is_in_target(&self) -> bool {
        self.in_target
    }
}

impl<CT, CH, D, S, T, TH> Runtime<CT, S> for InProcessRuntime<CH, D, S, T, TH>
where
    T: FnMut(&mut RuntimeHandle<CT, S>, &mut S) -> Result<(), Error>,
    CH: FnMut(&mut D) -> Result<(), Error> + Send + Sync + Unpin + 'static,
    D: Send + Sync + Unpin + 'static,
    TH: FnMut(&mut D) -> Result<(), Error> + Send + Sync + Unpin + 'static,
{
    // TODO: handle signals
    unsafe fn run_impl(&mut self, rt_handle: &mut RuntimeHandle<CT, S>) -> Result<(), Error> {
        self.signal_handler.init();

        (self.task)(rt_handle, &mut self.state)
    }

    fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        let timer = TimerStruct::new(timeout);
        self.timer = Some(timer);

        Ok(())
    }

    fn arm_timeout(&mut self) -> Result<(), Error> {
        if let Some(timer) = &mut self.timer {
            timer.set_timer();
        }

        Ok(())
    }

    fn disarm_timeout(&mut self) -> Result<(), Error> {
        if let Some(timer) = &mut self.timer {
            timer.unset_timer();
        }

        Ok(())
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        let mut timer = self.timer.take().expect("Could not get timer");

        timer.unset_timer();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use libafl_core::Error;
    use rusty_fork::{rusty_fork_id, rusty_fork_test};

    use crate::{
        inputs::NopInput,
        nop::NopController,
        runtimes::{Runtime, RuntimeHandle, inprocess::InProcessRuntime},
        state::NopState,
    };

    rusty_fork_test! {
        #[test]
        fn test_runtime_create() {
            let mut state = NopState::<NopInput>::new();
            let mut controller = NopController;

            let task = |_rt_handle: &mut RuntimeHandle<NopController, NopState<NopInput>>, _state: &mut NopState<NopInput>, _controller: &mut NopController| {
                Err(Error::shutting_down())
            };

            let crash_handler = |data: &mut ()| Ok(());

            let timeout_handler = |data: &mut ()| Ok(());

            let mut runtime = InProcessRuntime::new(state, task, crash_handler, (), timeout_handler);

            match runtime.run(&mut controller).err() {
                Some(Error::ShuttingDown) => {}
                _ => {
                    panic!("Task did not run successfully");
                }
            }
        }
    }

    fn run_runtime<CH, T, TH>(task: T, crash_handler: CH, timeout_handler: TH)
    where
        T: FnMut(
                &mut RuntimeHandle<NopController, NopState<NopInput>>,
                &mut NopState<NopInput>,
                &mut NopController,
            ) -> Result<(), Error>
            + 'static,
        CH: FnMut(&mut ()) -> Result<(), Error> + Send + Sync + Unpin + 'static,
        TH: FnMut(&mut ()) -> Result<(), Error> + Send + Sync + Unpin + 'static,
    {
        let state = NopState::<NopInput>::new();
        let mut controller = NopController;

        let mut runtime = InProcessRuntime::new(state, task, crash_handler, (), timeout_handler);
        runtime.signal_handler.inner_mut().enter_target();

        runtime.run(&mut controller).unwrap();
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
                let task = |rt_handle: &mut RuntimeHandle<NopController, NopState<NopInput>>,
                            _state: &mut NopState<NopInput>,
                            _controller: &mut NopController| {
                    rt_handle.set_timeout(Duration::from_millis(10));

                    rt_handle.arm_timeout();
                    thread::sleep(Duration::from_millis(50));

                    panic!("Did not timeout!");

                    #[allow(unreachable_code)]
                    Ok::<(), Error>(())
                };

                let crash_handler = |_data: &mut ()| Ok(());

                let timeout_handler = |_data: &mut ()| Ok(());

                run_runtime(task, crash_handler, timeout_handler)
            },
        )
        .unwrap();

        assert_eq!(
            status.code(),
            Some(55),
            "Expected child to exit with code 55 (timeout handler), got {:?}",
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
                let task = |_rt_handle: &mut RuntimeHandle<NopController, NopState<NopInput>>,
                            _state: &mut NopState<NopInput>,
                            _controller: &mut NopController| {
                    unsafe {
                        libc::raise(libc::SIGSEGV);
                    }

                    panic!("Did not crash!");

                    #[allow(unreachable_code)]
                    Ok::<(), Error>(())
                };

                let crash_handler = |_data: &mut ()| Ok(());

                let timeout_handler = |_data: &mut ()| Ok(());

                run_runtime(task, crash_handler, timeout_handler)
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
                let task = |rt_handle: &mut RuntimeHandle<NopController, NopState<NopInput>>,
                            _state: &mut NopState<NopInput>,
                            _controller: &mut NopController| {
                    rt_handle.set_timeout(Duration::from_millis(10));

                    rt_handle.arm_timeout()?;

                    thread::sleep(Duration::from_millis(50));

                    panic!("Did not timeout!");

                    #[allow(unreachable_code)]
                    Ok::<(), Error>(())
                };

                let crash_handler = |_data: &mut ()| Ok(());

                let timeout_handler = |_data: &mut ()| unsafe {
                    libc::exit(114);
                };

                run_runtime(task, crash_handler, timeout_handler)
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
                let task = |_rt_handle: &mut RuntimeHandle<NopController, NopState<NopInput>>,
                            _state: &mut NopState<NopInput>,
                            _controller: &mut NopController| {
                    unsafe {
                        libc::raise(libc::SIGSEGV);
                    }

                    panic!("Did not crash!");

                    #[allow(unreachable_code)]
                    Ok::<(), Error>(())
                };

                let crash_handler = |_data: &mut ()| unsafe {
                    libc::exit(114);
                };

                let timeout_handler = |_data: &mut ()| Ok(());

                run_runtime(task, crash_handler, timeout_handler)
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
}
