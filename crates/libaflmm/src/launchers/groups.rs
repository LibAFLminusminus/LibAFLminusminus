use crate::{
    controllers::{Controller, Worker},
    launchers::Instances,
    runtimes::{NopRuntime, Runtime, RuntimeHandle, StdForkserverRuntime, StdInProcessRuntime},
    states::NopState,
    sync::GroupId,
    Result,
};
use core::{fmt::Debug, marker::PhantomData, num::NonZeroUsize, time::Duration};
use libaflmm_bolts::{Cores, StdTimer};
use libaflmm_core::illegal_argument;
use serde::Serialize;

/// The default timeout for a fuzzer execution.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// The default maximum state size per worker.
// TODO: use a proper heuristic to choose correct ram size
pub const DEFAULT_MAX_STATE_SIZE_PER_WORKER: NonZeroUsize = NonZeroUsize::new(1 << 30).unwrap();

pub trait Group<W>
where
    W: Worker,
{
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

pub struct StdGroupBuilder<RT, S, SB, TM, W> {
    cores: Cores,
    state_builder: SB,
    runtime: RT,
    max_state_size_per_client: Option<NonZeroUsize>,
    timer: TM,
    timeout: Option<Duration>,
    phantom: PhantomData<(S, W)>,
}

#[derive(Debug)]
pub struct StdGroup<RT, S, SB> {
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

impl<RT, S, SB, W> Group<W> for StdGroup<RT, S, SB>
where
    RT: Runtime<S, W> + Clone + 'static,
    S: 'static,
    SB: FnMut(&W) -> Result<S>,
    W: Worker,
{
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

impl StdGroup<NopRuntime, NopState, fn() -> Result<NopState>> {
    #[expect(clippy::type_complexity)]
    pub fn builder<CT>(
        _controller: &CT,
    ) -> StdGroupBuilder<
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

impl<RT, S, SB, TM, W> StdGroupBuilder<RT, S, SB, TM, W> {
    /// Set the cores associated to each [`Instance`](crate::launchers::Instance).
    #[must_use]
    pub fn cores(mut self, cores: Cores) -> Self {
        self.cores = cores;
        self
    }

    /// Set the [`Runtime`].
    #[must_use]
    pub fn runtime<RT2>(self, runtime: RT2) -> StdGroupBuilder<RT2, S, SB, TM, W> {
        StdGroupBuilder {
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
    pub fn state_builder<S2, SB2>(self, state_builder: SB2) -> StdGroupBuilder<RT, S2, SB2, TM, W>
    where
        SB2: FnMut(&W) -> Result<S2>,
    {
        StdGroupBuilder {
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
    pub fn timer<TM2>(self, timer: TM2) -> StdGroupBuilder<RT, S, SB, TM2, W> {
        StdGroupBuilder {
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
    pub fn build_inprocess<T>(
        self,
        task: T,
    ) -> Result<StdGroup<StdInProcessRuntime<S, T, TM>, S, SB>>
    where
        S: Serialize,
        T: FnMut(&mut RuntimeHandle<S, W>, &mut S) -> Result<()> + Clone,
    {
        let ram_limit = self
            .max_state_size_per_client
            .unwrap_or(DEFAULT_MAX_STATE_SIZE_PER_WORKER);

        let runtime = StdInProcessRuntime::new(task, ram_limit, self.timer, self.timeout);

        StdGroup::new(self.cores, self.state_builder, runtime)
    }

    /// Set the runtime as an [`StdForkserverRuntime`].
    pub fn build_forkserver<T>(self, task: T) -> Result<StdGroup<StdForkserverRuntime<T>, S, SB>>
    where
        T: FnMut(&mut RuntimeHandle<S, W>, &mut S) -> Result<()> + Clone,
    {
        let runtime = StdForkserverRuntime::new(task);

        StdGroup::new(self.cores, self.state_builder, runtime)
    }

    pub fn build(self) -> Result<StdGroup<RT, S, SB>> {
        StdGroup::new(self.cores, self.state_builder, self.runtime)
    }
}

impl<RT, S, SB> StdGroup<RT, S, SB> {
    pub fn new(cores: Cores, state_builder: SB, runtime: RT) -> Result<Self> {
        if cores.is_empty() {
            return Err(illegal_argument!(
                "No CPU cores have been allocated for the group."
            ));
        }

        Ok(Self {
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
        let (PendingGroup { config, group }, tail) = self;

        let configured_tail = tail.register_all(controller)?;
        let group_id = controller.register_group(config, group.cores())?;

        Ok((ConfiguredGroup { group_id, group }, configured_tail))
    }
}
