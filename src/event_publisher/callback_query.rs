use std::sync::Arc;

use grammers_client::update::CallbackQuery;
use log::{debug, warn};
use presence_rs::Presence;

use crate::convert::event::satori_event_from_tg_callback;
use crate::event_publisher::EventPublisher;

impl EventPublisher {
    pub async fn handle_callback_query(self: Arc<Self>, callback: &CallbackQuery) {
        let mut self_info_cache = self.self_info_cache.lock().await;
        let login = match self_info_cache.get().await {
            Ok(user) => user,
            Err(err) => {
                warn!(
                    "Failed to get login info, event won't be published: {:?}",
                    err
                );
                return;
            }
        };
        drop(self_info_cache);
        if let Err(err) = callback.answer().send().await {
            warn!("Failed to answer callback query: {:?}", err);
        }
        let message = match callback.load_message().await {
            Ok(message) => message,
            Err(err) => {
                warn!("Failed to get callback query message: {:?}", err);
                return;
            }
        };
        let event = satori_event_from_tg_callback(&login, callback, &message);
        if let Presence::Some(button) = &event.button {
            debug!(
                "Received callback: {} {:?}",
                event.format_session(),
                button.id
            );
        } else {
            debug!("Received callback: {} ???", event.format_session());
        }
        self.send(event);
    }
}
