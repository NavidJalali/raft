use crate::{
    cluster::Cluster,
    kv_app::{KVApp, Store},
    local_node_ref::LocalNodeRef,
    static_cluster::StaticCluster,
};

pub trait AppState<A: Clone + Eq>: Clone + Send + Sync + 'static {
    type ClusterType: Cluster<A> + Send + Sync;
    type App: Store + Clone + Send + Sync + 'static;

    fn cluster(&self) -> Self::ClusterType;
    fn local_node(&self) -> LocalNodeRef<A>;
    fn app(&self) -> Self::App;
}

#[derive(Clone)]
pub struct LiveState<A: Clone + Eq> {
    pub cluster: StaticCluster<A>,
    pub local_node: LocalNodeRef<A>,
    pub app: KVApp,
}

impl<A: Clone + Eq + Send + Sync + 'static + std::fmt::Debug> AppState<A> for LiveState<A> {
    type ClusterType = StaticCluster<A>;
    type App = KVApp;

    fn cluster(&self) -> StaticCluster<A> {
        self.cluster.clone()
    }

    fn local_node(&self) -> LocalNodeRef<A> {
        self.local_node.clone()
    }

    fn app(&self) -> KVApp {
        self.app.clone()
    }
}
