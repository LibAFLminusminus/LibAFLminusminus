use libafl_core::Error;

pub struct StdLauncherBuilder<MCT, SB> {
    global_controller: MCT,
    state_builder: SB,
}

pub struct StdLauncher<MCT, RT> {
    global_controller: MCT,
    runtime: RT,
}

impl<MCT, RT> StdLauncher<MCT, RT> {
    pub fn new(main_controller: MCT, runtime: RT) -> Self {
        Self {
            global_controller: main_controller,
            runtime,
        }
    }

    pub fn start(self) -> Result<(), Error> {
        Ok(())
    }
}
