use grammers_client::peer::Peer;
use presence_rs::Presence;

use crate::convert::link::{satori_link_from_tg_peer_photo, satori_link_from_tg_user_photo};
use crate::satori::types::{User, provide};

pub fn satori_user_from_tg_user(self_id: i64, user: &grammers_client::peer::User) -> User {
    User {
        id: user
            .id()
            .bot_api_dialog_id()
            .unwrap_or_default()
            .to_string(),
        name: provide(user.username().map(|s| s.to_string())),
        nick: Presence::Some(user.full_name()),
        avatar: provide(satori_link_from_tg_user_photo(self_id, user)),
        is_bot: Presence::Some(user.is_bot()),
    }
}

pub fn satori_user_from_tg_peer(self_id: i64, peer: &Peer) -> User {
    // TODO only channels/supergroups can act like users, not (regular) groups
    User {
        id: peer
            .id()
            .bot_api_dialog_id()
            .unwrap_or_default()
            .to_string(),
        name: provide(peer.username().map(|s| s.to_string())),
        nick: match peer {
            Peer::User(user) => Presence::Some(user.full_name()),
            Peer::Group(group) => Presence::Some(group.title().unwrap_or_default().to_string()),
            Peer::Channel(channel) => Presence::Some(channel.title().to_string()),
        },
        avatar: provide(satori_link_from_tg_peer_photo(self_id, peer)),
        is_bot: match peer {
            Peer::User(user) => Presence::Some(user.is_bot()),
            _ => Presence::Some(false),
        },
    }
}
