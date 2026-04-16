/// Simplest runtime, just runs the task.
struct DirectRuntime<S, T> {
    state: S,
    task: T,
}

impl<S, T> DependencyResolver for DirectRuntime<S, T> {}

impl<C, S, T> Runtime<C, S> for DirectRuntime<S, T>
where
    T: FnMut(&mut RuntimeHandle<C, S>, &mut S) -> Result<(), Error>,
{
    unsafe fn run_impl(
        &mut self,
        driver: &mut RuntimeHandle<C, S>,
        controller: &mut C,
    ) -> Result<(), Error> {
        (self.task)(driver, &mut self.state)
    }

    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), Error> {
        unimplemented!("The direct runtime does not implement timeout")
    }

    fn arm_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("The direct runtime does not implement timeout")
    }

    fn disarm_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("The direct runtime does not implement timeout")
    }

    fn unset_timeout(&mut self) -> Result<(), Error> {
        unimplemented!("The direct runtime does not implement timeout")
    }
}
