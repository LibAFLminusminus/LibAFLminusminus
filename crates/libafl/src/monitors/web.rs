//! WebUI gathers data from fuzzers and show stats to users through a web interface
use std::{
    fs::OpenOptions,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    vec::Vec,
};

use alloc::string::ToString;

use libafl_bolts::current_time;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Controller, Descriptor, Result,
    monitors::Monitor,
    states::{Stats, read_stats_json},
};

const FRONTEND_HTML: &str = include_str!("frontend/index.html");
const HISTORY_LIMIT: usize = 100;

// Only used for NDJSON serialization/deserialization — not stored in SharedState.
#[derive(Serialize, Deserialize)]
struct Snapshot {
    timestamp_secs: u64,
    stats: Vec<Stats>,
}

// serde_json::Value is Send + Sync, so SharedState is too.
#[derive(Debug)]
struct SharedState {
    history: Vec<Value>,
}

/// WebUI gathers data from fuzzers and show stats to users through a web interface
#[derive(Debug)]
pub struct WebMonitor {
    history_path: PathBuf,
    shared: Arc<RwLock<SharedState>>,
}

impl WebMonitor {
    /// constructor for [`struct@WebMonitor`]
    pub fn new(history_path: PathBuf) -> Self {
        Self::with_port(history_path, 13337)
    }

    /// constructor for [`struct@WebMonitor`] specifying an opening port
    pub fn with_port(history_path: PathBuf, port: u16) -> Self {
        let _ = std::fs::remove_file(&history_path);
        let shared = Arc::new(RwLock::new(SharedState {
            history: Vec::new(),
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
    use axum::{Json, Router, extract::State, response::Html, routing::get};

    async fn root() -> Html<&'static str> {
        Html(FRONTEND_HTML)
    }

    async fn history(State(s): State<Arc<RwLock<SharedState>>>) -> Json<Value> {
        Json(Value::Array(s.read().unwrap().history.clone()))
    }

    let app = Router::new()
        .route("/", get(root))
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

        let mut state = self.shared.write().unwrap();
        state.history.push(snapshot_value);
        if state.history.len() > HISTORY_LIMIT {
            state.history.remove(0);
        }

        Ok(())
    }
}
