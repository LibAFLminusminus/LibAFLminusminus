//! An `AFL++`-style synchronizer.
//!
//! If follows closely how AFL++ does synchronization, with
//!     - 1 main node
//!     - `n-1` secondary nodes.
//!
//! Nodes discovery and inputs sharing is done through the filesystem.
//! The same idea of remembering the already handled test cases is used to avoid loading already imported inputs.
//!
//! THIS IS OUTDATED, IT SHOULD USE THE NEW DESIGN

use crate::{
    Result,
    controllers::{Descriptor, Worker},
    inputs::Input,
    synchronizer::{InputRepr, Synchronizer},
};
use libaflmm_core::WorkerId;
use std::path::PathBuf;

const IS_MAIN_MARKER_FILENAME: &str = "is_main_node";

struct NodeRef<D> {
    desc: D,
    last_imported: Option<usize>,
}

struct MainNode<D> {
    // sync dir, in which other clients live.
    secondary_nodes: Vec<NodeRef<D>>,
    // buffer with loaded inputs, useful to avoid realloc each time.
    input_buf: Vec<AflppInputRepr>,
}

struct SecondaryNode<D> {
    // the main node, if already discovered.
    main_node: Option<NodeRef<D>>,
}

pub enum AflppNode<D> {
    /// will read inputs from other secondary workers
    Main(MainNode<D>),
    /// will read inputs only from the main worker
    Secondary(SecondaryNode<D>),
}

pub struct AflppSynchronizer<D> {
    desc: D,
    node: AflppNode<D>,
}

pub struct AflppInputRepr {
    path: PathBuf,
    worker_id: WorkerId,
}

fn is_main_node<W: Worker>(desc: &W::Descriptor) -> bool {
    desc.workdir().is_file(IS_MAIN_MARKER_FILENAME)
}

fn is_secondary_node<W: Worker>(desc: &W::Descriptor) -> bool {
    !is_main_node::<W>(desc)
}

impl<I> InputRepr<I> for AflppInputRepr
where
    I: Input,
{
    fn load_input(&self) -> Result<I> {
        I::from_file(&self.path)
    }
}

impl<D> AflppSynchronizer<D>
where
    D: Clone,
{
    pub fn new(desc: &D, node: AflppNode<D>) -> Self {
        Self {
            desc: desc.clone(),
            node,
        }
    }
}

impl<D> MainNode<D> {
    pub fn new() -> Self {
        MainNode {
            input_buf: Vec::new(),
            secondary_nodes: Vec::new(),
        }
    }
}

impl<D> SecondaryNode<D> {
    pub fn new() -> Self {
        SecondaryNode { main_node: None }
    }
}

impl<D> NodeRef<D>
where
    D: Clone,
{
    pub fn new(desc: &D) -> Self {
        Self {
            desc: desc.clone(),
            last_imported: None,
        }
    }
}

impl<D, I> Synchronizer<D, I> for AflppSynchronizer<D>
where
    D: Descriptor,
    I: Input,
{
    type InputRepr = AflppInputRepr;

    fn on_create(&mut self) -> Result<()> {
        match &self.node {
            AflppNode::Main(_) => {
                // mark ourself as main node
                self.desc.workdir().create_file(IS_MAIN_MARKER_FILENAME)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn on_new_worker(&mut self, desc: &D) -> Result<()> {
        match &self.node {
            AflppNode::Main(node) => {
                if is_secondary_node(desc) {
                    // i'm a main node and i register a secondary node
                    node.secondary_nodes.push(NodeRef::new(desc));
                }
            }
            AflppNode::Secondary(node) => {
                if is_main_node(desc) {
                    if node.main_node.is_some() {
                        panic!(
                            "A main worker has already been registered. Multiple main nodes is not supported."
                        );
                    }

                    // i'm a secondary node and i register the main node
                    node.main_node = Some(NodeRef::new(desc))
                }
            }
        }

        Ok(())
    }

    fn report_input(&mut self, _desc: &mut D, _input_repr: Self::InputRepr) -> Result<()> {
        Ok(())
    }

    fn sync_input(&mut self, _desc: &mut D) -> Result<impl Iterator<Item = AflppInputRepr>> {
        match &self.node {
            AflppNode::Main(node) => {
                // main node does sync with every other secondary nodes
            }
        }

        Ok([].into_iter())
    }
}
