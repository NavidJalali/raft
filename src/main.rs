use config::Config;

use raft_node::RaftNode;
use time_window::TimeWindow;

mod cluster;
mod config;
mod key_value_command;
mod kv_app;
mod local_cluster;
mod local_node_ref;
mod log_entry;
mod message;
mod node_id;
mod node_role;
mod node_state;
mod raft_node;
mod remote_node_ref;
mod routes;
mod state;
mod static_cluster;
mod term;
mod time_window;

use node_id::NodeId;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let config = Config {
        election_time_window: TimeWindow::new(
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(10),
        ),
        heartbeat_time_window: TimeWindow::new(
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(2),
        ),
    };

    let remote_node_1 =
        remote_node_ref::RemoteNodeRef::new(NodeId(1), "localhost".to_string(), 8080);

    let remote_node_2 =
        remote_node_ref::RemoteNodeRef::new(NodeId(2), "localhost".to_string(), 8081);

    let remote_node_3 =
        remote_node_ref::RemoteNodeRef::new(NodeId(3), "localhost".to_string(), 8082);

    let static_cluster = static_cluster::StaticCluster::new(
        vec![
            (NodeId(1), remote_node_1),
            (NodeId(2), remote_node_2),
            (NodeId(3), remote_node_3),
        ]
        .into_iter()
        .collect(),
    );

    // parse node id from command line
    let node_id = NodeId(
        std::env::args()
            .nth(1)
            .expect("node id required")
            .parse::<u32>()
            .expect("node id must be a number"),
    );

    let kv_app = kv_app::KVApp::new().await;

    let kv_app_sender = kv_app.sender.clone();

    let local_node_ref = RaftNode::make_fresh(
        node_id,
        config.clone(),
        static_cluster.clone(),
        kv_app_sender,
    )
    .await;

    let state = state::LiveState {
        cluster: static_cluster,
        local_node: local_node_ref,
        app: kv_app,
    };

    let router = routes::make_router(state.clone());

    let self_remote_ref = state.cluster.nodes.get(&node_id).unwrap().0.clone();
    let host = self_remote_ref.host.clone();
    let port = self_remote_ref.port;
    let url = format!("{}:{}", host, port);

    let listener = TcpListener::bind(url).await.unwrap();
    axum::serve(listener, router.into_make_service())
        .await
        .unwrap();
}
