use crate::{
    controllers::{
        Controller, Descriptor, StdDescriptor, StdWorker, WorkdirFile,
        standard::builder::StdControllerBuilder,
    },
    launchers::InstanceId,
    sync::{
        ControllerSync, Exchange, GroupId, HandleProvider, Orchestrator, Router, StdCommand,
        StdNotification, StdOrchestrator, Transport,
        transports::{HandleProviderFactory, WaitResult},
    },
};
use core::{fmt::Debug, marker::PhantomData, mem, time::Duration};
use libaflmm_bolts::{CoreId, Cores};
use libaflmm_core::{Result, WorkerId, illegal_argument, illegal_state, internal_bug};
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fs,
    os::fd::BorrowedFd,
    path::{Path, PathBuf},
};

// get the synchronizer type out of a pair of <Input, Orchestrator>
pub(crate) type TransportOf<I, O> = <O as Orchestrator<StdDescriptor, I>>::Transport;
pub(crate) type InputReprOf<I, O> = <O as Orchestrator<StdDescriptor, I>>::Provider;
pub(crate) type HandleOf<I, O> = <InputReprOf<I, O> as HandleProvider<I>>::Handle;
pub(crate) type CommandOf<I, O> = <O as Orchestrator<StdDescriptor, I>>::Command;
pub(crate) type NotificationOf<I, O> = <O as Orchestrator<StdDescriptor, I>>::Notification;

pub(crate) type ControllerSyncOf<I, O> = <TransportOf<I, O> as Transport<
    CommandOf<I, O>,
    StdDescriptor,
    NotificationOf<I, O>,
>>::ControllerSync;
// pub(crate) type WorkerSyncOf<I, O> = <TransportOf<I, O> as Transport<
//     CommandOf<I, O>,
//     StdDescriptor,
//     NotificationOf<I, O>,
// >>::WorkerSync;

/// The standard controller.
#[derive(Debug)]
#[expect(clippy::type_complexity)]
pub struct StdController<I, O>
where
    I: Debug,
    O: Orchestrator<
            StdDescriptor,
            I,
            Command = StdCommand<HandleOf<I, O>>,
            Notification = StdNotification<HandleOf<I, O>>,
        >,
{
    orchestrator: O,
    controller_sync: Option<ControllerSyncOf<I, O>>,
    descriptors: HashMap<WorkerId, StdDescriptor>,
    root_dir: PathBuf,
    finalized: bool,

    worker_id_ctr: u32,

    // buffers
    pending_workers: HashMap<GroupId, Vec<StdWorker<O::Provider, I, O::WorkerSync>>>,
    pending_groups: HashMap<GroupId, Cores>,
    pending_notifications: Vec<(StdNotification<HandleOf<I, O>>, WorkerId)>,

    worker_stdout: WorkdirFile,
    worker_stderr: WorkdirFile,
    worker_stats: WorkdirFile,

    phantom: PhantomData<I>,
}

impl<I, O> Controller for StdController<I, O>
where
    I: Debug,
    O: Orchestrator<
            StdDescriptor,
            I,
            Command = StdCommand<HandleOf<I, O>>,
            Notification = StdNotification<HandleOf<I, O>>,
        >,
{
    type Worker = StdWorker<O::Provider, I, O::WorkerSync>;
    type GroupConfig = <O::Router as Router<CommandOf<I, O>, StdDescriptor>>::GroupConfig;

    fn register_group(&mut self, config: Self::GroupConfig, cores: &Cores) -> Result<GroupId> {
        let group_id = self.orchestrator.router_mut().register_group(config)?;
        self.pending_groups.insert(group_id, cores.clone());
        Ok(group_id)
    }

    fn finalize_orchestration(&mut self) -> Result<()> {
        if mem::replace(&mut self.finalized, true) {
            return Err(illegal_state!(
                "Trying to finalize a controller more than one time. This is not legal."
            ));
        }

        let mut used_cores: HashSet<CoreId> = HashSet::new();
        let mut worker_desc: HashMap<WorkerId, StdDescriptor> = HashMap::new();

        // check core pinning correctness
        for cores in self.pending_groups.values() {
            for core_id in cores {
                if let Some(core) = core_id
                    && !used_cores.insert(core)
                {
                    return Err(illegal_argument!(
                        "core {core:?} is getting pinned on by multiple workers. Use unpinned cores instead."
                    ));
                }
            }
        }

        // declare each worker, with the corresponding descriptors.
        for (group_id, cores) in mem::take(&mut self.pending_groups) {
            for core_id in &cores {
                let worker_id = WorkerId(self.worker_id_ctr);
                self.worker_id_ctr += 1;

                let desc = self.new_descriptor(worker_id, group_id, core_id)?;
                self.orchestrator.router_mut().register_worker(&desc)?;

                worker_desc.insert(worker_id, desc);
            }
        }

        self.orchestrator.router_mut().finalize()?;

        for (wid, desc) in &worker_desc {
            let source_wids: Vec<WorkerId> = self.orchestrator.router().sources(*wid).collect();

            let sources: Vec<&StdDescriptor> = source_wids
                .into_iter()
                .map(|src_wid| worker_desc.get(&src_wid).unwrap())
                .collect();

            let handle_provider = self
                .orchestrator
                .handle_provider_factory_mut()
                .create(desc, sources.iter().copied())?;

            let worker_sync = self
                .orchestrator
                .transport_mut()
                .create_worker_sync(desc, sources.iter().copied())?;

            let should_report = self.orchestrator.router().has_destinations(*wid);

            let worker = StdWorker::new(desc.clone(), handle_provider, worker_sync, should_report);

            match self.pending_workers.entry(desc.group_id()) {
                Entry::Occupied(mut entry) => entry.get_mut().push(worker),
                Entry::Vacant(entry) => {
                    entry.insert(vec![worker]);
                }
            }

            self.descriptors.insert(*wid, desc.clone());
        }

        self.orchestrator.handle_provider_factory_mut().finalize()?;
        self.controller_sync = Some(self.orchestrator.transport_mut().create_controller_sync()?);

        Ok(())
    }

    fn take_group_workers(&mut self, group: GroupId) -> Result<impl Iterator<Item = Self::Worker>> {
        Ok(self
            .pending_workers
            .remove(&group)
            .ok_or(illegal_argument!(
                "The group ID {group:?} has not been registered"
            ))?
            .into_iter())
    }

    fn worker_descriptors(&self) -> impl IntoIterator<Item = &StdDescriptor> {
        self.descriptors.values()
    }

    fn worker_descriptors_mut(&mut self) -> impl IntoIterator<Item = &mut StdDescriptor> {
        self.descriptors.values_mut()
    }

    fn on_worker_start(&mut self, descriptor: &StdDescriptor, _id: InstanceId) -> Result<()> {
        log::info!("Started worker {:?}", descriptor.worker_id());
        Ok(())
    }

    fn on_worker_termination(
        &mut self,
        descriptor: &StdDescriptor,
        _termination_code: nix::sys::signal::Signal,
    ) -> Result<()> {
        log::info!("Terminated worker {:?}", descriptor.worker_id);
        Ok(())
    }

    fn wait_notifications(&mut self, wake_fds: &[BorrowedFd<'_>], timeout: Duration) -> Result<()> {
        let sync = self.controller_sync.as_mut().unwrap();

        if matches!(sync.wait(wake_fds, timeout)?, WaitResult::Event) {
            // notification ready to be handled
            self.pending_notifications.extend(sync.poll()?);

            for (notification, source) in self.pending_notifications.drain(..) {
                let src_desc = self.descriptors.get(&source).unwrap();

                // self.on_notification(src_desc, &notification)?;

                let Some(command) = self
                    .orchestrator
                    .exchange_mut()
                    .notif_to_command(src_desc, notification)?
                else {
                    continue;
                };

                let destinations = self.orchestrator.router_mut().route(source, &command)?;
                self.controller_sync
                    .as_mut()
                    .unwrap()
                    .send(destinations, command)?;
            }
        }

        Ok(())
    }

    fn root_dir(&self) -> &Path {
        self.root_dir.as_path()
    }

    fn shutdown(&mut self, worker: WorkerId) -> Result<()> {
        self.controller_sync
            .as_mut()
            .unwrap()
            .send_to(worker, StdCommand::Shutdown)
    }
}

impl<I, O> StdController<I, O>
where
    I: Debug,
    O: Orchestrator<
            StdDescriptor,
            I,
            Command = StdCommand<HandleOf<I, O>>,
            Notification = StdNotification<HandleOf<I, O>>,
        >,
{
    fn new_descriptor(
        &self,
        worker_id: WorkerId,
        group_id: GroupId,
        core_id: Option<CoreId>,
    ) -> Result<StdDescriptor> {
        let worker_dir = self.root_dir.join(format!("worker_{}", worker_id.0));

        if worker_dir.exists() {
            return Err(internal_bug!(
                "The worker dir \"{}\" already exists.",
                worker_dir.display()
            ));
        }

        fs::create_dir(worker_dir.as_path())?;

        StdDescriptor::new(
            worker_dir,
            self.worker_stdout.clone(),
            self.worker_stderr.clone(),
            self.worker_stats.clone(),
            worker_id,
            core_id,
            group_id,
        )
    }
}

impl<I, O> StdController<I, O>
where
    I: Debug,
    O: Orchestrator<
            StdDescriptor,
            I,
            Command = StdCommand<HandleOf<I, O>>,
            Notification = StdNotification<HandleOf<I, O>>,
        >,
{
    /// Create a new [`StdController`] using `root_dir` as the root directory.
    /// If overwrite is true, the `root_dir` will be removed before being created again.
    pub fn new(
        orchestrator: O,
        root_dir: PathBuf,
        worker_stdout: WorkdirFile,
        worker_stderr: WorkdirFile,
        worker_stats: WorkdirFile,
        overwrite: bool,
    ) -> Result<Self> {
        if root_dir.exists() {
            if overwrite {
                fs::remove_dir_all(root_dir.as_path())?;
            } else {
                return Err(illegal_argument!(
                    "Wordir already exists: {}. Set `overwrite` to `true` if you want to overwrite.",
                    root_dir.display()
                ));
            }
        }

        fs::create_dir(root_dir.as_path())?;

        Ok(Self {
            orchestrator,
            root_dir,
            controller_sync: None,
            worker_stdout,
            worker_stderr,
            worker_stats,
            descriptors: HashMap::default(),
            pending_groups: HashMap::default(),
            pending_workers: HashMap::default(),
            pending_notifications: Vec::new(),
            worker_id_ctr: 0,
            finalized: false,
            phantom: PhantomData,
        })
    }

    /// Get a [`StdControllerBuilder`], to build a [`StdController`].
    #[must_use]
    pub fn builder() -> StdControllerBuilder<StdOrchestrator> {
        StdControllerBuilder::default()
    }
}
