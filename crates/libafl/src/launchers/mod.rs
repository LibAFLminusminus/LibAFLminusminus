pub struct StdLauncherBuilder<MCT, SB> {
    main_controller: MCT,
    state_builder: SB,
}

pub struct StdLauncher<MCT> {
    main_controller: MCT,
}
