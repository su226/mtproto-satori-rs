pub mod file_id;

use std::io;
use std::io::SeekFrom;
use std::path::Path;

use grammers_client::Client;
use grammers_client::media::Uploaded;
use grammers_client::message::{InputMessage, ReplyMarkup};
use grammers_client::tl::enums::{Document, DocumentAttribute};
use grammers_client::tl::types::MessageMediaDocument;
use tokio::fs;
use tokio::io::AsyncSeekExt;

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
