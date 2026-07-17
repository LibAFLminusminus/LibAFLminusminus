use crate::{
    controllers::{
        StdCommand, StdDescriptor, SyncWorker, Workdir, Worker,
        standard::{StdControllerConnection, StdWorkerConnection},
    },
    corpus::{Testcase, TestcaseId},
    sync::Synchronizer,
};
use libaflmm_core::{Result, WorkerId};
use nix::unistd::{dup2_stderr, dup2_stdout};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, marker::PhantomData};

/// The standard [`Worker`].
#[derive(Debug)]
pub struct StdWorker<I, SY> {
    descriptor: StdDescriptor,
    synchronizer: SY,
    connection: StdWorkerConnection,
    imported_testcases: HashSet<TestcaseId>,
    pending_commands: Vec<StdCommand>,
    pending_imports: Vec<Testcase<I>>,
    should_report: bool,
    phantom: PhantomData<I>,
}

/// A representation of a [`StdWorker`], to be used by [`StdController`].
#[derive(Debug)]
pub struct StdWorkerRepr {
    descriptor: StdDescriptor,
    connection: StdControllerConnection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StdNotification {
    NewTestcase { id: TestcaseId, serialized: Vec<u8> },
}

impl StdWorkerRepr {
    pub fn new(descriptor: StdDescriptor, connection: StdControllerConnection) -> Self {
        Self {
            descriptor,
            connection,
        }
    }

    pub fn descriptor(&self) -> &StdDescriptor {
        &self.descriptor
    }

    pub fn descriptor_mut(&mut self) -> &mut StdDescriptor {
        &mut self.descriptor
    }

    pub fn connection(&self) -> &StdControllerConnection {
        &self.connection
    }

    pub fn connection_mut(&mut self) -> &mut StdControllerConnection {
        &mut self.connection
    }
}

impl<I, SY> Worker for StdWorker<I, SY>
where
    SY: Synchronizer<I>,
{
    type Descriptor = StdDescriptor;

    type Command = StdCommand;
    type Notification = StdNotification;

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
        if let Some(f) = self.descriptor.workdir.stdout()? {
            dup2_stdout(f)?;
        }

        if let Some(f) = self.descriptor.workdir.stderr()? {
            dup2_stderr(f)?;
        }

        Ok(())
    }

    fn poll_commands_filtered(
        &mut self,
        mut filter: impl FnMut(&Self::Command) -> bool,
    ) -> Result<impl Iterator<Item = Self::Command>> {
        // collect pending commands
        self.pending_commands.extend(self.connection.poll()?);

        Ok(self.pending_commands.extract_if(.., move |elt| filter(elt)))
    }

    fn send_notification(&mut self, notification: Self::Notification) -> Result<()> {
        if !self.connection.send(&notification)? {
            log::warn!(
                "Notification could not be sent, most likely because there is some congestion. Falling back to blocking send..."
            );
            self.connection.send_blocking(&notification)?;
        }

        Ok(())
    }
}

impl<I, SY> SyncWorker<I> for StdWorker<I, SY>
where
    SY: Synchronizer<I>,
{
    fn report_testcase(&mut self, testcase: &Testcase<I>) -> Result<()> {
        if let Some(repr) = self.synchronizer.export(testcase)? {
            let serialized = postcard::to_allocvec(&repr)?;
            self.send_notification(StdNotification::NewTestcase {
                id: *testcase.id(),
                serialized,
            })?;
        }

        Ok(())
    }

    fn sync_pending_inputs(&mut self) -> Result<impl Iterator<Item = Testcase<I>>> {
        let imports =
            self.poll_commands_filtered(|cmd| matches!(cmd, StdCommand::Import { .. }))?;

        // import inputs from the representatives
        for import in imports {
            match import {
                StdCommand::Import {
                    id,
                    serialized,
                    source,
                } => {
                    if self.imported_testcases.insert(id) {
                        let repr = postcard::from_bytes(&serialized)?;
                        self.synchronizer.import(source, id, repr)?;
                    }
                }
                _ => unreachable!(),
            }
        }

        // drain all pending inputs buffered in the synchronizer
        for tc in self.synchronizer.drain()? {
            if self.imported_testcases.insert(*tc.id()) {
                self.pending_imports.push(tc);
            }
        }

        Ok(self.pending_imports.drain(..))
    }
}

impl<I, SY> StdWorker<I, SY>
where
    SY: Synchronizer<I>,
{
    /// Get a new [`StdWorker`].
    #[must_use]
    pub fn new(
        descriptor: StdDescriptor,
        synchronizer: SY,
        should_report: bool,
    ) -> Result<(Self, StdWorkerRepr)> {
        let (ctrl_conn, worker_conn) = StdControllerConnection::create()?;

        Ok((
            Self {
                descriptor: descriptor.clone(),
                synchronizer,
                connection: worker_conn,
                should_report,
                imported_testcases: HashSet::new(),
                pending_commands: Vec::new(),
                pending_imports: Vec::new(),
                phantom: PhantomData,
            },
            StdWorkerRepr {
                descriptor: descriptor,
                connection: ctrl_conn,
            },
        ))
    }
}
