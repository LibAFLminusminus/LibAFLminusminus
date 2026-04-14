pub trait GlobalController {
    type LocalController<Workdir = Self::Workdir>;
    type Workdir;

    fn create_local_controller(&mut self) -> Result<Self::LocalController, Error>;

    fn workdir(&self) -> &Self::Workdir;
    fn workdir(&mut self) -> &mut Self::Workdir;
}

pub trait LocalController {
    type Workdir;

    fn workdir(&self) -> &Self::Workdir;
    fn workdir_mut(&mut self) -> &mut Self::Workdir;
}

// pub trait Controller {
//     type GlobalController;
//     type LocalController;
// }