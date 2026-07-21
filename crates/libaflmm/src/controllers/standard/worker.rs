use crate::{
    controllers::{StdDescriptor, SyncWorker, Workdir, Worker},
    corpus::{Testcase, TestcaseId},
    inputs::Input,
    sync::{
        InputRepr, WorkerSync,
        exchanges::standard::{StdCommand, StdNotification},
    },
};
use libaflmm_core::{Result, WorkerId, illegal_argument};
use nix::unistd::{dup2_stderr, dup2_stdout};
use std::{collections::HashSet, rc::Rc};

/// The standard [`Worker`].
#[derive(Debug)]
pub struct StdWorker<I, IR, WS>
where
    IR: InputRepr<I>,
{
    descriptor: StdDescriptor,
    input_repr: IR,
    sync: WS,
    imported_testcases: HashSet<TestcaseId>,
    pending_commands: Vec<StdCommand<IR::InputHandle>>,
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

impl<I, IR, WS> Worker for StdWorker<I, IR, WS>
where
    IR: InputRepr<I>,
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

impl<I, IR, WS> SyncWorker<I> for StdWorker<I, IR, WS>
where
    I: Input,
    IR: InputRepr<I>,
    WS: WorkerSync<StdNotification<IR::InputHandle>, StdCommand<IR::InputHandle>>,
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

        let handle = self.input_repr.create_handle(&testcase.input())?;

        self.sync.send(StdNotification::NewTestcase {
            id: *testcase.id(),
            handle,
        })
    }

    fn recv_testcases(&mut self) -> Result<impl Iterator<Item = Testcase<I>>> {
        self.pending_commands.extend(self.sync.poll()?);

        for cmd in self
            .pending_commands
            .extract_if(.., |c| matches!(c, StdCommand::Import { .. }))
        {
            if let StdCommand::Import { id, handle, .. } = cmd {
                if !self.imported_testcases.contains(&id) {
                    let tc = if let Some(input) = self.input_repr.handle_to_input(handle)? {
                        Testcase::new(Rc::new(input))
                    };
                    let tc = ;
                    if *tc.id() != id {
                        return Err(illegal_argument!(
                            "imported ID does not match input content"
                        ));
                    }
                    self.imported_testcases.insert(id);
                    self.pending_imports.push(tc);
                }
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

impl<I, IR, WS> StdWorker<I, IR, WS>
where
    IR: InputRepr<I>,
{
    /// Get a new [`StdWorker`].
    #[must_use]
    pub fn new(
        descriptor: StdDescriptor,
        input_repr: IR,
        sync: WS,
        should_report: bool,
    ) -> Result<Self> {
        // let (ctrl_conn, worker_conn) = StdControllerConnection::create()?;

        Ok(Self {
            descriptor: descriptor.clone(),
            input_repr,
            sync,
            should_report,
            imported_testcases: HashSet::new(),
            pending_commands: Vec::new(),
            pending_imports: Vec::new(),
        })
    }
}
