use std::sync::Arc;
use std::time::{Duration, Instant};

use grammers_client::Client;
use grammers_client::peer::User;
use grammers_session::types::PeerId;
use log::debug;

pub struct SelfInfoCache {
    info: User,
    client: Arc<Client>,
    last_updated: Option<Instant>,
}

impl SelfInfoCache {
    pub fn new(info: User, client: Arc<Client>) -> Self {
        Self {
            info,
            client,
            last_updated: Some(Instant::now()),
        }
    }

    fn should_update(&self) -> bool {
        if let Some(instant) = self.last_updated {
            instant < Instant::now() - Duration::from_mins(1)
        } else {
            true
        }
    }

    pub async fn get(&mut self) -> grammers_client::Result<User> {
        if self.should_update() {
            debug!("Self info expired, updating.");
            self.info = self.client.get_me().await?;
            self.last_updated = Some(Instant::now());
        }
        Ok(self.info.clone())
    }

    pub fn invalidate(&mut self) {
        self.last_updated = None;
    }

    pub fn get_id(&self) -> PeerId {
        self.info.id()
    }
}
