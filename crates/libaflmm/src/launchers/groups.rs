use crate::{
    Result,
    controllers::{Controller, Worker, WorkerId},
    launchers::Instances,
    runtimes::{NopRuntime, Runtime, RuntimeHandle, StdForkserverRuntime, StdInProcessRuntime},
    states::NopState,
    sync::GroupId,
};
use core::{fmt::Debug, marker::PhantomData, num::NonZeroUsize, time::Duration};
use libaflmm_bolts::{Cores, StdTimer};
use libaflmm_core::illegal_argument;
use serde::Serialize;
use std::path::{Component, Path, PathBuf};

/// The default timeout for a fuzzer execution.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// The default maximum state size per worker.
// TODO: use a proper heuristic to choose correct ram size
pub const DEFAULT_MAX_STATE_SIZE_PER_WORKER: NonZeroUsize = NonZeroUsize::new(1 << 30).unwrap();

pub trait Group<W>
where
    W: Worker,
{
    /// Layout of a worker in the group, given its group ID
    fn layout(&mut self, group_id: GroupId, worker_id: WorkerId) -> Result<WorkerLayout>;

    /// Cores binded to the group
    fn cores(&self) -> &Cores;

    /// Register instances the group contains
    fn register_instances(
        self,
        workers: impl Iterator<Item = W>,
        instances: &mut Instances<W::Descriptor, W>,
    ) -> Result<()>;
}

pub trait GroupTuple<CT>
where
    CT: Controller,
{
    type Configured: RegisteredGroupTuple<CT>;

    /// Register all groups in the tuple
    fn register_all(self, controller: &mut CT) -> Result<Self::Configured>;
}

pub trait RegisteredGroupTuple<CT>
where
    CT: Controller,
{
    /// build instances from the configured groups
    fn instantiate_all(
        self,
        controller: &mut CT,
        instances: &mut Instances<<CT::Worker as Worker>::Descriptor, CT::Worker>,
    ) -> Result<()>;
}

pub struct PendingGroup<C, G> {
    group: G,
    config: C,
}

pub struct ConfiguredGroup<G> {
    group: G,
    group_id: GroupId,
}

#[derive(Debug, Clone)]
pub struct WorkerLayout {
    name: String,
    workdir: PathBuf,
}

#[derive(Debug)]
pub struct StdGroupBuilder<L, RT, S, SB, TM, W> {
    layout_fn: L,
    cores: Cores,
    state_builder: SB,
    runtime: RT,
    max_state_size_per_client: Option<NonZeroUsize>,
    timer: TM,
    timeout: Option<Duration>,
    phantom: PhantomData<(S, W)>,
}

impl<L, RT, S, SB, TM, W> Clone for StdGroupBuilder<L, RT, S, SB, TM, W>
where
    L: Clone,
    RT: Clone,
    SB: Clone,
    TM: Clone,
{
    fn clone(&self) -> Self {
        Self {
            layout_fn: self.layout_fn.clone(),
            cores: self.cores.clone(),
            state_builder: self.state_builder.clone(),
            runtime: self.runtime.clone(),
            max_state_size_per_client: self.max_state_size_per_client,
            timer: self.timer.clone(),
            timeout: self.timeout,
            phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct StdGroup<L, RT, S, SB> {
    layout_fn: L,
    cores: Cores,
    state_builder: SB,
    runtime: RT,
    phantom: PhantomData<S>,
}

impl<C, G> PendingGroup<C, G> {
    pub fn new(group: G, config: C) -> Self {
        Self { group, config }
    }
}

impl<L, RT, S, SB, W> Group<W> for StdGroup<L, RT, S, SB>
where
    L: FnMut(GroupId, WorkerId) -> Result<WorkerLayout>,
    RT: Runtime<S, W> + Clone + 'static,
    S: 'static,
    SB: FnMut(&W) -> Result<S>,
    W: Worker,
{
    fn layout(&mut self, group_id: GroupId, worker_id: WorkerId) -> Result<WorkerLayout> {
        (self.layout_fn)(group_id, worker_id)
    }

    fn cores(&self) -> &Cores {
        &self.cores
    }

    fn register_instances(
        mut self,
        workers: impl Iterator<Item = W>,
        instances: &mut Instances<W::Descriptor, W>,
    ) -> Result<()> {
        // create an instance per core, ready to run.
        for worker in workers {
            // get the core of the worker
            let core = worker.core_id();

            // create the state for the instance
            let state: S = (self.state_builder)(&worker)?;

            // clone the runtime
            let mut runtime = self.runtime.clone();

            // add the instance to the list
            instances.add(move |worker| runtime.run(state, worker), worker, core);
        }

        Ok(())
    }
}

impl
    StdGroup<
        fn(GroupId, WorkerId) -> Result<String>,
        NopRuntime,
        NopState,
        fn() -> Result<NopState>,
    >
{
    #[expect(clippy::type_complexity)]
    pub fn builder<CT>(
        _controller: &CT,
    ) -> StdGroupBuilder<
        fn(GroupId, WorkerId) -> Result<WorkerLayout>,
        NopRuntime,
        NopState,
        fn(&CT::Worker) -> Result<NopState>,
        StdTimer,
        CT::Worker,
    >
    where
        CT: Controller,
    {
        let runtime = NopRuntime;
        let cores = Cores::one();

        StdGroupBuilder {
            layout_fn: |_, wid| WorkerLayout::flat(format!("worker_{:}", wid.id())),
            runtime,
            cores,
            state_builder: |_| NopState::nop(),
            max_state_size_per_client: None,
            timeout: Some(DEFAULT_TIMEOUT),
            timer: StdTimer::new(),
            phantom: PhantomData,
        }
    }
}

impl<L, RT, S, SB, TM, W> StdGroupBuilder<L, RT, S, SB, TM, W> {
    /// Set the [`Runtime`].
    #[must_use]
    pub fn worker_layout_fn<L2>(self, layout_fn: L2) -> StdGroupBuilder<L2, RT, S, SB, TM, W> {
        StdGroupBuilder {
            layout_fn,
            runtime: self.runtime,
            cores: self.cores,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            timeout: self.timeout,
            timer: self.timer,
            phantom: self.phantom,
        }
    }

    /// Set the cores associated to each [`Instance`](crate::launchers::Instance).
    #[must_use]
    pub fn cores(mut self, cores: Cores) -> Self {
        self.cores = cores;
        self
    }

    /// Set the [`Runtime`].
    #[must_use]
    pub fn runtime<RT2>(self, runtime: RT2) -> StdGroupBuilder<L, RT2, S, SB, TM, W> {
        StdGroupBuilder {
            layout_fn: self.layout_fn,
            runtime,
            cores: self.cores,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            timeout: self.timeout,
            timer: self.timer,
            phantom: self.phantom,
        }
    }

    /// Set the [`State`](crate::states::State) builder closure.
    #[must_use]
    pub fn state_builder<S2, SB2>(
        self,
        state_builder: SB2,
    ) -> StdGroupBuilder<L, RT, S2, SB2, TM, W>
    where
        SB2: FnMut(&W) -> Result<S2>,
    {
        StdGroupBuilder {
            layout_fn: self.layout_fn,
            runtime: self.runtime,
            cores: self.cores,
            state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            timeout: self.timeout,
            timer: self.timer,
            phantom: PhantomData,
        }
    }

    /// Set the timer used by the runtime built with [`Self::build_inprocess`].
    #[must_use]
    pub fn timer<TM2>(self, timer: TM2) -> StdGroupBuilder<L, RT, S, SB, TM2, W> {
        StdGroupBuilder {
            layout_fn: self.layout_fn,
            cores: self.cores,
            runtime: self.runtime,
            state_builder: self.state_builder,
            max_state_size_per_client: self.max_state_size_per_client,
            timeout: self.timeout,
            timer,
            phantom: self.phantom,
        }
    }

    /// Set the timeout of the instance
    #[must_use]
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the runtime as an [`StdInProcessRuntime`].
    #[expect(clippy::type_complexity)]
    pub fn build_inprocess<T>(
        self,
        task: T,
    ) -> Result<StdGroup<L, StdInProcessRuntime<S, T, TM>, S, SB>>
    where
        S: Serialize,
        T: FnMut(&mut RuntimeHandle<S, W>, &mut S) -> Result<()> + Clone,
    {
        let ram_limit = self
            .max_state_size_per_client
            .unwrap_or(DEFAULT_MAX_STATE_SIZE_PER_WORKER);

        let runtime = StdInProcessRuntime::new(task, ram_limit, self.timer, self.timeout);

        StdGroup::new(self.layout_fn, self.cores, self.state_builder, runtime)
    }

    /// Set the runtime as an [`StdForkserverRuntime`].
    pub fn build_forkserver<T>(self, task: T) -> Result<StdGroup<L, StdForkserverRuntime<T>, S, SB>>
    where
        T: FnMut(&mut RuntimeHandle<S, W>, &mut S) -> Result<()> + Clone,
    {
        let runtime = StdForkserverRuntime::new(task);

        StdGroup::new(self.layout_fn, self.cores, self.state_builder, runtime)
    }

    pub fn build(self) -> Result<StdGroup<L, RT, S, SB>> {
        StdGroup::new(self.layout_fn, self.cores, self.state_builder, self.runtime)
    }
}

impl<L, RT, S, SB> StdGroup<L, RT, S, SB> {
    pub fn new(name_fn: L, cores: Cores, state_builder: SB, runtime: RT) -> Result<Self> {
        if cores.is_empty() {
            return Err(illegal_argument!(
                "No CPU cores have been allocated for the group."
            ));
        }

        Ok(Self {
            layout_fn: name_fn,
            cores,
            state_builder,
            runtime,
            phantom: PhantomData,
        })
    }
}

impl<CT> RegisteredGroupTuple<CT> for ()
where
    CT: Controller,
{
    fn instantiate_all(
        self,
        _controller: &mut CT,
        _instances: &mut Instances<<CT::Worker as Worker>::Descriptor, CT::Worker>,
    ) -> Result<()> {
        Ok(())
    }
}

impl<CT> GroupTuple<CT> for ()
where
    CT: Controller,
{
    type Configured = ();

    fn register_all(self, _controller: &mut CT) -> Result<Self::Configured> {
        Ok(())
    }
}

impl<CT, Head, Tail> RegisteredGroupTuple<CT> for (ConfiguredGroup<Head>, Tail)
where
    CT: Controller,
    Head: Group<CT::Worker>,
    Tail: RegisteredGroupTuple<CT>,
{
    fn instantiate_all(
        self,
        controller: &mut CT,
        instances: &mut Instances<
            <<CT as Controller>::Worker as Worker>::Descriptor,
            <CT as Controller>::Worker,
        >,
    ) -> Result<()> {
        let (ConfiguredGroup { group_id, group }, tail) = self;

        // start instantiation with tail
        tail.instantiate_all(controller, instances)?;

        // get the works of the group being handled
        let workers = controller.take_group_workers(group_id)?;

        // register instances for the group's workers
        group.register_instances(workers, instances)
    }
}

impl<CT, Head, Tail> GroupTuple<CT> for (PendingGroup<CT::GroupConfig, Head>, Tail)
where
    CT: Controller,
    Head: Group<CT::Worker>,
    Tail: GroupTuple<CT>,
{
    type Configured = (ConfiguredGroup<Head>, Tail::Configured);

    fn register_all(self, controller: &mut CT) -> Result<Self::Configured> {
        let (PendingGroup { config, mut group }, tail) = self;

        let configured_tail = tail.register_all(controller)?;
        let group_id = controller.register_group(config, &mut group)?;

        Ok((ConfiguredGroup { group, group_id }, configured_tail))
    }
}

impl WorkerLayout {
    pub fn new(name: impl AsRef<str>, workdir: impl AsRef<Path>) -> Result<Self> {
        if name.as_ref().is_empty() {
            return Err(illegal_argument!("Group name must not be empty"));
        }

        if workdir.as_ref().as_os_str().is_empty() || workdir.as_ref().file_name().is_none() {
            return Err(illegal_argument!(
                "Worker workdir must identify a directory"
            ));
        }

        if workdir
            .as_ref()
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(illegal_argument!(
                "Worker workdir must not contain '..'. Use Absolute paths instead."
            ));
        }

        Ok(Self {
            name: name.as_ref().to_string(),
            workdir: workdir.as_ref().to_path_buf(),
        })
    }

    /// Create a flat Group layout: the name and the workdir are identical
    pub fn flat(name: impl AsRef<str>) -> Result<Self> {
        Self::new(name.as_ref(), name.as_ref())
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// work directory relative to the root directory
    #[must_use]
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }
}
