use core::{fmt::Debug, mem, time::Duration};
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fs,
    os::fd::BorrowedFd,
    path::{self, Path, PathBuf},
};

use libaflmm_bolts::{CoreId, terminations::TerminationCode};
use libaflmm_core::{Result, illegal_argument, illegal_state};

use crate::{
    controllers::{
        Controller, Descriptor, GenericWorker, StdDescriptor, WorkdirFile, WorkerId,
        standard::builder::StdControllerBuilder,
    },
    launchers::{
        InstanceId,
        groups::{Group, WorkerLayout},
    },
    sync::{
        ControllerSync, GenericCommand, GenericNotification, GroupId, InputHandleBackend,
        Orchestrator, Router, StdInputHandleBackendFactory, StdOrchestrator, StdRouter,
        StdTransfer, Transfer,
        transfers::{InputHandleBackendFactory, WaitResult},
    },
};

pub(crate) type BackendOf<HBF, I> = <HBF as InputHandleBackendFactory<StdDescriptor, I>>::Backend;
pub(crate) type HandleOf<HBF, I> = <BackendOf<HBF, I> as InputHandleBackend<I>>::Handle;

/// The standard controller.
#[derive(Debug)]
#[expect(clippy::type_complexity)]
pub struct GenericController<HBF, I, R, T>
where
    HBF: InputHandleBackendFactory<StdDescriptor, I>,
    I: Debug,
    T: Transfer<StdDescriptor, HandleOf<HBF, I>>,
{
    orchestrator: Orchestrator<HBF, R, T>,
    controller_sync: Option<T::ControllerSync>,
    descriptors: HashMap<WorkerId, StdDescriptor>,
    workers: HashMap<GroupId, Vec<GenericWorker<HBF::Backend, I, T::Custom, T::WorkerSync>>>,
    root_dir: PathBuf,
    finalized: bool,

    worker_id_ctr: u32,
    used_cores: HashSet<CoreId>,

    // buffers
    pending_notifications: Vec<(GenericNotification<HandleOf<HBF, I>, T::Custom>, WorkerId)>,

    worker_stdout: WorkdirFile,
    worker_stderr: WorkdirFile,
    worker_stats: WorkdirFile,
}

impl<HBF, I, R, T> Controller for GenericController<HBF, I, R, T>
where
    HBF: InputHandleBackendFactory<StdDescriptor, I>,
    I: Debug,
    R: Router<GenericCommand<HandleOf<HBF, I>, T::Custom>, StdDescriptor>,
    T: Transfer<StdDescriptor, HandleOf<HBF, I>>,
{
    type Worker = GenericWorker<HBF::Backend, I, T::Custom, T::WorkerSync>;
    type GroupConfig = R::GroupConfig;

    fn register_group<G>(&mut self, config: Self::GroupConfig, group: &mut G) -> Result<GroupId>
    where
        G: Group<Self::Worker>,
    {
        if self.finalized {
            return Err(illegal_state!(
                "Trying to register a group in a finalized controller. This is not legal."
            ));
        }

        let group_id = self.orchestrator.router.register_group(config)?;
        let cores = group.cores().clone();

        for core_id in &cores {
            // check core pinning correctness
            if let Some(core_id) = core_id
                && !self.used_cores.insert(core_id)
            {
                return Err(illegal_argument!(
                    "core {core_id:?} is getting pinned on by multiple workers. Use unpinned cores instead."
                ));
            }

            let worker_id = WorkerId(self.worker_id_ctr);
            self.worker_id_ctr += 1;
            let desc = self.new_descriptor(
                &group.layout(group_id, worker_id)?,
                worker_id,
                group_id,
                core_id,
            )?;

            self.orchestrator.router.register_worker(&desc)?;

            self.descriptors.insert(worker_id, desc);
        }

        Ok(group_id)
    }

    fn finalize_orchestration(&mut self) -> Result<()> {
        if mem::replace(&mut self.finalized, true) {
            return Err(illegal_state!(
                "Trying to finalize a controller more than one time. This is not legal."
            ));
        }

        self.orchestrator.router.finalize()?;

        for (wid, desc) in &self.descriptors {
            let source_wids: Vec<WorkerId> = self.orchestrator.router.sources(*wid).collect();

            let sources: Vec<&StdDescriptor> = source_wids
                .into_iter()
                .map(|src_wid| self.descriptors.get(&src_wid).unwrap())
                .collect();

            let handle_backend = self
                .orchestrator
                .handle_backend_factory
                .create(desc, sources.iter().copied())?;

            let worker_sync = self
                .orchestrator
                .transfer
                .create_worker_sync(desc, sources.iter().copied())?;

            let should_report = self.orchestrator.router.has_destinations(*wid);

            let worker =
                GenericWorker::new(desc.clone(), handle_backend, worker_sync, should_report);

            match self.workers.entry(desc.group_id()) {
                Entry::Occupied(mut entry) => entry.get_mut().push(worker),
                Entry::Vacant(entry) => {
                    entry.insert(vec![worker]);
                }
            }
        }

        self.orchestrator.handle_backend_factory.finalize()?;
        self.controller_sync = Some(self.orchestrator.transfer.create_controller_sync()?);

        Ok(())
    }

    fn take_group_workers(&mut self, group: GroupId) -> Result<impl Iterator<Item = Self::Worker>> {
        Ok(self
            .workers
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

    fn on_worker_exit(&mut self, descriptor: &StdDescriptor, exit_code: i32) -> Result<()> {
        log::info!(
            "Worker {:?} exited with code {exit_code}",
            descriptor.worker_id()
        );
        self.controller_sync
            .as_mut()
            .unwrap()
            .remove_worker(descriptor.worker_id())
    }

    fn on_worker_termination(
        &mut self,
        descriptor: &StdDescriptor,
        _termination_code: TerminationCode,
    ) -> Result<()> {
        log::info!("Terminated worker {:?}", descriptor.worker_id);
        self.controller_sync
            .as_mut()
            .unwrap()
            .remove_worker(descriptor.worker_id())
    }

    fn wait_notifications(&mut self, wake_fds: &[BorrowedFd<'_>], timeout: Duration) -> Result<()> {
        let sync = self.controller_sync.as_mut().unwrap();

        if matches!(sync.wait(wake_fds, timeout)?, WaitResult::Event) {
            // notification ready to be handled
            self.pending_notifications.extend(sync.poll()?);

            let mut pending = mem::take(&mut self.pending_notifications);
            for (notification, source) in pending.drain(..) {
                let src_desc = self.descriptors.get(&source).unwrap();
                let Some(command) = notification.into_command(src_desc.group_id())? else {
                    continue;
                };

                let destinations = self.orchestrator.router.route(source, &command)?;
                let sync = self.controller_sync.as_mut().unwrap();

                log::debug!("routing command from worker {source:?}");
                sync.send(destinations, &command)?;
            }
            self.pending_notifications = pending;
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
            .send_to(worker, &GenericCommand::Shutdown)
    }
}

impl<HBF, I, R, T> GenericController<HBF, I, R, T>
where
    HBF: InputHandleBackendFactory<StdDescriptor, I>,
    I: Debug,
    T: Transfer<StdDescriptor, HandleOf<HBF, I>>,
{
    fn resolve_worker_layout(&self, layout: &WorkerLayout) -> Result<PathBuf> {
        let path = layout.workdir();

        let resolved = if path.is_absolute() {
            path.to_owned()
        } else {
            self.root_dir.join(path)
        };

        Ok(path::absolute(resolved)?)
    }

    fn validate_worker_layout(&self, layout: &WorkerLayout) -> Result<PathBuf> {
        let path = self.resolve_worker_layout(layout)?;

        for desc in self.descriptors.values() {
            if desc.name == layout.name() {
                return Err(illegal_argument!(
                    "Worker name {:?} is already in use.",
                    desc.name
                ));
            }

            let desc_dir = desc.workdir.root_dir();

            if desc_dir == path {
                return Err(illegal_argument!(
                    "Worker directory {} is already in use.",
                    path.display()
                ));
            }

            if desc_dir.starts_with(&path) || path.starts_with(desc_dir) {
                return Err(illegal_argument!(
                    "Worker directory {} and {} are overlapping.",
                    path.display(),
                    desc_dir.display(),
                ));
            }
        }

        Ok(path)
    }

    fn new_descriptor(
        &self,
        layout: &WorkerLayout,
        worker_id: WorkerId,
        group_id: GroupId,
        core_id: Option<CoreId>,
    ) -> Result<StdDescriptor> {
        let worker_dir = self.validate_worker_layout(layout)?;

        if let Some(parent) = worker_dir.parent()
            && (!parent.exists() || !parent.is_dir())
        {
            return Err(illegal_argument!(
                "The worker dir \"{}\"'s parent directory does not exist or is not a directory.",
                worker_dir.display()
            ));
        }

        if worker_dir.exists() {
            return Err(illegal_argument!(
                "The worker dir \"{}\" already exists.",
                worker_dir.display()
            ));
        }

        fs::create_dir(worker_dir.as_path())?;

        StdDescriptor::new(
            layout.name().to_string(),
            worker_dir,
            self.worker_stdout.clone(),
            self.worker_stderr.clone(),
            self.worker_stats.clone(),
            worker_id,
            core_id,
            group_id,
        )
    }

    /// Create a new [`GenericController`] using `root_dir` as the root directory.
    /// If overwrite is true, the `root_dir` will be removed before being created again.
    pub fn new(
        orchestrator: Orchestrator<HBF, R, T>,
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
            used_cores: HashSet::default(),
            descriptors: HashMap::default(),
            workers: HashMap::default(),
            pending_notifications: Vec::new(),
            worker_id_ctr: 0,
            finalized: false,
        })
    }
}

impl<I> GenericController<StdInputHandleBackendFactory, I, StdRouter, StdTransfer>
where
    I: Debug,
{
    /// Get a [`StdControllerBuilder`], to build a [`StdController`](crate::controllers::StdController).
    #[must_use]
    pub fn builder() -> StdControllerBuilder<I, StdOrchestrator> {
        StdControllerBuilder::default()
    }
}
