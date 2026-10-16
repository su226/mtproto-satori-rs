//! Bot-API style file ID. Used by official bot API and Pyrogram.
//! Also supported in Telethon.

use std::iter::repeat_n;

use base64::prelude::*;
use grammers_client::media::Downloadable;
use grammers_client::peer::{Channel, Community, Group, User};
use grammers_client::tl::enums::{
    Chat,
    DocumentAttribute,
    InputFileLocation,
    InputStickerSet,
    User as UserEnum,
};
use grammers_client::tl::types::{
    ChatPhoto,
    Document,
    InputDocumentFileLocation,
    InputEncryptedFileLocation,
    InputPeerPhotoFileLocation,
    InputPhotoFileLocation,
    InputPhotoLegacyFileLocation,
    InputSecureFileLocation,
    InputStickerSetId,
    InputStickerSetThumb,
    Photo,
    UserProfilePhoto,
};
use grammers_client::tl::{Cursor, Deserializable, Serializable, deserialize};
use grammers_session::types::{PeerAuth, PeerId, PeerRef};

fn rle_encode(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut zero_count = 0;
    for byte in data {
        if *byte == 0 {
            zero_count += 1;
        } else {
            if zero_count > 0 {
                result.push(0);
                result.push(zero_count);
                zero_count = 0;
            }
            result.push(*byte);
        }
    }
    if zero_count > 0 {
        result.push(0);
        result.push(zero_count);
    }
    result
}

fn rle_decode(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut is_zero = false;
    for byte in data {
        if *byte == 0 {
            is_zero = true;
        } else if is_zero {
            result.extend(repeat_n(0, *byte as usize));
            is_zero = false;
        } else {
            result.push(*byte);
        }
    }
    result
}

const MAJOR: u8 = 4;
const MINOR: u8 = 61;
const MINOR_REMOVE_PHOTO_VOLUME_AND_LOCAL_ID: u8 = 32;
const MINOR_ADD_PHOTO_SIZE_SOURCE: u8 = 22;

enum ParserClass {
    Photo,
    Common,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u32)]
pub enum FileType {
    Thumbnail,
    ProfilePhoto,
    Photo,
    VoiceNote,
    Video,
    Document,
    Encrypted,
    Temp,
    Sticker,
    Audio,
    Animation,
    EncryptedThumbnail,
    Wallpaper,
    VideoNote,
    SecureDecrypted,
    SecureEncrypted,
    Background,
    DocumentAsFile,
    Ringtone,
    CallLog,
    PhotoStory,
    VideoStory,
    SelfDestructingPhoto,
    SelfDestructingVideo,
    SelfDestructingVideoNote,
    SelfDestructingVoiceNote,
    LivePhoto,
    SelfDestructingLivePhoto,
}

impl FileType {
    fn get_parser_class(self) -> ParserClass {
        match self {
            FileType::Photo => ParserClass::Photo,
            FileType::ProfilePhoto => ParserClass::Photo,
            FileType::Thumbnail => ParserClass::Photo,
            FileType::EncryptedThumbnail => ParserClass::Photo,
            FileType::Wallpaper => ParserClass::Photo,
            FileType::PhotoStory => ParserClass::Photo,
            FileType::SelfDestructingPhoto => ParserClass::Photo,
            FileType::Video => ParserClass::Common,
            FileType::VoiceNote => ParserClass::Common,
            FileType::Document => ParserClass::Common,
            FileType::Sticker => ParserClass::Common,
            FileType::Audio => ParserClass::Common,
            FileType::Animation => ParserClass::Common,
            FileType::VideoNote => ParserClass::Common,
            FileType::Background => ParserClass::Common,
            FileType::DocumentAsFile => ParserClass::Common,
            FileType::Ringtone => ParserClass::Common,
            FileType::CallLog => ParserClass::Common,
            FileType::VideoStory => ParserClass::Common,
            FileType::SelfDestructingVideo => ParserClass::Common,
            FileType::SelfDestructingVideoNote => ParserClass::Common,
            FileType::SelfDestructingVoiceNote => ParserClass::Common,
            FileType::LivePhoto => ParserClass::Common,
            FileType::SelfDestructingLivePhoto => ParserClass::Common,
            FileType::SecureDecrypted => ParserClass::Common,
            FileType::SecureEncrypted => ParserClass::Common,
            FileType::Encrypted => ParserClass::Common,
            FileType::Temp => ParserClass::None,
        }
    }
}

impl TryFrom<u32> for FileType {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Thumbnail),
            1 => Ok(Self::ProfilePhoto),
            2 => Ok(Self::Photo),
            3 => Ok(Self::VoiceNote),
            4 => Ok(Self::Video),
            5 => Ok(Self::Document),
            6 => Ok(Self::Encrypted),
            7 => Ok(Self::Temp),
            8 => Ok(Self::Sticker),
            9 => Ok(Self::Audio),
            10 => Ok(Self::Animation),
            11 => Ok(Self::EncryptedThumbnail),
            12 => Ok(Self::Wallpaper),
            13 => Ok(Self::VideoNote),
            14 => Ok(Self::SecureDecrypted),
            15 => Ok(Self::SecureEncrypted),
            16 => Ok(Self::Background),
            17 => Ok(Self::DocumentAsFile),
            18 => Ok(Self::Ringtone),
            19 => Ok(Self::CallLog),
            20 => Ok(Self::PhotoStory),
            21 => Ok(Self::VideoStory),
            22 => Ok(Self::SelfDestructingPhoto),
            23 => Ok(Self::SelfDestructingVideo),
            24 => Ok(Self::SelfDestructingVideoNote),
            25 => Ok(Self::SelfDestructingVoiceNote),
            26 => Ok(Self::LivePhoto),
            27 => Ok(Self::SelfDestructingLivePhoto),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub enum PhotoSizeSource {
    Legacy {
        secret: i64,
    },
    Thumbnail {
        file_type: FileType,
        thumbnail_type: u32,
    },
    DialogPhotoSmall {
        dialog_id: PeerId,
        dialog_access_hash: PeerAuth,
    },
    DialogPhotoBig {
        dialog_id: PeerId,
        dialog_access_hash: PeerAuth,
    },
    StickerSetThumbnail {
        sticker_set_id: i64,
        sticker_set_access_hash: i64,
    },
    FullLegacy {
        volume_id: i64,
        local_id: i32,
        secret: i64,
    },
    DialogPhotoSmallLegacy {
        dialog_id: PeerId,
        dialog_access_hash: PeerAuth,
        volume_id: i64,
        local_id: i32,
    },
    DialogPhotoBigLegacy {
        dialog_id: PeerId,
        dialog_access_hash: PeerAuth,
        volume_id: i64,
        local_id: i32,
    },
    StickerSetThumbnailLegacy {
        sticker_set_id: i64,
        sticker_set_access_hash: i64,
        volume_id: i64,
        local_id: i32,
    },
    StickerSetThumbnailVersion {
        sticker_set_id: i64,
        sticker_set_access_hash: i64,
        version: i32,
    },
}

impl PhotoSizeSource {
    fn get_file_type(&self) -> FileType {
        match self {
            Self::Thumbnail { file_type, .. } => *file_type,
            Self::DialogPhotoSmall { .. }
            | Self::DialogPhotoBig { .. }
            | Self::DialogPhotoSmallLegacy { .. }
            | Self::DialogPhotoBigLegacy { .. } => FileType::ProfilePhoto,
            Self::StickerSetThumbnail { .. }
            | Self::StickerSetThumbnailLegacy { .. }
            | Self::StickerSetThumbnailVersion { .. } => FileType::Thumbnail,
            Self::Legacy { .. } | Self::FullLegacy { .. } => unreachable!(),
        }
    }
}

impl Deserializable for PhotoSizeSource {
    fn deserialize(buf: &'_ mut Cursor<'_>) -> deserialize::Result<Self>
    where
        Self: std::marker::Sized,
    {
        let variant = i32::deserialize(buf)?;
        match variant {
            0 => {
                let secret = i64::deserialize(buf)?;
                Ok(Self::Legacy { secret })
            }
            1 => {
                let file_type = u32::deserialize(buf)?;
                let file_type = FileType::try_from(file_type)
                    .map_err(|_| deserialize::Error::UnexpectedConstructor { id: file_type })?;
                let thumbnail_type = u32::deserialize(buf)?;
                Ok(Self::Thumbnail {
                    file_type,
                    thumbnail_type,
                })
            }
            2 => {
                let dialog_id = i64::deserialize(buf)?;
                let dialog_id = PeerId::from_bot_api_dialog_id(dialog_id)
                    .ok_or(deserialize::Error::UnexpectedConstructor { id: variant as u32 })?;
                let dialog_access_hash = i64::deserialize(buf)?;
                let dialog_access_hash = PeerAuth::from_hash(dialog_access_hash);
                Ok(Self::DialogPhotoSmall {
                    dialog_id,
                    dialog_access_hash,
                })
            }
            3 => {
                let dialog_id = i64::deserialize(buf)?;
                let dialog_id = PeerId::from_bot_api_dialog_id(dialog_id)
                    .ok_or(deserialize::Error::UnexpectedConstructor { id: variant as u32 })?;
                let dialog_access_hash = i64::deserialize(buf)?;
                let dialog_access_hash = PeerAuth::from_hash(dialog_access_hash);
                Ok(Self::DialogPhotoBig {
                    dialog_id,
                    dialog_access_hash,
                })
            }
            4 => {
                let sticker_set_id = i64::deserialize(buf)?;
                let sticker_set_access_hash = i64::deserialize(buf)?;
                Ok(Self::StickerSetThumbnail {
                    sticker_set_id,
                    sticker_set_access_hash,
                })
            }
            5 => {
                let volume_id = i64::deserialize(buf)?;
                let secret = i64::deserialize(buf)?;
                let local_id = i32::deserialize(buf)?;
                Ok(Self::FullLegacy {
                    volume_id,
                    secret,
                    local_id,
                })
            }
            6 => {
                let dialog_id = i64::deserialize(buf)?;
                let dialog_id = PeerId::from_bot_api_dialog_id(dialog_id)
                    .ok_or(deserialize::Error::UnexpectedConstructor { id: 0 })?;
                let dialog_access_hash = i64::deserialize(buf)?;
                let dialog_access_hash = PeerAuth::from_hash(dialog_access_hash);
                let volume_id = i64::deserialize(buf)?;
                let local_id = i32::deserialize(buf)?;
                if local_id < 0 {
                    return Err(deserialize::Error::UnexpectedConstructor { id: variant as u32 });
                }
                Ok(Self::DialogPhotoSmallLegacy {
                    dialog_id,
                    dialog_access_hash,
                    volume_id,
                    local_id,
                })
            }
            7 => {
                let dialog_id = i64::deserialize(buf)?;
                let dialog_id = PeerId::from_bot_api_dialog_id(dialog_id)
                    .ok_or(deserialize::Error::UnexpectedConstructor { id: 0 })?;
                let dialog_access_hash = i64::deserialize(buf)?;
                let dialog_access_hash = PeerAuth::from_hash(dialog_access_hash);
                let volume_id = i64::deserialize(buf)?;
                let local_id = i32::deserialize(buf)?;
                if local_id < 0 {
                    return Err(deserialize::Error::UnexpectedConstructor { id: variant as u32 });
                }
                Ok(Self::DialogPhotoBigLegacy {
                    dialog_id,
                    dialog_access_hash,
                    volume_id,
                    local_id,
                })
            }
            8 => {
                let sticker_set_id = i64::deserialize(buf)?;
                let sticker_set_access_hash = i64::deserialize(buf)?;
                let volume_id = i64::deserialize(buf)?;
                let local_id = i32::deserialize(buf)?;
                if local_id < 0 {
                    return Err(deserialize::Error::UnexpectedConstructor { id: variant as u32 });
                }
                Ok(Self::StickerSetThumbnailLegacy {
                    sticker_set_id,
                    sticker_set_access_hash,
                    volume_id,
                    local_id,
                })
            }
            9 => {
                let sticker_set_id = i64::deserialize(buf)?;
                let sticker_set_access_hash = i64::deserialize(buf)?;
                let version = i32::deserialize(buf)?;
                Ok(Self::StickerSetThumbnailVersion {
                    sticker_set_id,
                    sticker_set_access_hash,
                    version,
                })
            }
            _ => Err(deserialize::Error::UnexpectedConstructor { id: variant as u32 }),
        }
    }
}

impl Serializable for PhotoSizeSource {
    fn serialize(&self, buf: &mut impl Extend<u8>) {
        match self {
            PhotoSizeSource::Legacy { secret } => {
                0i32.serialize(buf);
                secret.serialize(buf);
            }
            PhotoSizeSource::Thumbnail {
                file_type,
                thumbnail_type,
            } => {
                1i32.serialize(buf);
                (*file_type as u32).serialize(buf);
                thumbnail_type.serialize(buf);
            }
            PhotoSizeSource::DialogPhotoSmall {
                dialog_id,
                dialog_access_hash,
            } => {
                2i32.serialize(buf);
                dialog_id
                    .bot_api_dialog_id()
                    .unwrap_or_default()
                    .serialize(buf);
                dialog_access_hash.hash().serialize(buf);
            }
            PhotoSizeSource::DialogPhotoBig {
                dialog_id,
                dialog_access_hash,
            } => {
                3i32.serialize(buf);
                dialog_id
                    .bot_api_dialog_id()
                    .unwrap_or_default()
                    .serialize(buf);
                dialog_access_hash.hash().serialize(buf);
            }
            PhotoSizeSource::StickerSetThumbnail {
                sticker_set_id,
                sticker_set_access_hash,
            } => {
                4i32.serialize(buf);
                sticker_set_id.serialize(buf);
                sticker_set_access_hash.serialize(buf);
            }
            PhotoSizeSource::FullLegacy {
                volume_id,
                local_id,
                secret,
            } => {
                5i32.serialize(buf);
                volume_id.serialize(buf);
                secret.serialize(buf);
                local_id.serialize(buf);
            }
            PhotoSizeSource::DialogPhotoSmallLegacy {
                dialog_id,
                dialog_access_hash,
                volume_id,
                local_id,
            } => {
                6i32.serialize(buf);
                dialog_id
                    .bot_api_dialog_id()
                    .unwrap_or_default()
                    .serialize(buf);
                dialog_access_hash.hash().serialize(buf);
                volume_id.serialize(buf);
                local_id.serialize(buf);
            }
            PhotoSizeSource::DialogPhotoBigLegacy {
                dialog_id,
                dialog_access_hash,
                volume_id,
                local_id,
            } => {
                7i32.serialize(buf);
                dialog_id
                    .bot_api_dialog_id()
                    .unwrap_or_default()
                    .serialize(buf);
                dialog_access_hash.hash().serialize(buf);
                volume_id.serialize(buf);
                local_id.serialize(buf);
            }
            PhotoSizeSource::StickerSetThumbnailLegacy {
                sticker_set_id,
                sticker_set_access_hash,
                volume_id,
                local_id,
            } => {
                8i32.serialize(buf);
                sticker_set_id.serialize(buf);
                sticker_set_access_hash.serialize(buf);
                volume_id.serialize(buf);
                local_id.serialize(buf);
            }
            PhotoSizeSource::StickerSetThumbnailVersion {
                sticker_set_id,
                sticker_set_access_hash,
                version,
            } => {
                9i32.serialize(buf);
                sticker_set_id.serialize(buf);
                sticker_set_access_hash.serialize(buf);
                version.serialize(buf);
            }
        }
    }
}

#[derive(Debug)]
pub enum FileIdVariant {
    Web {
        url: String,
        access_hash: i64,
    },
    Photo {
        file_id: i64,
        access_hash: i64,
        source: PhotoSizeSource,
    },
    Common {
        file_id: i64,
        access_hash: i64,
    },
}

#[derive(Debug)]
pub struct FileId {
    pub major: u8,
    pub minor: u8,
    pub file_type: FileType,
    pub dc_id: i32,
    pub file_reference: Vec<u8>,
    pub variant: FileIdVariant,
}

const WEB_LOCATION_FLAG: u32 = 1 << 24;
const FILE_REFERENCE_FLAG: u32 = 1 << 25;

impl FileId {
    pub fn user_photo_big(user: &User, photo: &UserProfilePhoto) -> Self {
        let access_hash = match &user.raw {
            UserEnum::Empty(_) => 0,
            UserEnum::User(user) => user.access_hash.unwrap_or_default(),
        };
        let source = PhotoSizeSource::DialogPhotoBig {
            dialog_id: user.id(),
            dialog_access_hash: PeerAuth::from_hash(access_hash),
        };
        Self {
            major: MAJOR,
            minor: MINOR,
            file_type: source.get_file_type(),
            dc_id: photo.dc_id,
            file_reference: Vec::new(),
            variant: FileIdVariant::Photo {
                file_id: photo.photo_id,
                access_hash: 0,
                source,
            },
        }
    }

    pub fn group_photo_big(group: &Group, photo: &ChatPhoto) -> Self {
        let access_hash = match &group.raw {
            Chat::Empty(_) => 0,
            Chat::Chat(_) => 0,
            Chat::Forbidden(_) => 0,
            Chat::Channel(channel) => channel.access_hash.unwrap_or_default(),
            Chat::ChannelForbidden(forbidden) => forbidden.access_hash,
            Chat::Community(community) => community.access_hash.unwrap_or_default(),
            Chat::CommunityForbidden(forbidden) => forbidden.access_hash.unwrap_or_default(),
        };
        let source = PhotoSizeSource::DialogPhotoBig {
            dialog_id: group.id(),
            dialog_access_hash: PeerAuth::from_hash(access_hash),
        };
        Self {
            major: MAJOR,
            minor: MINOR,
            file_type: source.get_file_type(),
            dc_id: photo.dc_id,
            file_reference: Vec::new(),
            variant: FileIdVariant::Photo {
                file_id: photo.photo_id,
                access_hash: 0,
                source,
            },
        }
    }

    pub fn channel_photo_big(channel: &Channel, photo: &ChatPhoto) -> Self {
        let access_hash = channel.raw.access_hash.unwrap_or_default();
        let source = PhotoSizeSource::DialogPhotoBig {
            dialog_id: channel.id(),
            dialog_access_hash: PeerAuth::from_hash(access_hash),
        };
        Self {
            major: MAJOR,
            minor: MINOR,
            file_type: source.get_file_type(),
            dc_id: photo.dc_id,
            file_reference: Vec::new(),
            variant: FileIdVariant::Photo {
                file_id: photo.photo_id,
                access_hash: 0,
                source,
            },
        }
    }

    pub fn community_photo_big(community: &Community, photo: &ChatPhoto) -> Self {
        let access_hash = community.raw.access_hash.unwrap_or_default();
        let source = PhotoSizeSource::DialogPhotoBig {
            dialog_id: community.id(),
            dialog_access_hash: PeerAuth::from_hash(access_hash),
        };
        Self {
            major: MAJOR,
            minor: MINOR,
            file_type: source.get_file_type(),
            dc_id: photo.dc_id,
            file_reference: Vec::new(),
            variant: FileIdVariant::Photo {
                file_id: photo.photo_id,
                access_hash: 0,
                source,
            },
        }
    }

    pub fn photo(photo: &Photo, size: char) -> Self {
        let source = PhotoSizeSource::Thumbnail {
            file_type: FileType::Photo,
            thumbnail_type: size as u32,
        };
        Self {
            major: MAJOR,
            minor: MINOR,
            file_type: source.get_file_type(),
            dc_id: photo.dc_id,
            file_reference: photo.file_reference.clone(),
            variant: FileIdVariant::Photo {
                file_id: photo.id,
                access_hash: photo.access_hash,
                source,
            },
        }
    }

    pub fn document(document: &Document) -> Self {
        let mut file_type = FileType::Document;
        for attr in &document.attributes {
            // Voice/video note? Self destructing? Live photo? Subtypes?
            match attr {
                DocumentAttribute::Animated => file_type = FileType::Animation,
                DocumentAttribute::Sticker(_) => file_type = FileType::Sticker,
                DocumentAttribute::Video(_) => file_type = FileType::Video,
                DocumentAttribute::Audio(_) => file_type = FileType::Audio,
                DocumentAttribute::CustomEmoji(_) => file_type = FileType::Sticker,
                _ => (),
            }
        }
        Self {
            major: MAJOR,
            minor: MINOR,
            file_type,
            dc_id: document.dc_id,
            file_reference: document.file_reference.clone(),
            variant: FileIdVariant::Common {
                file_id: document.id,
                access_hash: document.access_hash,
            },
        }
    }

    pub fn decode(id: &str) -> Option<Self> {
        let data = rle_decode(&BASE64_URL_SAFE_NO_PAD.decode(id).ok()?);
        let (major, data) = data.split_last()?;
        let major = *major;
        if major != MAJOR {
            return None;
        }
        let (minor, data) = data.split_last()?;
        let minor = *minor;

        let mut cursor = Cursor::from_slice(data);

        let mut file_type = u32::deserialize(&mut cursor).ok()?;
        let has_web_location = file_type & WEB_LOCATION_FLAG != 0;
        let has_file_reference = file_type & FILE_REFERENCE_FLAG != 0;
        file_type &= !WEB_LOCATION_FLAG;
        file_type &= !FILE_REFERENCE_FLAG;
        let file_type = FileType::try_from(file_type).ok()?;
        let dc_id = i32::deserialize(&mut cursor).ok()?;

        let file_reference = if has_file_reference {
            Vec::deserialize(&mut cursor).ok()?
        } else {
            Vec::new()
        };

        if has_web_location {
            let url = String::deserialize(&mut cursor).ok()?;
            let access_hash = i64::deserialize(&mut cursor).ok()?;
            if cursor.pos() != data.len() {
                return None;
            }
            return Some(Self {
                major,
                minor,
                file_type,
                dc_id,
                file_reference,
                variant: FileIdVariant::Web { url, access_hash },
            });
        }

        match file_type.get_parser_class() {
            ParserClass::Photo => {
                let file_id = i64::deserialize(&mut cursor).ok()?;
                let access_hash = i64::deserialize(&mut cursor).ok()?;
                let source = if minor > MINOR_REMOVE_PHOTO_VOLUME_AND_LOCAL_ID {
                    PhotoSizeSource::deserialize(&mut cursor).ok()?
                } else {
                    let volume_id = i64::deserialize(&mut cursor).ok()?;
                    let (source, local_id) = if minor >= MINOR_ADD_PHOTO_SIZE_SOURCE {
                        let source = PhotoSizeSource::deserialize(&mut cursor).ok()?;
                        let local_id = i32::deserialize(&mut cursor).ok()?;
                        (source, local_id)
                    } else {
                        let secret = i64::deserialize(&mut cursor).ok()?;
                        let local_id = i32::deserialize(&mut cursor).ok()?;
                        let source = PhotoSizeSource::FullLegacy {
                            volume_id,
                            local_id,
                            secret,
                        };
                        (source, local_id)
                    };
                    match source {
                        PhotoSizeSource::Legacy { secret } => PhotoSizeSource::FullLegacy {
                            volume_id,
                            local_id,
                            secret,
                        },
                        PhotoSizeSource::FullLegacy { .. } | PhotoSizeSource::Thumbnail { .. } => {
                            source
                        }
                        PhotoSizeSource::DialogPhotoSmall {
                            dialog_id,
                            dialog_access_hash,
                        } => PhotoSizeSource::DialogPhotoSmallLegacy {
                            dialog_id,
                            dialog_access_hash,
                            volume_id,
                            local_id,
                        },
                        PhotoSizeSource::DialogPhotoBig {
                            dialog_id,
                            dialog_access_hash,
                        } => PhotoSizeSource::DialogPhotoBigLegacy {
                            dialog_id,
                            dialog_access_hash,
                            volume_id,
                            local_id,
                        },
                        PhotoSizeSource::StickerSetThumbnail {
                            sticker_set_id,
                            sticker_set_access_hash,
                        } => PhotoSizeSource::StickerSetThumbnailLegacy {
                            sticker_set_id,
                            sticker_set_access_hash,
                            volume_id,
                            local_id,
                        },
                        _ => return None,
                    }
                };
                if cursor.pos() != data.len() {
                    return None;
                }
                match &source {
                    PhotoSizeSource::Legacy { .. } | PhotoSizeSource::FullLegacy { .. } => {}
                    PhotoSizeSource::Thumbnail { .. } => {
                        if source.get_file_type() != file_type
                            || (file_type != FileType::Photo
                                && file_type != FileType::PhotoStory
                                && file_type != FileType::SelfDestructingPhoto
                                && file_type != FileType::Thumbnail
                                && file_type != FileType::EncryptedThumbnail)
                        {
                            return None;
                        }
                    }
                    PhotoSizeSource::DialogPhotoSmall { .. }
                    | PhotoSizeSource::DialogPhotoBig { .. }
                    | PhotoSizeSource::DialogPhotoSmallLegacy { .. }
                    | PhotoSizeSource::DialogPhotoBigLegacy { .. } => {
                        if file_type != FileType::ProfilePhoto {
                            return None;
                        }
                    }
                    PhotoSizeSource::StickerSetThumbnail { .. }
                    | PhotoSizeSource::StickerSetThumbnailLegacy { .. }
                    | PhotoSizeSource::StickerSetThumbnailVersion { .. } => {
                        if file_type != FileType::Thumbnail {
                            return None;
                        }
                    }
                }
                Some(Self {
                    major,
                    minor,
                    file_type,
                    dc_id,
                    file_reference,
                    variant: FileIdVariant::Photo {
                        file_id,
                        access_hash,
                        source,
                    },
                })
            }
            ParserClass::Common => {
                let file_id = i64::deserialize(&mut cursor).ok()?;
                let access_hash = i64::deserialize(&mut cursor).ok()?;
                if cursor.pos() != data.len() {
                    return None;
                }
                Some(Self {
                    major,
                    minor,
                    file_type,
                    dc_id,
                    file_reference,
                    variant: FileIdVariant::Common {
                        file_id,
                        access_hash,
                    },
                })
            }
            ParserClass::None => None,
        }
    }

    pub fn encode(&self) -> String {
        let mut data = Vec::new();
        let mut file_type = self.file_type as u32;
        if !self.file_reference.is_empty() {
            file_type |= FILE_REFERENCE_FLAG;
        }
        if matches!(self.variant, FileIdVariant::Web { .. }) {
            file_type |= WEB_LOCATION_FLAG;
        }
        file_type.serialize(&mut data);
        self.dc_id.serialize(&mut data);
        if !self.file_reference.is_empty() {
            self.file_reference.serialize(&mut data);
        }
        match &self.variant {
            FileIdVariant::Web { url, access_hash } => {
                url.serialize(&mut data);
                access_hash.serialize(&mut data);
            }
            FileIdVariant::Photo {
                file_id,
                access_hash,
                source,
            } => {
                file_id.serialize(&mut data);
                access_hash.serialize(&mut data);
                source.serialize(&mut data);
            }
            FileIdVariant::Common {
                file_id,
                access_hash,
            } => {
                file_id.serialize(&mut data);
                access_hash.serialize(&mut data);
            }
        }
        data.push(self.minor);
        data.push(self.major);
        BASE64_URL_SAFE_NO_PAD.encode(rle_encode(&data))
    }
}

impl Downloadable for FileId {
    fn to_raw_input_location(&self) -> Option<InputFileLocation> {
        match self.variant {
            FileIdVariant::Photo {
                file_id,
                access_hash,
                ref source,
            } => {
                match source {
                    PhotoSizeSource::Legacy { .. } => None,
                    PhotoSizeSource::Thumbnail {
                        file_type,
                        thumbnail_type,
                    } => match file_type {
                        FileType::Photo | FileType::PhotoStory | FileType::SelfDestructingPhoto => {
                            Some(InputFileLocation::InputPhotoFileLocation(
                                InputPhotoFileLocation {
                                    id: file_id,
                                    access_hash,
                                    file_reference: self.file_reference.clone(),
                                    thumb_size: (*thumbnail_type as u8 as char).to_string(),
                                },
                            ))
                        }
                        FileType::Thumbnail => Some(InputFileLocation::InputDocumentFileLocation(
                            InputDocumentFileLocation {
                                id: file_id,
                                access_hash,
                                file_reference: self.file_reference.clone(),
                                thumb_size: (*thumbnail_type as u8 as char).to_string(),
                            },
                        )),
                        _ => None,
                    },
                    PhotoSizeSource::DialogPhotoSmall {
                        dialog_id,
                        dialog_access_hash,
                    } => Some(InputFileLocation::InputPeerPhotoFileLocation(
                        InputPeerPhotoFileLocation {
                            big: false,
                            peer: PeerRef {
                                id: *dialog_id,
                                auth: *dialog_access_hash,
                            }
                            .into(),
                            photo_id: file_id,
                        },
                    )),
                    PhotoSizeSource::DialogPhotoBig {
                        dialog_id,
                        dialog_access_hash,
                    } => Some(InputFileLocation::InputPeerPhotoFileLocation(
                        InputPeerPhotoFileLocation {
                            big: true,
                            peer: PeerRef {
                                id: *dialog_id,
                                auth: *dialog_access_hash,
                            }
                            .into(),
                            photo_id: file_id,
                        },
                    )),
                    PhotoSizeSource::StickerSetThumbnail { .. } => None,
                    PhotoSizeSource::FullLegacy {
                        volume_id,
                        local_id,
                        secret,
                    } => Some(InputFileLocation::InputPhotoLegacyFileLocation(
                        InputPhotoLegacyFileLocation {
                            id: file_id,
                            access_hash,
                            file_reference: self.file_reference.clone(),
                            volume_id: *volume_id,
                            local_id: *local_id,
                            secret: *secret,
                        },
                    )),
                    // InputPeerPhotoFileLocationLegacy
                    PhotoSizeSource::DialogPhotoSmallLegacy { .. } => None,
                    PhotoSizeSource::DialogPhotoBigLegacy { .. } => None,
                    // InputStickerSetThumbLegacy
                    PhotoSizeSource::StickerSetThumbnailLegacy { .. } => None,
                    PhotoSizeSource::StickerSetThumbnailVersion {
                        sticker_set_id,
                        sticker_set_access_hash,
                        version,
                    } => Some(InputFileLocation::InputStickerSetThumb(
                        InputStickerSetThumb {
                            stickerset: InputStickerSet::Id(InputStickerSetId {
                                id: *sticker_set_id,
                                access_hash: *sticker_set_access_hash,
                            }),
                            thumb_version: *version,
                        },
                    )),
                }
            }
            FileIdVariant::Common {
                file_id,
                access_hash,
            } => match self.file_type {
                FileType::Encrypted => Some(InputFileLocation::InputEncryptedFileLocation(
                    InputEncryptedFileLocation {
                        id: file_id,
                        access_hash,
                    },
                )),
                FileType::SecureDecrypted | FileType::SecureEncrypted => Some(
                    InputFileLocation::InputSecureFileLocation(InputSecureFileLocation {
                        id: file_id,
                        access_hash,
                    }),
                ),
                _ => Some(InputFileLocation::InputDocumentFileLocation(
                    InputDocumentFileLocation {
                        id: file_id,
                        access_hash,
                        file_reference: self.file_reference.clone(),
                        thumb_size: "".to_string(),
                    },
                )),
            },
            FileIdVariant::Web { .. } => None,
        }
    }
}
