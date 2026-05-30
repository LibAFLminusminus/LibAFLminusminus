//! An on-disk store

use alloc::rc::Rc;
use core::marker::PhantomData;
use std::path::{Path, PathBuf};

use libaflmm_bolts::Error;
use libaflmm_core::{Result, illegal_argument};
use serde::{Deserialize, Serialize};

use super::{InMemoryCorpusMap, Store};
use crate::{
    corpus::{Testcase, TestcaseFilenameFormat, store::StorageResult, testcase::{TestcaseFilename, TestcaseId}},
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
pub struct OnDiskStore<I, M, S, TF> {
    root_dir: PathBuf,
    testcase_format: TF,
    enabled_map: M,
    disabled_map: M,
    phantom: PhantomData<(I, S)>,
}

/// A builder for [`OnDiskStore`]
#[derive(Default, Debug, Clone)]
pub struct OnDiskStoreBuilder {
    pub(crate) root_dir: Option<PathBuf>,
    pub(crate) filename_format: TestcaseFilenameFormat,
}

impl<I, M, S, TF> OnDiskStore<I, M, S, TF>
where
    TF: TestcaseFilename<I, S>,
{
    /// Instantiate an [`OnDiskStoreBuilder`].
    #[must_use]
    pub fn builder() -> OnDiskStoreBuilder {
        OnDiskStoreBuilder::default()
    }

    /// Get the root path of the disk manager.
    #[must_use]
    pub fn root_path(&self) -> &Path {
        self.root_dir.as_path()
    }

    fn testcase_path(&self, id: TestcaseId) -> PathBuf {
        let testcase_name = self.
        self.root_dir.join(self.file_fmt.to_filename(&id))
    }

    /// Save the input and the metadata on disk
    pub fn save_testcase(&self, testcase: &Testcase<I>) -> Result<TestcaseId> {
        let testcase_id = *testcase.id();
        let testcase_path = self.testcase_path(testcase_id);

        testcase.input().to_file(testcase_path.as_path())?;

        Ok(testcase_id)
    }

    /// load a testcase from its ID
    ///
    /// prerequisite: the testcase should not have been "removed" before.
    /// also, it should only happen if it has been saved before.
    pub fn load_testcase(self: &Rc<Self>, testcase_id: &TestcaseId) -> Result<Testcase<I>> {
        // let testcase_md_path = self.as_ref().testcase_md_path(testcase_id);
        // let ser_fmt = self.md_format.clone();
        // let md = ser_fmt.from_file(testcase_md_path.as_path())?;

        let testcase_path = self.as_ref().testcase_path(*testcase_id);
        let input = I::from_file(testcase_path.as_path())?;

        Ok(Testcase::new(Rc::new(input)))
    }
}

impl<I, M, TF> OnDiskStore<I, M, TF>
where
    M: Default,
{
    /// Create a new [`OnDiskStore`]
    pub fn new(root: impl AsRef<Path>, testcase_format: TF) -> Result<Self> {
        let dir = root.as_ref();

        if !dir.is_dir() {
            return Err(illegal_argument!(
                "On-disk root directory is not a directory: {}",
                dir.display()
            ));
        }

        if !dir.exists() {
            return Err(illegal_argument!(
                "Corpus on-disk directory does not exist: {}",
                dir.display()
            ));
        }
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

    fn add_shared<const ENABLED: bool>(&mut self, testcase: Testcase<I>) -> Result<StorageResult> {
        let testcase_id = *testcase.id();

        let is_present = if ENABLED {
            self.enabled_map.add(testcase_id, testcase_id)
        } else {
            self.disabled_map.add(testcase_id, testcase_id)
        };

        let res = if is_present {
            StorageResult::Duplicate(testcase_id)
        } else {
            self.disk_mgr.save_testcase(&testcase)?;
            StorageResult::Stored(testcase_id)
        };

        Ok(res)
    }

    fn get_from<const ENABLED: bool>(&self, id: &TestcaseId) -> Result<Testcase<I>> {
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

    fn disable(&mut self, id: &TestcaseId) -> Result<()> {
        let tc = self
            .enabled_map
            .remove(id)
            .ok_or_else(|| Error::key_not_found(format!("Index {id} not found")))?;
        self.disabled_map.add(*id, tc);
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
    pub fn root_dir(&mut self, root_dir: impl AsRef<Path>) -> &mut Self {
        self.root_dir = Some(root_dir.as_ref().to_path_buf());
        self
    }

    /// Set the on-disk filename format
    pub fn filename_format(&mut self, filename_format: TestcaseFilenameFormat) -> &mut Self {
        self.filename_format = filename_format;
        self
    }

    /// Build an [`OnDiskStore`].
    /// The root directory must be set.
    pub fn build<I, M>(&self) -> Result<OnDiskStore<I, M>>
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
