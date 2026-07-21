use crate::{
    controllers::{
        Controller, Descriptor, StdDescriptor, StdWorker, StdWorkerRepr, WorkdirFile,
        standard::builder::StdControllerBuilder,
    },
    launchers::InstanceId,
    sync::{
        GroupId, InputRepr, Orchestrator, Router, StdCommand, StdNotification, StdOrchestrator,
        Transport,
    },
};
use libaflmm_bolts::{CoreId, Cores};
use libaflmm_core::{Result, WorkerId, illegal_argument, internal_bug};
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fs,
    marker::PhantomData,
    mem,
    path::{Path, PathBuf},
};

// get the synchronizer type out of a pair of <Input, Orchestrator>
pub(super) type TransportOf<I, O> = <O as Orchestrator<StdDescriptor, I>>::Transport;
type InputReprOf<I, O> = <O as Orchestrator<StdDescriptor, I>>::InputRepr;
type InputHandleOf<I, O> = <InputReprOf<I, O> as InputRepr<I>>::InputHandle;
pub(super) type StdCommandOf<I, O> = StdCommand<InputHandleOf<I, O>>;
pub(super) type StdNotificationOf<I, O> = StdNotification<InputHandleOf<I, O>>;

type ControllerSyncOf<I, O> = <TransportOf<I, O> as Transport<
    StdCommandOf<I, O>,
    StdDescriptor,
    StdNotificationOf<I, O>,
>>::ControllerSync;
type WorkerSyncOf<I, O> = <TransportOf<I, O> as Transport<
    StdCommandOf<I, O>,
    StdDescriptor,
    StdNotificationOf<I, O>,
>>::WorkerSync;

/// The standard controller.
#[derive(Debug)]
pub struct StdController<I, O>
where
    O: Orchestrator<StdDescriptor, I>,
    TransportOf<I, O>: Transport<StdCommandOf<I, O>, StdDescriptor, StdNotificationOf<I, O>>,
{
    orchestrator: O,
    root_dir: PathBuf,
    workers: HashMap<WorkerId, StdWorkerRepr<ControllerSyncOf<I, O>>>,
    worker_stdout: WorkdirFile,
    worker_stderr: WorkdirFile,
    worker_stats: WorkdirFile,
    phantom: PhantomData<I>,

    worker_id_ctr: u32,
    pending_workers: HashMap<GroupId, Vec<StdWorker<I, InputReprOf<I, O>, WorkerSyncOf<I, O>>>>,
    pending_groups: HashMap<GroupId, Cores>,
}

impl<I, O> Controller for StdController<I, O>
where
    O: Orchestrator<StdDescriptor, I>,
    O::Router: Router<StdCommandOf<I, O>, StdDescriptor>,
    TransportOf<I, O>: Transport<StdCommandOf<I, O>, StdDescriptor, StdNotificationOf<I, O>>,
{
    type Worker = StdWorker<I, InputReprOf<I, O>, WorkerSyncOf<I, O>>;
    type GroupConfig = <O::Router as Router<StdCommandOf<I, O>, StdDescriptor>>::GroupConfig;

    fn register_group(&mut self, config: Self::GroupConfig, cores: &Cores) -> Result<GroupId> {
        let group_id = self.orchestrator.router_mut().register_group(config)?;
        self.pending_groups.insert(group_id, cores.clone());
        Ok(group_id)
    }

    fn finalize_orchestration(&mut self) -> Result<()> {
        let mut used_cores: HashSet<CoreId> = HashSet::new();
        let mut worker_desc: HashMap<WorkerId, StdDescriptor> = HashMap::new();

        for (_, cores) in &self.pending_groups {
            for core_id in cores {
                if let Some(c) = core_id {
                    if !used_cores.insert(c) {
                        return Err(illegal_argument!(
                            "core {c:?} is getting pinned on by multiple workers. Use unpinned cores instead."
                        ));
                    }
                }
            }
        }

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

        for (wid, desc) in worker_desc.iter() {
            let wid = desc.worker_id();

            let source_wids: Vec<WorkerId> = self.orchestrator.router().sources(wid).collect();

            let sources: Vec<&StdDescriptor> = source_wids
                .into_iter()
                .map(|src_wid| worker_desc.get(&src_wid).unwrap())
                .collect();

            let synchronizer = self
                .orchestrator
                .transport_mut()
                .create_synchronizer(desc, sources.iter())?;

            let should_report = self.orchestrator.router().has_destinations(wid);

            let (worker, worker_repr) = StdWorker::new(desc.clone(), synchronizer, should_report)?;

            match self.pending_workers.entry(desc.group_id()) {
                Entry::Occupied(entry) => entry.get_mut().push(worker),
                Entry::Vacant(entry) => {
                    entry.insert(vec![worker]);
                }
            }

            self.workers.insert(wid, worker_repr);
        }

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
        self.workers.values().map(|repr| repr.descriptor())
    }

    fn worker_descriptors_mut(&mut self) -> impl IntoIterator<Item = &mut StdDescriptor> {
        self.workers.values_mut().map(|repr| repr.descriptor_mut())
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

    // fn send_command(&mut self, command: StdCommand, worker_id: WorkerId) -> Result<()> {
    //     let repr = self
    //         .workers
    //         .get_mut(&worker_id)
    //         .ok_or(illegal_argument!("Unknown worker ID"))?;

    //     match &command {
    //         StdCommand::Shutdown => repr.connection_mut().send_blocking(&command),
    //         _ => {
    //             if !repr.connection_mut().send(&command)? {
    //                 log::warn!(
    //                     "Could not send command asynchronously to worker {worker_id:?}. The socket must be full. Falling back to synchronous send..."
    //                 );
    //                 repr.connection_mut().send_blocking(&command)?;
    //             }

    //             Ok(())
    //         }
    //     }
    // }

    fn wait_notifications(&mut self, _timeout: Option<std::time::Duration>) -> Result<()> {
        todo!()
    }

    fn root_dir(&self) -> &Path {
        self.root_dir.as_path()
    }
}

impl<I, O> StdController<I, O>
where
    O: Orchestrator<StdDescriptor, I>,
    TransportOf<I, O>: Transport<StdCommandOf<I, O>, StdDescriptor, StdNotificationOf<I, O>>,
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
    O: Orchestrator<StdDescriptor, I>,
    TransportOf<I, O>: Transport<StdCommandOf<I, O>, StdDescriptor, StdNotificationOf<I, O>>,
{
    /// Create a new [`StdGlobalController`] and will use `root_dir` as the root directory.
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
            worker_stdout,
            worker_stderr,
            worker_stats,
            workers: HashMap::default(),
            pending_groups: HashMap::default(),
            pending_workers: HashMap::default(),
            worker_id_ctr: 0,
            phantom: PhantomData,
        })
    }

    /// Get a [`StdControllerBuilder`], to build a [`StdController`].
    #[must_use]
    pub fn builder() -> StdControllerBuilder<StdOrchestrator> {
        StdControllerBuilder::default()
    }
}
