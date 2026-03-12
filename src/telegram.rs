pub mod file_id;

use std::{
    io::{self, SeekFrom},
    path::Path,
};

use grammers_client::{
    Client,
    media::Uploaded,
    message::{InputMessage, ReplyMarkup},
    tl::{
        enums::{Document, DocumentAttribute},
        types::MessageMediaDocument,
    },
};
use grammers_session::types::PeerId;
use tokio::{fs, io::AsyncSeekExt};

pub fn peer_id_from_bot_api_id(id: i64) -> Option<PeerId> {
    match id {
        1..=0xffffffffff => Some(PeerId::user_unchecked(id)),
        -999999999999..=-1 => Some(PeerId::chat_unchecked(-id)),
        -1997852516352..=-1000000000001 => Some(PeerId::channel_unchecked(-id - 1000000000000)),
        -4000000000000..=-2002147483649 => Some(PeerId::channel_unchecked(-id - 1000000000000)),
        _ => None,
    }
}

pub fn add_reply_markup(message: InputMessage, markup: Option<ReplyMarkup>) -> InputMessage {
    if let Some(markup) = markup {
        message.reply_markup(markup)
    } else {
        message
    }
}

pub async fn upload_file_custom_name(
    client: &Client,
    path: &Path,
    name: Option<String>,
) -> Result<Uploaded, io::Error> {
    let mut file = fs::File::open(&path).await?;
    let size = file.seek(SeekFrom::End(0)).await? as usize;
    file.seek(SeekFrom::Start(0)).await?;
    let name = name.unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().to_string());
    client.upload_stream(&mut file, size, name).await
}

pub fn is_audio(document: &MessageMediaDocument) -> bool {
    if let Some(Document::Document(document)) = &document.document {
        for attr in &document.attributes {
            if let DocumentAttribute::Audio(_) = attr {
                return true;
            }
        }
    }
    false
}
