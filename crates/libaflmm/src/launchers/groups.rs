use crate::{
    Result,
    controllers::Controller,
    launchers::Instances,
    runtimes::{NopRuntime, Runtime, RuntimeHandle, StdForkserverRuntime, StdInProcessRuntime},
    states::NopState,
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

pub trait Group<W> {
    /// Cores Binded to the group
    fn cores(&self) -> &Cores;

    /// Register instances the group contains
    fn register_instances<CT>(
        self,
        controller: &mut CT,
        instances: &mut Instances<CT::Descriptor, W>,
    ) -> Result<()>
    where
        CT: Controller<Worker = W>;
}

pub trait GroupTuple<W> {
    fn register_all<CT>(
        self,
        controller: &mut CT,
        instances: &mut Instances<CT::Descriptor, W>,
    ) -> Result<()>
    where
        CT: Controller<Worker = W>;
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

impl<RT, S, SB, W> Group<W> for StdGroup<RT, S, SB>
where
    RT: Runtime<S, W> + Clone + 'static,
    S: 'static,
    SB: FnMut(&W) -> Result<S>,
{
    fn cores(&self) -> &Cores {
        &self.cores
    }

    fn register_instances<CT>(
        mut self,
        controller: &mut CT,
        instances: &mut Instances<CT::Descriptor, W>,
    ) -> Result<()>
    where
        CT: Controller<Worker = W>,
    {
        // create an instance per core, ready to run.
        for core in &self.cores {
            // spawn a controller for the instance
            let worker = controller.create_worker(core)?;

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

impl<W> GroupTuple<W> for () {
    fn register_all<CT>(
        self,
        _controller: &mut CT,
        _instances: &mut Instances<CT::Descriptor, W>,
    ) -> Result<()>
    where
        CT: Controller<Worker = W>,
    {
        Ok(())
    }
}

impl<W, Head, Tail> GroupTuple<W> for (Head, Tail)
where
    Head: Group<W>,
    Tail: GroupTuple<W>,
{
    fn register_all<CT>(
        self,
        controller: &mut CT,
        instances: &mut Instances<CT::Descriptor, W>,
    ) -> Result<()>
    where
        CT: Controller<Worker = W>,
    {
        self.0.register_instances(controller, instances)?;
        self.1.register_all(controller, instances)
    }
}
