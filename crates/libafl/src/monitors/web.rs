use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    net::SocketAddr,
    path::{PathBuf, Path},
    string::{String, ToString},
    sync::{Arc, RwLock},
    vec::Vec,
};

use libafl_bolts::current_time;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Controller, Descriptor, Result,
    monitors::Monitor,
    states::{Stats, read_stats_json},
};

const HISTORY_LIMIT: usize = 100;

// Only used for NDJSON serialization/deserialization — not stored in SharedState.
#[derive(Serialize, Deserialize)]
struct Snapshot {
    timestamp_secs: u64,
    stats: Vec<Stats>,
}

// serde_json::Value is Send + Sync, so SharedState is too.
struct SharedState {
    current: Value,
    history: Vec<Value>,
    frontend_path: PathBuf,
}

pub struct WebMonitor {
    history_path: PathBuf,
    shared: Arc<RwLock<SharedState>>,
}

impl WebMonitor {
    pub fn new(history_path: PathBuf, frontend_path: PathBuf) -> Self {
        Self::with_port(history_path, frontend_path, 13337)
    }

    pub fn with_port(history_path: PathBuf, frontend_path: PathBuf, port: u16) -> Self {
        let current = Value::Null;
        let shared = Arc::new(RwLock::new(SharedState {
            current,
            history: Vec::new(),
            frontend_path,
        }));

        let shared_clone = shared.clone();
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime for WebMonitor")
                .block_on(serve(shared_clone, port));
        });

        WebMonitor { history_path, shared }
    }

    fn append_to_file(path: &Path, json: &str) {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{json}");
        }
    }
}

async fn serve(shared: Arc<RwLock<SharedState>>, port: u16) {
    use axum::{Json, Router, extract::State, http::StatusCode, response::{Html, IntoResponse}, routing::get};

    async fn root(State(s): State<Arc<RwLock<SharedState>>>) -> impl IntoResponse {
        let path = s.read().unwrap().frontend_path.join("index.html");
        match tokio::task::spawn_blocking(move || std::fs::read_to_string(&path)).await {
            Ok(Ok(html)) => Html(html).into_response(),
            _ => (StatusCode::NOT_FOUND, "index.html not found").into_response(),
        }
    }

    async fn current_stats(State(s): State<Arc<RwLock<SharedState>>>) -> Json<Value> {
        Json(s.read().unwrap().current.clone())
    }

    async fn history(State(s): State<Arc<RwLock<SharedState>>>) -> Json<Value> {
        Json(Value::Array(s.read().unwrap().history.clone()))
    }

    let app = Router::new()
        .route("/", get(root))
        .route("/api/stats", get(current_stats))
        .route("/api/history", get(history))
        .with_state(shared);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("WebMonitor failed to bind");
    log::info!("WebMonitor listening on http://127.0.0.1:{port}");
    axum::serve(listener, app).await.unwrap();
}

impl Monitor for WebMonitor {
    fn display<CT: Controller>(&mut self, controller: &mut CT) -> Result<()> {
        let mut all_stats: Vec<Stats> = Vec::new();

        for desc in controller.worker_descriptors_mut() {
            if let Some(stats_file) = desc.workdir_mut().get_stats()? {
                all_stats.push(read_stats_json(stats_file)?);
            }
        }

        if all_stats.is_empty() {
            return Ok(());
        }

        let snapshot = Snapshot {
            timestamp_secs: current_time().as_secs(),
            stats: all_stats,
        };

        let snapshot_value = serde_json::to_value(&snapshot).unwrap_or(Value::Null);
        Self::append_to_file(&self.history_path, &snapshot_value.to_string());

        let current = snapshot_value
            .get("stats")
            .cloned()
            .unwrap_or(Value::Null);

        let mut state = self.shared.write().unwrap();
        state.current = current;
        state.history.push(snapshot_value);
        if state.history.len() > HISTORY_LIMIT {
            state.history.remove(0);
        }

        Ok(())
    }
}
