use base64::prelude::*;
use grammers_client::media::Downloadable;
use grammers_client::tl::enums::{InputFileLocation, InputPeer};
use grammers_client::tl::types::{
    InputDocumentFileLocation,
    InputPeerPhotoFileLocation,
    InputPeerUser,
    InputPhotoFileLocation,
};

fn write_array(bytes: &mut Vec<u8>, data: &[u8]) {
    bytes.extend((data.len() as u32).to_be_bytes());
    bytes.extend(data);
}

fn read_array(bytes: &[u8], start: usize) -> Option<(&[u8], usize)> {
    let mid = start + 4;
    if mid > bytes.len() {
        return None;
    }
    let len = u32::from_be_bytes(*bytes[start..mid].as_array().unwrap());
    let end = mid + len as usize;
    if end > bytes.len() {
        return None;
    }
    Some((&bytes[mid..end], end))
}

const TYPE_PEER_PHOTO: u8 = 0;
const TYPE_PHOTO: u8 = 1;
const TYPE_DOCUMENT: u8 = 2;

pub enum FileId {
    PeerPhoto {
        peer_id: i64,
        access_hash: i64,
        photo_id: i64,
    },
    Photo {
        id: i64,
        access_hash: i64,
        file_reference: Vec<u8>,
        thumb_size: Vec<u8>,
    },
    Document {
        id: i64,
        access_hash: i64,
        file_reference: Vec<u8>,
    },
}

impl FileId {
    pub fn encode(&self) -> String {
        let mut bytes = Vec::new();
        match self {
            FileId::PeerPhoto {
                peer_id,
                access_hash,
                photo_id,
            } => {
                bytes.push(TYPE_PEER_PHOTO);
                bytes.extend(peer_id.to_be_bytes());
                bytes.extend(access_hash.to_be_bytes());
                bytes.extend(photo_id.to_be_bytes());
            }
            FileId::Photo {
                id,
                access_hash,
                file_reference,
                thumb_size,
            } => {
                bytes.push(TYPE_PHOTO);
                bytes.extend(id.to_be_bytes());
                bytes.extend(access_hash.to_be_bytes());
                write_array(&mut bytes, file_reference);
                write_array(&mut bytes, thumb_size);
            }
            FileId::Document {
                id,
                access_hash,
                file_reference,
            } => {
                bytes.push(TYPE_DOCUMENT);
                bytes.extend(id.to_be_bytes());
                bytes.extend(access_hash.to_be_bytes());
                write_array(&mut bytes, file_reference);
            }
        }
        BASE64_URL_SAFE_NO_PAD.encode(bytes)
    }

    pub fn decode(id: &str) -> Option<Self> {
        let bytes = BASE64_URL_SAFE_NO_PAD.decode(id).ok()?;
        let id_type = *bytes.first()?;
        match id_type {
            TYPE_PEER_PHOTO => {
                if bytes.len() != 25 {
                    return None;
                }
                Some(Self::PeerPhoto {
                    peer_id: i64::from_be_bytes(*bytes[1..9].as_array().unwrap()),
                    access_hash: i64::from_be_bytes(*bytes[9..17].as_array().unwrap()),
                    photo_id: i64::from_be_bytes(*bytes[17..25].as_array().unwrap()),
                })
            }
            TYPE_PHOTO => {
                if bytes.len() < 17 {
                    return None;
                }
                let id = i64::from_be_bytes(*bytes[1..9].as_array().unwrap());
                let access_hash = i64::from_be_bytes(*bytes[9..17].as_array().unwrap());
                let (file_reference, i) = read_array(&bytes, 17)?;
                let (size, i) = read_array(&bytes, i)?;
                if i < bytes.len() {
                    return None;
                }
                Some(Self::Photo {
                    id,
                    access_hash,
                    file_reference: file_reference.to_vec(),
                    thumb_size: size.to_vec(),
                })
            }
            TYPE_DOCUMENT => {
                if bytes.len() < 17 {
                    return None;
                }
                let id = i64::from_be_bytes(*bytes[1..9].as_array().unwrap());
                let access_hash = i64::from_be_bytes(*bytes[9..17].as_array().unwrap());
                let (file_reference, i) = read_array(&bytes, 17)?;
                if i < bytes.len() {
                    return None;
                }
                Some(Self::Document {
                    id,
                    access_hash,
                    file_reference: file_reference.to_vec(),
                })
            }
            _ => None,
        }
    }
}

impl Downloadable for FileId {
    fn to_raw_input_location(&self) -> Option<InputFileLocation> {
        match self {
            FileId::PeerPhoto {
                peer_id,
                access_hash,
                photo_id,
            } => Some(InputFileLocation::InputPeerPhotoFileLocation(
                InputPeerPhotoFileLocation {
                    big: true,
                    peer: InputPeer::User(InputPeerUser {
                        user_id: *peer_id,
                        access_hash: *access_hash,
                    }),
                    photo_id: *photo_id,
                },
            )),
            FileId::Photo {
                id,
                access_hash,
                file_reference,
                thumb_size,
            } => Some(InputFileLocation::InputPhotoFileLocation(
                InputPhotoFileLocation {
                    id: *id,
                    access_hash: *access_hash,
                    file_reference: file_reference.clone(),
                    thumb_size: String::from_utf8_lossy(thumb_size).to_string(),
                },
            )),
            FileId::Document {
                id,
                access_hash,
                file_reference,
            } => Some(InputFileLocation::InputDocumentFileLocation(
                InputDocumentFileLocation {
                    id: *id,
                    access_hash: *access_hash,
                    file_reference: file_reference.clone(),
                    thumb_size: "".to_string(),
                },
            )),
        }
    }
}
