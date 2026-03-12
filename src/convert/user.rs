use grammers_client::peer::Peer;

use crate::{
    convert::link::{satori_link_from_tg_peer_photo, satori_link_from_tg_user_photo},
    satori::types::User,
};

pub fn satori_user_from_tg_user(self_id: i64, user: &grammers_client::peer::User) -> User {
    User {
        id: user.id().bot_api_dialog_id().to_string(),
        name: user.username().map(|s| s.to_string()),
        nick: Some(user.full_name()),
        avatar: satori_link_from_tg_user_photo(self_id, user),
        is_bot: Some(user.is_bot()),
    }
}

pub fn satori_user_from_tg_peer(self_id: i64, peer: &Peer) -> User {
    // TODO only channels/supergroups can act like users, not (regular) groups
    User {
        id: peer.id().bot_api_dialog_id().to_string(),
        name: peer.username().map(|s| s.to_string()),
        nick: match peer {
            Peer::User(user) => Some(user.full_name()),
            Peer::Group(group) => group.title().map(|x| x.to_string()),
            Peer::Channel(channel) => Some(channel.title().to_string()),
        },
        avatar: satori_link_from_tg_peer_photo(self_id, peer),
        is_bot: match peer {
            Peer::User(user) => Some(user.is_bot()),
            _ => Some(false),
        },
    }
}
