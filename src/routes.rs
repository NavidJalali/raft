use crate::{
    key_value_command::KeyValueCommand,
    kv_app::Store,
    message::{LocalClientToNodeMessage, Message, NodeToNodeMessage, Outcome},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use tower_http::trace::TraceLayer;
use tracing::info;

pub fn make_router<S: AppState<KeyValueCommand>>(state: S) -> Router {
    Router::new()
        .route("/raft", post(raft_handler::<S>))
        .route("/store/:key", get(get_handler::<S>))
        .route("/store/:key", post(post_handler::<S>))
        .route("/store/:key", delete(delete_handler::<S>))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn raft_handler<S: AppState<KeyValueCommand>>(
    state: State<S>,
    Json(body): Json<NodeToNodeMessage<KeyValueCommand>>,
) -> String {
    let local_node_ref = state.local_node();
    info!("Received message: {:?}", body);
    local_node_ref.offer(Message::NodeToNode(body));
    "OK".to_string()
}

async fn get_handler<S: AppState<KeyValueCommand>>(
    state: State<S>,
    Path(key): Path<String>,
) -> (StatusCode, String) {
    let app = state.app();
    match app.get(key).await {
        Some(value) => (StatusCode::OK, value),
        None => (StatusCode::NOT_FOUND, "Key not found".to_string()),
    }
}

async fn post_handler<S: AppState<KeyValueCommand>>(
    state: State<S>,
    Path(key): Path<String>,
    Json(value): Json<String>,
) -> String {
    let node_ref = state.local_node();
    // make oneshot
    let (tx, rx) = tokio::sync::oneshot::channel();
    node_ref.offer(Message::LocalClientToNode(
        LocalClientToNodeMessage::Broadcast {
            entry: KeyValueCommand::Put(key.clone(), value),
            on_commit: tx,
        },
    ));

    // wait for response
    match rx.await {
        Ok(Outcome::Success) => "OK".to_string(),
        Ok(Outcome::Failure(reason)) => format!("Error: {}", reason),
        Err(_) => "Error: Channel closed".to_string(),
    }
}

async fn delete_handler<S: AppState<KeyValueCommand>>(
    state: State<S>,
    Path(key): Path<String>,
) -> (StatusCode, String) {
    let node_ref = state.local_node();
    // make oneshot
    let (tx, rx) = tokio::sync::oneshot::channel();
    node_ref.offer(Message::LocalClientToNode(
        LocalClientToNodeMessage::Broadcast {
            entry: KeyValueCommand::Delete(key.clone()),
            on_commit: tx,
        },
    ));

    // wait for response
    match rx.await {
        Ok(Outcome::Success) => (StatusCode::OK, "OK".to_string()),
        Ok(Outcome::Failure(reason)) => (StatusCode::INTERNAL_SERVER_ERROR, reason),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Channel closed".to_string(),
        ),
    }
}
