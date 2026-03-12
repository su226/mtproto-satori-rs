use grammers_client::peer::Peer;

use crate::{
    convert::link::{satori_link_from_tg_channel_photo, satori_link_from_tg_group_photo},
    satori::types::Guild,
};

pub fn satori_guild_from_tg_peer(self_id: i64, peer: &Peer) -> Option<Guild> {
    match peer {
        Peer::User(_) => None,
        Peer::Group(group) => Some(Guild {
            id: group.id().bot_api_dialog_id().to_string(),
            name: group.title().map(|title| title.to_string()),
            avatar: satori_link_from_tg_group_photo(self_id, group),
        }),
        Peer::Channel(channel) => Some(Guild {
            id: channel.id().bot_api_dialog_id().to_string(),
            name: Some(channel.title().to_string()),
            avatar: satori_link_from_tg_channel_photo(self_id, channel),
        }),
    }
}
