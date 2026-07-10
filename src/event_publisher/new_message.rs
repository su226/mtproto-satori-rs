use std::sync::Arc;
use std::time::{Duration, Instant};

use grammers_client::update::Message;
use log::{debug, warn};
use ntex::time::sleep;
use presence_rs::Presence;

use crate::convert::event::{satori_event_from_tg_message, satori_event_from_tg_messages};
use crate::event_publisher::{EventPublisher, MediaGroup};

impl EventPublisher {
    pub async fn handle_new_message(self: Arc<Self>, message: &Message) {
        if self.settings.merge_media_group.receive > 0
            && let Some(group_id) = message.grouped_id()
        {
            let now = {
                let mut media_groups = self.media_groups.lock().unwrap();
                let mut messages = if let Some(group) = media_groups.remove(&group_id) {
                    group.messages
                } else {
                    Vec::new()
                };
                let now = Instant::now();
                messages.push(message.clone());
                media_groups.insert(
                    group_id,
                    MediaGroup {
                        time: now,
                        messages,
                    },
                );
                now
            };
            sleep(Duration::from_millis(
                self.settings.merge_media_group.receive,
            ))
            .await;
            let group = {
                let mut media_groups = self.media_groups.lock().unwrap();
                let group = match media_groups.get(&group_id) {
                    Some(group) => group,
                    None => return,
                };
                if group.time != now {
                    return;
                }
                match media_groups.remove(&group_id) {
                    Some(group) => group,
                    None => return,
                }
            };
            let mut messages = group.messages;
            messages.sort_by_key(|m| m.id());
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
            let contents = messages.iter().map(|x| &**x).collect::<Vec<_>>();
            let event = satori_event_from_tg_messages(&login, contents.as_slice());
            debug!("Received message: {:#?} => {:#?}", messages, event);
            self.send(event);
        } else {
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
            let event = satori_event_from_tg_message(&login, message);
            if let Presence::Some(message) = &event.message
                && let Presence::Some(content) = &message.content
            {
                debug!("Received message: {} {:?}", event.format_session(), content);
            } else {
                debug!("Received message: {} ???", event.format_session());
            }
            self.send(event);
        }
    }
}
