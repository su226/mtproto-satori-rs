use std::io;
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

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub client: Client,
    pub self_info_cache: Arc<Mutex<SelfInfoCache>>,
    pub event_publisher: Arc<EventPublisher>,
    pub session_name: Arc<String>,
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
        session_name: Arc<String>,
    ) -> Self {
        Self(AppState {
            settings,
            client,
            self_info_cache,
            event_publisher,
            session_name,
        })
    }
}

impl ServerAppConfig for AppStateBuilder {
    type State = AppState;

    async fn create(&self) -> io::Result<Self::State> {
        Ok(self.0.clone())
    }
}
