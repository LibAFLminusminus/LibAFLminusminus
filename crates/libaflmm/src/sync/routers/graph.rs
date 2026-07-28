use crate::{
    Result,
    controllers::Descriptor,
    sync::{GroupId, Router},
};
use core::{fmt::Debug, hash::Hash, mem};
use libaflmm_core::{WorkerId, illegal_argument, illegal_state, internal_bug};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct GraphRouterBuilder<K> {
    routes: HashSet<(K, K)>,
}

/// A graph-style router
///
/// GID is a generic group identifier
#[derive(Debug)]
pub struct GraphRouter<K = usize> {
    // registered groups
    group_keys: HashMap<K, GroupId>,
    group_ids: HashMap<GroupId, HashSet<WorkerId>>,
    workers: HashMap<WorkerId, GroupId>,

    // fill be filled on finalize
    routes: HashSet<(K, K)>,
    dsts: HashMap<WorkerId, HashSet<WorkerId>>,
    srcs: HashMap<WorkerId, HashSet<WorkerId>>,

    group_id_ctr: u64,
    finalized: bool,
}

impl<K> GraphRouter<K> {
    #[must_use]
    pub fn builder() -> GraphRouterBuilder<K> {
        GraphRouterBuilder::default()
    }

    // this is private because routes must be correct to start with
    // there is no reason to use that over the builder
    fn new(routes: HashSet<(K, K)>) -> Self {
        Self {
            routes,
            group_id_ctr: 0,
            dsts: HashMap::default(),
            finalized: false,
            group_ids: HashMap::default(),
            group_keys: HashMap::default(),
            srcs: HashMap::default(),
            workers: HashMap::default(),
        }
    }

    fn alloc_group_id(&mut self) -> GroupId {
        let id = self.group_id_ctr;
        self.group_id_ctr += 1;
        GroupId { id }
    }

    fn check_registering(&self) -> Result<()> {
        if self.finalized {
            return Err(illegal_argument!(
                "graph router has already been finalized, it cannot register anything else"
            ));
        }

        Ok(())
    }

    fn check_finalized(&self) -> Result<()> {
        if !self.finalized {
            return Err(illegal_argument!("graph router has not been finalized yet"));
        }

        Ok(())
    }
}

impl<CMD, D, K> Router<CMD, D> for GraphRouter<K>
where
    D: Descriptor,
    K: Clone + Debug + Eq + Hash,
{
    type GroupConfig = K;

    fn register_group(&mut self, group_key: K) -> Result<GroupId> {
        self.check_registering()?;

        if let Some(group_key) = self.group_keys.get(&group_key) {
            return Err(illegal_argument!(
                "Group with key {group_key:?} has already been registered"
            ));
        }

        let group_id = self.alloc_group_id();
        self.group_ids.insert(group_id, HashSet::new());
        self.group_keys.insert(group_key, group_id);

        Ok(group_id)
    }

    fn register_worker(&mut self, desc: &D) -> Result<()> {
        self.check_registering()?;

        let worker = desc.worker_id();
        let group = desc.group_id();

        if self.workers.contains_key(&worker) {
            return Err(internal_bug!(
                "The same worker has been registered multiple times"
            ));
        }

        if let Some(workers) = self.group_ids.get_mut(&group) {
            self.workers.insert(worker, group);
            workers.insert(worker);
            Ok(())
        } else {
            Err(illegal_argument!("An unknown group ID has been provided"))
        }
    }

    fn finalize(&mut self) -> Result<()> {
        if mem::replace(&mut self.finalized, true) {
            return Err(illegal_state!("graph router has already been finalized"));
        }

        let mut group_routes: HashSet<(GroupId, GroupId)> = HashSet::default();
        for (src_key, dst_key) in &self.routes {
            let src = self
                .group_keys
                .get(src_key)
                .ok_or_else(|| illegal_argument!("using unregistered source group {src_key:?}"))?;

            let dst = self.group_keys.get(dst_key).ok_or_else(|| {
                illegal_argument!("using unregistered destination group {src_key:?}")
            })?;

            group_routes.insert((*src, *dst));
        }

        for worker in self.workers.keys() {
            self.srcs.insert(*worker, HashSet::default());
            self.dsts.insert(*worker, HashSet::default());
        }

        // efficient and works well for small graphs, but memory inneficient for dense graphs, as it will have many edges to store.
        // switch to lazy computation if this becomes a problem.
        for (src_grp, dst_grp) in group_routes {
            let srcs = self.group_ids.get(&src_grp).unwrap();
            let dsts = self.group_ids.get(&dst_grp).unwrap();

            for src in srcs {
                for dst in dsts {
                    if src == dst {
                        return Err(illegal_state!("groups must be disjoint"));
                    }

                    self.dsts.get_mut(src).unwrap().insert(*dst);
                    self.srcs.get_mut(dst).unwrap().insert(*src);
                }
            }
        }

        Ok(())
    }

    fn destinations(&self, worker: WorkerId) -> impl Iterator<Item = WorkerId> {
        self.check_finalized().unwrap();

        self.dsts
            .get(&worker)
            .into_iter()
            .flat_map(|workers| workers.iter().copied())
    }

    fn sources(&self, worker: WorkerId) -> impl Iterator<Item = WorkerId> {
        self.check_finalized().unwrap();

        self.srcs
            .get(&worker)
            .into_iter()
            .flat_map(|workers| workers.iter().copied())
    }
}

impl<K> Default for GraphRouterBuilder<K> {
    fn default() -> Self {
        Self {
            routes: HashSet::default(),
        }
    }
}

impl<K> GraphRouterBuilder<K>
where
    K: Hash + Eq + Clone + Debug,
{
    pub fn route(mut self, src: K, dst: K) -> Result<Self> {
        let edge = (src, dst);

        if self.routes.contains(&edge) {
            Err(illegal_argument!(
                "The route {:?} -> {:?} already exists",
                edge.0,
                edge.1
            ))
        } else {
            self.routes.insert(edge);
            Ok(self)
        }
    }

    pub fn share(self, node: K, other_node: K) -> Result<Self> {
        self.route(node.clone(), other_node.clone())?
            .route(other_node, node)
    }
}

impl<K> GraphRouterBuilder<K> {
    #[must_use]
    pub fn build(self) -> GraphRouter<K> {
        GraphRouter::new(self.routes)
    }
}
