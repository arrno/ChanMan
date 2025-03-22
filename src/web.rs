use crate::chan_man::ChanMan;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

#[derive(Serialize, Deserialize)]
struct Response {
    message: String,
}

impl Response {
    pub fn new(message: String) -> Self {
        Self { message: message }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct PubPayload {
    topic: String,
    message: String,
}

#[tokio::main]
pub async fn serve() {
    let chanman = Arc::new(ChanMan::new());

    // ---- TEST ----
    let cm = chanman.clone();
    cm.subscribe("1".to_string(), "updates".to_string(), |msg| {
        println!("Web handler received: {}", msg)
    });
    cm.subscribe("2".to_string(), "updates".to_string(), |msg| {
        println!("Web handler received: {}", msg)
    });
    cm.unsubscribe("2".into(), "updates".into());
    // ---- TEST ----

    let app = Router::new()
        .route("/pub", post(publish))
        .route("/sub", get(subscribe))
        .with_state(chanman)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}

async fn publish(
    State(chanman): State<Arc<ChanMan>>,
    Json(payload): Json<PubPayload>,
) -> (StatusCode, Json<Response>) {
    chanman.publish(&payload.topic, &payload.message);
    (StatusCode::OK, Json(Response::new("Ok".into())))
}

// TODO
async fn subscribe() {
    // Generate session key
    // Open websocket, add callback handler to chanman topic
    // On socket close, unsubscribe
}
