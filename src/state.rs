use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use grammers_client::Client;
use ntex::server::ServerAppConfig;
use ntex::web;
use tokio::sync::Mutex;

use crate::event_publisher::EventPublisher;
use crate::self_info_cache::SelfInfoCache;
use crate::settings::Settings;

#[derive(Debug)]
pub struct AppErrors;

pub struct SessionName {
    pub session: String,
    pub files_dir: String,
    pub files_dir_path: PathBuf,
}

impl SessionName {
    pub fn new(session_name: String) -> Self {
        let files_dir = format!("files_{}", session_name);
        let files_dir_path = PathBuf::from(&files_dir);
        Self {
            session: session_name,
            files_dir,
            files_dir_path,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub client: Client,
    pub self_info_cache: Arc<Mutex<SelfInfoCache>>,
    pub event_publisher: Arc<EventPublisher>,
    pub names: Arc<SessionName>,
}

impl web::State for AppState {
    type Error = AppErrors;
}

pub struct AppStateBuilder(AppState);

impl AppStateBuilder {
    pub fn new(
        settings: Arc<Settings>,
        client: Client,
        self_info_cache: Arc<Mutex<SelfInfoCache>>,
        event_publisher: Arc<EventPublisher>,
        names: Arc<SessionName>,
    ) -> Self {
        Self(AppState {
            settings,
            client,
            self_info_cache,
            event_publisher,
            names,
        })
    }
}

impl ServerAppConfig for AppStateBuilder {
    type State = AppState;

    async fn create(&self) -> io::Result<Self::State> {
        Ok(self.0.clone())
    }
}
