use crate::chan_man::ChanMan;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use tower_http::trace::TraceLayer;
use uuid;

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
    let chan_man = Arc::new(ChanMan::new());

    // ---- TEST ----
    let cm = chan_man.clone();
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
        .with_state(chan_man)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}

async fn publish(
    State(chan_man): State<Arc<ChanMan>>,
    Json(payload): Json<PubPayload>,
) -> (StatusCode, Json<Response>) {
    chan_man.publish(&payload.topic, &payload.message);
    (StatusCode::OK, Json(Response::new("Ok".into())))
}

async fn subscribe(
    ws: WebSocketUpgrade,
    State(chan_man): State<Arc<ChanMan>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, chan_man))
}

async fn handle_socket(socket: WebSocket, chan_man: Arc<ChanMan>) {
    let session_key = uuid::Uuid::new_v4().to_string();
    let mut topic = None;

    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Wait for the initial message to determine the topic
    if let Some(Ok(Message::Text(text))) = receiver.next().await {
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            if let Some(t) = json.get("subscribe").and_then(|v| v.as_str()) {
                topic = Some(t.to_string());
            }
        }
    }

    let topic = match topic {
        Some(t) => t,
        None => return, // Close the connection if no valid topic is received
    };

    let log_session = session_key.clone();
    // Spawn a separate task to handle outgoing messages
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                // Client likely disconnecte d
                break;
            }
        }
        println!("Session {} terminated.", &log_session);
    });

    let callback = move |msg: &str| {
        let msg = msg.to_string();
        let _ = tx.send(msg); // Send message to the channel (ignore errors)
    };

    chan_man.subscribe(session_key.clone(), topic.clone(), callback);

    while let Some(msg_result) = receiver.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                if let Ok(json) = serde_json::from_str::<Value>(&text) {
                    if let Some(t) = json.get("unsubscribe").and_then(|v| v.as_str()) {
                        if t == topic {
                            break;
                        }
                    }
                }
            }
            Ok(Message::Close(_)) | Err(_) => {
                // Client closed connection or an error occurred -> cleanup session
                break;
            }
            _ => {}
        }
    }

    chan_man.unsubscribe(session_key, topic);
}
