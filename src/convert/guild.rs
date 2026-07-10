use grammers_client::peer::Peer;
use presence_rs::Presence;

use crate::convert::link::{satori_link_from_tg_channel_photo, satori_link_from_tg_group_photo};
use crate::satori::types::{Guild, provide};

pub fn satori_guild_from_tg_peer(self_id: i64, peer: &Peer) -> Option<Guild> {
    match peer {
        Peer::User(_) => None,
        Peer::Group(group) => Some(Guild {
            id: group
                .id()
                .bot_api_dialog_id()
                .unwrap_or_default()
                .to_string(),
            name: Presence::Some(group.title().unwrap_or_default().to_string()),
            avatar: provide(satori_link_from_tg_group_photo(self_id, group)),
        }),
        Peer::Channel(channel) => Some(Guild {
            id: channel
                .id()
                .bot_api_dialog_id()
                .unwrap_or_default()
                .to_string(),
            name: Presence::Some(channel.title().to_string()),
            avatar: provide(satori_link_from_tg_channel_photo(self_id, channel)),
        }),
    }
}
