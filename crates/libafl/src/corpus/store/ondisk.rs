//! An on-disk store

use alloc::{rc::Rc, string::String};
use core::{cell::RefCell, marker::PhantomData};
use std::{
    fs, io,
    path::{Path, PathBuf},
    string::ToString,
};

use libafl_bolts::Error;
use serde::{Deserialize, Serialize};

use super::{InMemoryCorpusMap, Store};
use crate::{
    corpus::{testcase::TestcaseId, Testcase, TestcaseFilenameFormat},
    inputs::Input,
};

/// An on-disk store
///
/// The maps only store the unique ID associated to the added [`Testcase`]s.
/// The inputs are added in the same directory.
///
/// This store does not support multiple concurrent management.
/// In other words, multiple [`OnDiskStore`]s should not be instantiated concurrently with
/// the same root directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnDiskStore<I, M> {
    filename_format: TestcaseFilenameFormat,
    disk_mgr: Rc<DiskMgr<I>>,
    enabled_map: M,
    disabled_map: M,
}

/// A builder for [`OnDiskStore`]
#[derive(Default, Debug, Clone)]
pub struct OnDiskStoreBuilder {
    pub(crate) root_dir: Option<PathBuf>,
    pub(crate) filename_format: TestcaseFilenameFormat,
    pub(crate) md_format: OnDiskMetadataFormat,
}

/// A Disk Manager, able to load and store [`Testcase`]s
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMgr<I> {
    root_dir: PathBuf,
    file_fmt: TestcaseFilenameFormat,
    phantom: PhantomData<I>,
}

/// An on-disk [`Testcase`] cell.
#[derive(Debug)]
pub struct OnDiskTestcaseCell<I> {
    mgr: Rc<DiskMgr<I>>,
    id: String,
    modified: RefCell<bool>,
}

/// Options for the the format of the on-disk metadata
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum OnDiskMetadataFormat {
    /// A binary-encoded postcard
    Postcard,
    /// JSON
    Json,
    /// JSON formatted for readability
    #[default]
    JsonPretty,
    /// The same as [`OnDiskMetadataFormat::JsonPretty`], but compressed
    #[cfg(feature = "gzip")]
    JsonGzip,
}

impl<I> OnDiskTestcaseCell<I> {
    /// Get a new [`OnDiskTestcaseCell`].
    #[must_use]
    pub fn new(mgr: Rc<DiskMgr<I>>, id: String) -> Self {
        Self {
            mgr,
            id,
            modified: RefCell::new(false),
        }
    }
}

impl<I> DiskMgr<I> {
    /// Create a new [`DiskMgr`]
    pub fn new(root_dir: PathBuf) -> Result<Self, Error> {
        Self::new_with_format(root_dir, TestcaseFilenameFormat::default())
    }

    /// Create a new [`DiskMgr`]
    pub fn new_with_format(
        root_dir: PathBuf,
        file_fmt: TestcaseFilenameFormat,
    ) -> Result<Self, Error> {
        match fs::create_dir_all(&root_dir) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }

        Ok(Self {
            root_dir,
            file_fmt,
            phantom: PhantomData,
        })
    }

    /// Get the root path of the disk manager.
    #[must_use]
    pub fn root_path(&self) -> &Path {
        self.root_dir.as_path()
    }

    fn testcase_path(&self, id: &TestcaseId) -> PathBuf {
        self.root_dir
            .join(self.file_fmt.to_filename(id.to_string().as_str()))
    }
}

impl<I> DiskMgr<I>
where
    I: Input,
{
    /// Save the input only to disk, according to metadata.
    pub fn save_input(&self, input: &I) -> Result<TestcaseId, Error> {
        let testcase_id = Testcase::<I>::compute_id(input);
        let testcase_path = self.testcase_path(&testcase_id);
        input.to_file(testcase_path.as_path())?;

        Ok(testcase_id)
    }

    /// Save the input and the metadata on disk
    pub fn save_testcase(&self, input: &I) -> Result<TestcaseId, Error> {
        let id = self.save_input(input)?;
        Ok(id)
    }

    /// load a testcase from its ID
    ///
    /// prerequisite: the testcase should not have been "removed" before.
    /// also, it should only happen if it has been saved before.
    pub fn load_testcase(self: &Rc<Self>, testcase_id: &TestcaseId) -> Result<Testcase<I>, Error> {
        // let testcase_md_path = self.as_ref().testcase_md_path(testcase_id);
        // let ser_fmt = self.md_format.clone();
        // let md = ser_fmt.from_file(testcase_md_path.as_path())?;

        let testcase_path = self.as_ref().testcase_path(testcase_id);
        let input = I::from_file(testcase_path.as_path())?;

        Ok(Testcase::new(Rc::new(input)))
    }
}

impl<I, M> OnDiskStore<I, M> {
    /// Instantiate an [`OnDiskStoreBuilder`].
    #[must_use]
    pub fn builder() -> OnDiskStoreBuilder {
        OnDiskStoreBuilder::default()
    }

    /// Get the disk manager of the store
    pub fn disk_mgr(&self) -> &DiskMgr<I> {
        self.disk_mgr.as_ref()
    }
}

impl<I, M> OnDiskStore<I, M>
where
    M: Default,
{
    /// Create a new [`OnDiskStore`]
    pub fn new(root: PathBuf, filename_format: TestcaseFilenameFormat) -> Result<Self, Error> {
        let disk_mgr = Rc::new(DiskMgr::new(root)?);

        Ok(Self {
            filename_format,
            disk_mgr,
            enabled_map: M::default(),
            disabled_map: M::default(),
        })
    }
}

impl<I, M> Store<I> for OnDiskStore<I, M>
where
    I: Input,
    M: InMemoryCorpusMap<TestcaseId>,
{
    fn count_all(&self) -> usize {
        self.count().saturating_add(self.count_disabled())
    }

    fn is_empty(&self) -> bool {
        self.count() == 0
    }

    fn count(&self) -> usize {
        self.enabled_map.count()
    }

    fn count_disabled(&self) -> usize {
        self.disabled_map.count()
    }

    fn add_shared<const ENABLED: bool>(&mut self, input: Rc<I>) -> Result<TestcaseId, Error> {
        let testcase_id = self.disk_mgr.save_testcase(input.as_ref())?;

        if ENABLED {
            // id map
            self.enabled_map.add(testcase_id, testcase_id);
        } else {
            self.disabled_map.add(testcase_id, testcase_id);
        }

        Ok(testcase_id)
    }

    fn get_from<const ENABLED: bool>(&self, id: TestcaseId) -> Result<Testcase<I>, Error> {
        let tc_id = if ENABLED {
            self.enabled_map
                .get(id)
                .ok_or_else(|| Error::key_not_found(format!("Index not found: {id}")))?
        } else {
            self.enabled_map
                .get(id)
                .or_else(|| self.disabled_map.get(id))
                .ok_or_else(|| Error::key_not_found(format!("Index {id} not found")))?
        };

        self.disk_mgr.load_testcase(tc_id)
    }

    fn disable(&mut self, id: TestcaseId) -> Result<(), Error> {
        let tc = self
            .enabled_map
            .remove(id)
            .ok_or_else(|| Error::key_not_found(format!("Index {id} not found")))?;
        self.disabled_map.add(id, tc);
        Ok(())
    }
}

impl OnDiskStoreBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the root directory, where the testcases will be stored.
    pub fn root_dir(&mut self, root: &Path) -> &mut Self {
        self.root_dir = Some(root.to_path_buf());
        self
    }

    /// Set the on-disk filename format
    pub fn filename_format(&mut self, filename_format: TestcaseFilenameFormat) -> &mut Self {
        self.filename_format = filename_format;
        self
    }

    /// Set the metadata serialization format.
    pub fn md_format(&mut self, md_format: OnDiskMetadataFormat) -> &mut Self {
        self.md_format = md_format;
        self
    }

    /// Build an [`OnDiskStore`].
    /// The root directory must be set.
    pub fn build<I, M>(&self) -> Result<OnDiskStore<I, M>, Error>
    where
        M: Default,
    {
        OnDiskStore::new(
            self.root_dir
                .as_ref()
                .ok_or_else(|| Error::illegal_argument("Root directory not set"))?
                .clone(),
            self.filename_format.clone(),
        )
    }
}
