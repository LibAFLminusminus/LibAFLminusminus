use crate::{
    controllers::{StdDescriptor, SyncWorker, Workdir, Worker},
    corpus::{Testcase, TestcaseId},
    inputs::Input,
    sync::{HandleProvider, StdCommand, StdNotification, WorkerSync},
};
use alloc::rc::Rc;
use libaflmm_core::{Result, WorkerId, illegal_argument};
use nix::unistd::{dup2_stderr, dup2_stdout};
use std::collections::HashSet;

/// The standard [`Worker`].
#[derive(Debug)]
pub struct StdWorker<HP, I, WS>
where
    HP: HandleProvider<I>,
{
    descriptor: StdDescriptor,
    handle_provider: HP,
    worker_sync: WS,
    imported_testcases: HashSet<TestcaseId>,
    pending_commands: Vec<StdCommand<HP::Handle>>,
    pending_imports: Vec<Testcase<I>>,
    should_report: bool,
}

/// A representation of a [`StdWorker`], to be used by [`StdController`].
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

impl<HP, I, WS> Worker for StdWorker<HP, I, WS>
where
    HP: HandleProvider<I>,
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

    fn reconcile(&self) -> Result<()> {
        // do nothing
        Ok(())
    }

    fn pre_runtime_exec(&mut self) -> Result<()> {
        dup2_stdout(self.descriptor.workdir.stdout()?)?;
        dup2_stderr(self.descriptor.workdir.stderr()?)?;

        Ok(())
    }

    // fn poll_commands_filtered(
    //     &mut self,
    //     mut filter: impl FnMut(&Self::Command) -> bool,
    // ) -> Result<impl Iterator<Item = Self::Command>> {
    //     // collect pending commands
    //     self.pending_commands.extend(self.connection.poll()?);

    //     Ok(self.pending_commands.extract_if(.., move |elt| filter(elt)))
    // }

    // fn send_notification(&mut self, notification: Self::Notification) -> Result<()> {
    //     if !self.connection.send(&notification)? {
    //         log::warn!(
    //             "Notification could not be sent, most likely because there is some congestion. Falling back to blocking send..."
    //         );
    //         self.connection.send_blocking(&notification)?;
    //     }

    //     Ok(())
    // }
}

impl<HP, I, WS> SyncWorker<I> for StdWorker<HP, I, WS>
where
    I: Input,
    HP: HandleProvider<I>,
    WS: WorkerSync<StdCommand<HP::Handle>, StdNotification<HP::Handle>>,
{
    fn send_testcase(&mut self, testcase: &Testcase<I>) -> Result<()> {
        // no destination to report to, skip
        if !self.should_report {
            return Ok(());
        }

        // no need to send a testcase already imported.
        if self.imported_testcases.contains(testcase.id()) {
            return Ok(());
        }

        let handle = self.handle_provider.create_handle(&testcase.input())?;

        self.worker_sync.send(StdNotification::NewTestcase {
            id: *testcase.id(),
            handle,
        })
    }

    fn recv_testcases(&mut self) -> Result<impl Iterator<Item = Testcase<I>>> {
        self.pending_commands.extend(self.worker_sync.poll()?);

        for cmd in self
            .pending_commands
            .extract_if(.., |c| matches!(c, StdCommand::Import { .. }))
        {
            if let StdCommand::Import { id, handle, .. } = cmd
                && !self.imported_testcases.contains(&id)
            {
                let input = self.handle_provider.resolve_handle(handle)?;
                let tc = Testcase::new(Rc::new(input));

                if *tc.id() != id {
                    return Err(illegal_argument!(
                        "imported ID does not match input content"
                    ));
                }
                self.imported_testcases.insert(id);
                self.pending_imports.push(tc);
            }
        }

        // // import inputs from the representatives
        // for import in imports {
        //     match import {
        //         StdCommand::Import {
        //             id,
        //             serialized,
        //             source,
        //         } => {
        //             if self.imported_testcases.insert(id) {
        //                 let repr = postcard::from_bytes(&serialized)?;
        //                 self.synchronizer.import(source, id, repr)?;
        //             }
        //         }
        //         _ => unreachable!(),
        //     }
        // }

        // // drain all pending inputs buffered in the synchronizer
        // for tc in self.synchronizer.drain()? {
        //     if self.imported_testcases.insert(*tc.id()) {
        //         self.pending_imports.push(tc);
        //     }
        // }

        // Ok(self.pending_imports.drain(..))
        Ok(self.pending_imports.drain(..))
    }
}

impl<HP, I, WS> StdWorker<HP, I, WS>
where
    HP: HandleProvider<I>,
{
    /// Get a new [`StdWorker`].
    #[must_use]
    pub fn new(
        descriptor: StdDescriptor,
        handle_provider: HP,
        worker_sync: WS,
        should_report: bool,
    ) -> Self {
        Self {
            descriptor,
            handle_provider,
            worker_sync,
            should_report,
            imported_testcases: HashSet::new(),
            pending_commands: Vec::new(),
            pending_imports: Vec::new(),
        }
    }
}
