use grammers_client::media::{Document, Photo};
use grammers_client::peer::{Channel, Group, Peer, User};
use grammers_client::tl::enums::{Document as DocumentEnum, Photo as PhotoEnum, PhotoSize};

use crate::telegram::file_id::FileId;

pub fn satori_link_from_tg_user_photo(self_id: i64, user: &User) -> Option<String> {
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        FileId::user_photo_big(user, user.photo()?).encode(),
    ))
}

pub fn satori_link_from_tg_group_photo(self_id: i64, group: &Group) -> Option<String> {
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        FileId::group_photo_big(group, group.photo()?).encode(),
    ))
}

pub fn satori_link_from_tg_channel_photo(self_id: i64, channel: &Channel) -> Option<String> {
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        FileId::channel_photo_big(channel, channel.photo()?).encode(),
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
    let size = sizes.iter().max_by_key(|size| size.1)?.0.chars().next()?;
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        FileId::photo(raw, size).encode(),
    ))
}

pub fn satori_link_from_tg_document(self_id: i64, document: &Document) -> Option<String> {
    let raw = match &document.raw.document {
        Some(DocumentEnum::Document(doc)) => doc,
        _ => return None,
    };
    Some(format!(
        "internal:telegram/{}/{}",
        self_id,
        FileId::document(raw).encode(),
    ))
}
