use grammers_client::Client;
use grammers_client::peer::Peer;
use grammers_session::types::PeerId;
use ntex::http::StatusCode;

use crate::error::WebError;
use crate::satori::types::{Channel, ChannelType};
use crate::telegram::peer_id_from_bot_api_id;

pub fn satori_channel_from_tg_peer(peer: &Peer, thread_id: Option<i32>) -> Channel {
    let chat_id = peer.id().bot_api_dialog_id();
    Channel {
        id: match thread_id {
            Some(thread_id) => format!("{}:{}", chat_id, thread_id),
            None => chat_id.to_string(),
        },
        channel_type: match peer {
            Peer::User(_) => ChannelType::Direct,
            _ => ChannelType::Text,
        },
        name: match peer {
            Peer::User(user) => Some(user.full_name()),
            Peer::Group(group) => group.title().map(|s| s.to_string()),
            Peer::Channel(channel) => Some(channel.title().to_string()),
        },
        parent_id: None,
    }
}

pub async fn tg_peer_id_from_satori_channel_id(
    client: &Client,
    channel_id: &str,
) -> Result<(PeerId, Option<i32>), WebError> {
    let (peer_id, thread_id) = if let Some((peer_id, thread_id)) = channel_id.split_once(":") {
        (peer_id, Some(thread_id))
    } else {
        (channel_id, None)
    };
    let peer_id = if let Ok(id) = peer_id.parse::<i64>() {
        peer_id_from_bot_api_id(id)
            .ok_or_else(|| WebError::new(StatusCode::BAD_REQUEST, "Peer ID invalid.".to_string()))?
    } else {
        client
            .resolve_username(peer_id)
            .await?
            .ok_or_else(|| {
                WebError::new(StatusCode::BAD_REQUEST, "Username not found.".to_string())
            })?
            .id()
    };
    let thread_id = if let Some(thread_id) = thread_id {
        Some(thread_id.parse::<i32>().map_err(|_| {
            WebError::new(StatusCode::BAD_REQUEST, "Thread ID invalid.".to_string())
        })?)
    } else {
        None
    };
    Ok((peer_id, thread_id))
}
