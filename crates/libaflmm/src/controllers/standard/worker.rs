use alloc::rc::Rc;
use core::mem;
use std::collections::HashSet;

use libaflmm_core::{Result, illegal_argument};
use nix::unistd::{dup2_stderr, dup2_stdout};

use crate::{
    controllers::{MessagingWorker, SharingWorker, StdDescriptor, Workdir, Worker, WorkerId},
    corpus::{Testcase, TestcaseId},
    inputs::Input,
    sync::{GenericCommand, GenericNotification, InputHandleBackend, WorkerSync},
};

/// The standard [`Worker`].
#[derive(Debug)]
pub struct GenericWorker<HB, I, U, WS>
where
    HB: InputHandleBackend<I>,
{
    descriptor: StdDescriptor,
    handle_backend: HB,
    worker_sync: WS,

    // testcases that have been either sent or received
    // it's to avoid loops in case the fuzzer is poorly configured
    // (like if feedback is always true)
    seen_testcases: HashSet<TestcaseId>,

    // buffers, filled in by `poll`
    pending_imports: Vec<(TestcaseId, HB::Handle)>,
    pending_customs: Vec<U>,
    shutdown_requested: bool,

    should_report: bool,
}

/// A representation of a [`StdWorker`](crate::controllers::StdWorker), to be used by [`StdController`](crate::controllers::StdController).
#[derive(Debug)]
pub struct StdWorkerRepr<CS> {
    descriptor: StdDescriptor,
    sync: CS,
}

impl<CS> StdWorkerRepr<CS> {
    pub fn new(descriptor: StdDescriptor, sync: CS) -> Self {
        Self { descriptor, sync }
    }

    pub fn descriptor(&self) -> &StdDescriptor {
        &self.descriptor
    }

    pub fn descriptor_mut(&mut self) -> &mut StdDescriptor {
        &mut self.descriptor
    }

    pub fn sync(&self) -> &CS {
        &self.sync
    }

    pub fn sync_mut(&mut self) -> &mut CS {
        &mut self.sync
    }
}

impl<HB, I, U, WS> Worker for GenericWorker<HB, I, U, WS>
where
    HB: InputHandleBackend<I>,
    WS: WorkerSync<GenericCommand<HB::Handle, U>, GenericNotification<HB::Handle, U>>,
{
    type Descriptor = StdDescriptor;

    fn id(&self) -> WorkerId {
        self.descriptor.worker_id
    }

    fn descriptor(&self) -> &StdDescriptor {
        &self.descriptor
    }

    fn descriptor_mut(&mut self) -> &mut StdDescriptor {
        &mut self.descriptor
    }

    fn workdir(&self) -> &Workdir {
        &self.descriptor.workdir
    }

    fn workdir_mut(&mut self) -> &mut Workdir {
        &mut self.descriptor.workdir
    }

    fn pre_runtime_exec(&mut self) -> Result<()> {
        dup2_stdout(self.descriptor.workdir.stdout()?)?;
        dup2_stderr(self.descriptor.workdir.stderr()?)?;

        Ok(())
    }

    fn poll(&mut self) -> Result<bool> {
        let mut received = false;

        for command in self.worker_sync.poll()? {
            received = true;

            match command {
                GenericCommand::Shutdown => self.shutdown_requested = true,
                GenericCommand::Import { id, handle, .. } => {
                    self.pending_imports.push((id, handle));
                }
                GenericCommand::Custom(payload) => self.pending_customs.push(payload),
            }
        }

        Ok(received)
    }

    fn should_shutdown(&mut self) -> bool {
        self.shutdown_requested
    }
}

impl<HB, I, U, WS> SharingWorker<I> for GenericWorker<HB, I, U, WS>
where
    I: Input,
    HB: InputHandleBackend<I>,
    WS: WorkerSync<GenericCommand<HB::Handle, U>, GenericNotification<HB::Handle, U>>,
{
    fn send_testcase(&mut self, testcase: &Testcase<I>) -> Result<()> {
        // no destination to report to, skip
        if !self.should_report {
            return Ok(());
        }

        // mark a testcase as seen
        self.seen_testcases.insert(*testcase.id());
        log::debug!("worker {:?} sends testcase {:?}", self.id(), testcase.id());

        let handle = self.handle_backend.create_handle(&testcase.input())?;
        self.worker_sync.send(GenericNotification::NewTestcase {
            id: *testcase.id(),
            handle,
        })
    }

    fn recv_testcases(&mut self) -> Result<impl Iterator<Item = Testcase<I>>> {
        let worker_id = self.descriptor.worker_id;

        let testcases: Vec<_> = self
            .pending_imports
            .drain(..)
            .filter(|(id, _)| self.seen_testcases.insert(*id))
            .map(|(id, handle)| {
                log::debug!("worker {worker_id:?} imports testcase {id:?}");

                let testcase = Testcase::new(Rc::new(self.handle_backend.resolve_handle(handle)?));

                if *testcase.id() != id {
                    return Err(illegal_argument!(
                        "imported ID does not match input content"
                    ));
                }

                Ok(testcase)
            })
            .collect::<Result<_>>()?;

        Ok(testcases.into_iter())
    }
}

impl<HB, I, U, WS> MessagingWorker<U> for GenericWorker<HB, I, U, WS>
where
    HB: InputHandleBackend<I>,
    WS: WorkerSync<GenericCommand<HB::Handle, U>, GenericNotification<HB::Handle, U>>,
{
    fn send_custom(&mut self, payload: U) -> Result<()> {
        self.worker_sync.send(GenericNotification::Custom(payload))
    }

    fn recv_custom(&mut self) -> Result<impl Iterator<Item = U>> {
        Ok(mem::take(&mut self.pending_customs).into_iter())
    }
}

impl<HB, I, U, WS> GenericWorker<HB, I, U, WS>
where
    HB: InputHandleBackend<I>,
{
    /// Get a new [`StdWorker`](crate::controllers::StdWorker).
    #[must_use]
    pub fn new(
        descriptor: StdDescriptor,
        handle_backend: HB,
        worker_sync: WS,
        should_report: bool,
    ) -> Self {
        Self {
            descriptor,
            handle_backend,
            worker_sync,
            should_report,
            seen_testcases: HashSet::new(),
            pending_imports: Vec::new(),
            pending_customs: Vec::new(),
            shutdown_requested: false,
        }
    }
}
