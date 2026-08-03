//! `WebUI` gathers data from fuzzers and show stats to users through a web interface
//!
//! Vibe-coding WARNING!! I fully vibe coded the frontend part with claude code Opus 4.7 since I know nothing about js and css.
//! But we are always looking for somebody who can help us design a better & maintainable beautiful web UI!

use crate::{
    Result,
    controllers::Controller,
    controllers::Descriptor,
    monitors::Monitor,
    states::{Stats, read_stats_json},
};
use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::net::SocketAddr;
use libaflmm_bolts::current_time;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::RwLock,
};

const FRONTEND_HTML: &str = include_str!("frontend/index.html");
const HISTORY_LIMIT: usize = 100;
const NAME_PLACEHOLDER: &str = "__MONITOR_NAME__";

#[derive(Serialize, Deserialize)]
struct Snapshot {
    timestamp_secs: u64,
    stats: Vec<Stats>,
}

#[derive(Debug)]
struct SharedState {
    history: Vec<Value>,
}

pub const WEBUI_PREFIX: &str = "libaflmm-webui";
const DEFAULT_PORT: u16 = 13337;

#[derive(Debug)]
struct WebMonitorConfig {
    port: u16,
    html: String,
}

/// `WebUI` gathers data from fuzzers and show stats to users through a web interface
#[derive(Debug)]
pub struct WebMonitor {
    history_path: PathBuf,
    shared: Arc<RwLock<SharedState>>,
    config: Option<WebMonitorConfig>,
}

impl WebMonitor {
    /// constructor for [`struct@WebMonitor`]; `name` is displayed as the page title.
    #[must_use]
    pub fn new<CT: Controller>(name: &str, controller: &CT) -> Self {
        Self::with_port(name, DEFAULT_PORT, controller)
    }

    /// constructor for [`struct@WebMonitor`] specifying an opening port
    #[must_use]
    pub fn with_port<CT: Controller>(name: &str, port: u16, controller: &CT) -> Self {
        let root_dir = controller.root_dir();

        let root_dir = if root_dir.is_absolute() {
            root_dir.to_path_buf()
        } else {
            let cwd = std::env::current_dir().unwrap();
            cwd.join(root_dir)
        };

        let filename = format!("{WEBUI_PREFIX}.json");
        let history_path = root_dir.join(filename);
        let _ = std::fs::remove_file(&history_path);
        let shared = Arc::new(RwLock::new(SharedState {
            history: Vec::new(),
        }));
        let html = FRONTEND_HTML.replace(NAME_PLACEHOLDER, name);

        WebMonitor {
            history_path,
            shared,
            config: Some(WebMonitorConfig { port, html }),
        }
    }

    fn append_to_file(path: &Path, json: &str) {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{json}");
        }
    }

    fn start_server(&mut self) {
        if let Some(WebMonitorConfig { port, html }) = self.config.take() {
            let shared = self.shared.clone();
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build tokio runtime for WebMonitor")
                    .block_on(serve(shared, port, html));
            });
        }
    }
}

async fn serve(shared: Arc<RwLock<SharedState>>, port: u16, html: String) {
    use axum::{Json, Router, extract::State, response::Html, routing::get};

    #[derive(Clone)]
    struct AppState {
        shared: Arc<RwLock<SharedState>>,
        html: Arc<str>,
    }

    async fn root(State(s): State<AppState>) -> Html<String> {
        Html(s.html.to_string())
    }

    async fn history(State(s): State<AppState>) -> Json<Value> {
        Json(Value::Array(s.shared.read().unwrap().history.clone()))
    }

    let state = AppState {
        shared,
        html: Arc::from(html),
    };
    let app = Router::new()
        .route("/", get(root))
        .route("/api/history", get(history))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("WebMonitor failed to bind");
    log::info!("WebMonitor listening on http://127.0.0.1:{port}");
    println!("WebMonitor listening on http://127.0.0.1:{port}");
    axum::serve(listener, app).await.unwrap();
}

impl Monitor for WebMonitor {
    fn start<CT: Controller>(&mut self, _controller: &mut CT) -> Result<()> {
        self.start_server();
        Ok(())
    }

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
