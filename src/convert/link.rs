use grammers_client::media::{Document, Photo};
use grammers_client::peer::{Channel, Group, Peer, User};
use grammers_client::tl::enums::{
    Chat,
    Document as DocumentEnum,
    Photo as PhotoEnum,
    PhotoSize,
    User as UserEnum,
};

use crate::telegram::file_id::FileId;

pub fn satori_link_from_tg_user_photo(self_id: i64, user: &User) -> Option<String> {
    let photo = user.photo()?;
    let access_hash = match &user.raw {
        UserEnum::Empty(_) => 0,
        UserEnum::User(user) => user.access_hash.unwrap_or(0),
    };
    let file_id = FileId::PeerPhoto {
        peer_id: user.id().bot_api_dialog_id(),
        access_hash,
        photo_id: photo.photo_id,
    };
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        file_id.encode(),
    ))
}

pub fn satori_link_from_tg_group_photo(self_id: i64, group: &Group) -> Option<String> {
    let photo = group.photo()?;
    let access_hash = match &group.raw {
        Chat::Empty(_) => 0,
        Chat::Chat(_) => 0,
        Chat::Forbidden(_) => 0,
        Chat::Channel(channel) => channel.access_hash.unwrap_or(0),
        Chat::ChannelForbidden(forbidden) => forbidden.access_hash,
    };
    let file_id = FileId::PeerPhoto {
        peer_id: group.id().bot_api_dialog_id(),
        access_hash,
        photo_id: photo.photo_id,
    };
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        file_id.encode(),
    ))
}

pub fn satori_link_from_tg_channel_photo(self_id: i64, channel: &Channel) -> Option<String> {
    let photo = channel.photo()?;
    let access_hash = channel.raw.access_hash.unwrap_or(0);
    let file_id = FileId::PeerPhoto {
        peer_id: channel.id().bot_api_dialog_id(),
        access_hash,
        photo_id: photo.photo_id,
    };
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        file_id.encode(),
    ))
}

pub fn satori_link_from_tg_peer_photo(self_id: i64, peer: &Peer) -> Option<String> {
    match peer {
        Peer::User(user) => satori_link_from_tg_user_photo(self_id, user),
        Peer::Group(group) => satori_link_from_tg_group_photo(self_id, group),
        Peer::Channel(channel) => satori_link_from_tg_channel_photo(self_id, channel),
    }
}

pub fn satori_link_from_tg_photo(self_id: i64, photo: &Photo) -> Option<String> {
    let raw = match &photo.raw.photo {
        Some(PhotoEnum::Photo(photo)) => photo,
        _ => return None,
    };
    let mut sizes = Vec::new();
    for size in &raw.sizes {
        match size {
            PhotoSize::Size(size) => sizes.push((&size.r#type, size.w * size.h)),
            PhotoSize::Progressive(size) => sizes.push((&size.r#type, size.w * size.h)),
            _ => {}
        }
    }
    sizes.sort_by_key(|size| size.1);
    let size = sizes.last()?;
    let file_id = FileId::Photo {
        id: raw.id,
        access_hash: raw.access_hash,
        file_reference: raw.file_reference.clone(),
        thumb_size: size.0.as_bytes().to_vec(),
    };
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        file_id.encode(),
    ))
}

pub fn satori_link_from_tg_document(self_id: i64, document: &Document) -> Option<String> {
    let raw = match &document.raw.document {
        Some(DocumentEnum::Document(doc)) => doc,
        _ => return None,
    };
    let file_id = FileId::Document {
        id: raw.id,
        access_hash: raw.access_hash,
        file_reference: raw.file_reference.clone(),
    };
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        file_id.encode(),
    ))
}
