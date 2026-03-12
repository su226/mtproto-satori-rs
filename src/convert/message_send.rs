use std::{
    borrow::BorrowMut,
    collections::{HashMap, HashSet},
    io::{self, Cursor},
    mem::replace,
    path::{Path, PathBuf},
};

use async_tempfile::TempFile;
use base64::prelude::*;
use futures_util::TryStreamExt;
use grammers_client::{
    Client, InvocationError,
    media::{InputMedia, Uploaded},
    message::{Button, ReplyMarkup},
    peer::{Channel, Group, Peer, User},
    tl::{
        enums::{
            Document as DocumentEnum, DocumentAttribute, InputChannel as InputChannelEnum,
            InputUser as InputUserEnum,
        },
        functions::{
            channels::GetChannels,
            messages::{GetChats, GetCustomEmojiDocuments},
            users::GetUsers,
        },
        types::{Document, InputChannel, InputUser},
    },
};
use grammers_session::types::PeerKind;
use image::{ImageError, ImageFormat, ImageReader, ImageResult};
use infer::Infer;
use lazy_static::lazy_static;
use log::debug;
use mime_guess::get_mime_extensions_str;
use ntex::{http::StatusCode, rt::spawn_blocking};
use regex::Regex;
use tokio::{fs, io::AsyncWriteExt};
use tokio_util::io::StreamReader;
use url::Url;

use crate::{
    error::MyError,
    satori::element::{Element, escape},
    telegram::{peer_id_from_bot_api_id, upload_file_custom_name},
};

pub struct MessagePack {
    pub content: String,
    pub asset: Vec<Element>,
    pub reply: Option<String>,
    pub rows: Vec<Vec<Button>>,
}

impl MessagePack {
    pub fn new() -> Self {
        Self {
            content: "".to_string(),
            asset: Vec::new(),
            reply: None,
            rows: Vec::new(),
        }
    }

    pub fn reply_markup(&self) -> Option<ReplyMarkup> {
        if self.rows.is_empty() {
            None
        } else {
            Some(ReplyMarkup::from_buttons(self.rows.as_slice()))
        }
    }
}

pub fn to_reply_markup(rows: &[Vec<Button>]) -> Option<ReplyMarkup> {
    if rows.is_empty() {
        None
    } else {
        Some(ReplyMarkup::from_buttons(rows))
    }
}

#[derive(PartialEq)]
enum MessageEncoderMode {
    Default,
    Figure,
}

#[derive(Debug)]
pub struct EncoderInfo {
    emojis: HashMap<i64, Document>,
    users_by_name: HashMap<String, Peer>,
    users_by_id: HashMap<i64, Peer>,
}

pub struct MessageEncoder {
    pub packs: Vec<MessagePack>,
    current: MessagePack,
    mode: MessageEncoderMode,
    info: EncoderInfo,
}

impl MessageEncoder {
    pub fn new(info: EncoderInfo) -> Self {
        Self {
            packs: Vec::new(),
            current: MessagePack::new(),
            mode: MessageEncoderMode::Default,
            info,
        }
    }

    fn get_emoji_name(&self, id: i64) -> Option<&str> {
        self.info.emojis.get(&id).and_then(|emoji| {
            let mut alt = None;
            for attr in &emoji.attributes {
                if let DocumentAttribute::Sticker(sticker) = attr {
                    alt = Some(sticker.alt.as_str())
                }
            }
            alt
        })
    }

    fn get_user_name(&self, id: i64) -> Option<String> {
        if let Some(user) = self.info.users_by_id.get(&id) {
            if let Some(username) = user.username() {
                return Some(format!("@{}", username));
            }
            let name = match user {
                Peer::User(user) => {
                    let name = user.full_name();
                    if name.is_empty() { None } else { Some(name) }
                }
                Peer::Group(group) => group.title().map(|x| x.to_string()),
                Peer::Channel(channel) => Some(channel.title().to_string()),
            };
            if name.is_some() {
                return name;
            }
        }
        return None;
    }

    fn get_user_id(&self, username: &str) -> Option<i64> {
        self.info
            .users_by_name
            .get(username)
            .map(|user| user.id().bot_api_dialog_id())
    }

    fn visit(&mut self, element: &Element) {
        match element.tag.as_str() {
            "text" => self.current.content += &escape(element.get_text().unwrap(), false),
            "br" => self.current.content += "\n",
            "p" => {
                if !self.current.content.ends_with("\n") {
                    self.current.content += "\n";
                }
                self.render(&element.children);
                if !self.current.content.ends_with("\n") {
                    self.current.content += "\n";
                }
            }
            "a" => {
                if let Some(href) = element.get_attr_str("href") {
                    self.current.content += &format!("<a href=\"{}\">", escape(href, true));
                } else {
                    self.current.content += "<a>";
                }
                self.render(&element.children);
                self.current.content += "</a>";
            }
            tag @ ("b" | "strong" | "i" | "em" | "u" | "ins" | "s" | "del") => {
                self.current.content += &format!("<{}>", tag);
                self.render(&element.children);
                self.current.content += &format!("</{}>", tag);
            }
            "spl" => {
                self.current.content += "<tg-spoiler>";
                self.render(&element.children);
                self.current.content += "</tg-spoiler>";
            }
            "code" => {
                self.current.content += "<code>";
                if let Some(content) = element.get_attr_str("content") {
                    self.current.content += &escape(content, false);
                } else {
                    self.render(&element.children);
                }
                self.current.content += "</code>"
            }
            "pre" | "code-block" => {
                if let Some(lang) = element.get_attr_str("lang") {
                    self.current.content +=
                        &format!("<pre><code class=\"language-{}\">", escape(lang, true));
                } else {
                    self.current.content += "<pre><code>";
                }
                self.render(&element.children);
                self.current.content += "</code></pre>";
            }
            "at" => {
                if let Some(id) = element.get_attr_str("id") {
                    if let Ok(id) = id.parse::<i64>() {
                        let display = element
                            .get_attr_str("name")
                            .map(|s| s.to_string())
                            .or_else(|| self.get_user_name(id))
                            .unwrap_or_else(|| "User".to_string());
                        self.current.content += &format!(
                            "<a href=\"tg://user?id={}\">{}</a>",
                            id,
                            escape(&display, false),
                        );
                        self.current.content += &format!(
                            "<a href=\"tg://user?id={}\">{}</a>",
                            id,
                            escape(&display, false),
                        );
                    } else {
                        let username = id.strip_prefix("@").unwrap_or(id);
                        let id = self
                            .get_user_id(username)
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| escape(&format!("@{}", username), true));
                        let display = element
                            .get_attr_str("name")
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| format!("@{}", username));
                        self.current.content += &format!(
                            "<a href=\"tg://user?id={}\">{}</a>",
                            id,
                            escape(&display, false),
                        );
                    }
                }
            }
            "emoji" => {
                if let Some(id) = element.get_attr_str("id")
                    && let Ok(id) = id.parse::<i64>()
                {
                    let name = element
                        .get_attr_str("name")
                        .or_else(|| self.get_emoji_name(id))
                        .unwrap_or("😀");
                    self.current.content += &format!(
                        "<tg-emoji emoji-id=\"{}\">{}</tg-emoji>",
                        id,
                        escape(name, false),
                    );
                }
            }
            "img" | "image" | "audio" | "video" | "file" => {
                self.current.asset.push(element.clone())
            }
            "figure" => {
                self.flush();
                self.mode = MessageEncoderMode::Figure;
                self.render(&element.children);
                self.flush();
            }
            "quote" => {
                if let Some(id) = element.get_attr_str("id") {
                    self.flush();
                    self.current.reply = Some(id.to_string());
                } else {
                    self.current.content += "<blockquote>";
                    self.render(&element.children);
                    self.current.content += "</blockquote>";
                }
            }
            "button" => {
                if self.current.rows.is_empty() {
                    self.current.rows.push(Vec::new());
                }
                let mut row = self.current.rows.last_mut().unwrap();
                if row.len() > 5 {
                    self.current.rows.push(Vec::new());
                    row = self.current.rows.last_mut().unwrap();
                }
                let label = element.strip();
                row.push(match element.get_attr_str("type") {
                    Some("link") => Button::url(label, element.get_attr_str("href").unwrap_or("")),
                    Some("input") => Button::switch(
                        label,
                        element.get_attr_str("text").unwrap_or("").to_string(),
                    ),
                    _ => Button::data(label, element.get_attr_str("id").unwrap_or("").to_string()),
                });
            }
            "button-group" => {
                self.current.rows.push(Vec::new());
                self.render(&element.children);
                self.current.rows.push(Vec::new());
            }
            "message" => {
                if self.mode == MessageEncoderMode::Figure {
                    self.render(&element.children);
                    self.current.content += "\n";
                } else {
                    self.flush();
                    self.render(&element.children);
                    self.flush();
                }
            }
            _ => self.render(&element.children),
        }
    }

    pub fn render(&mut self, elements: &[Element]) {
        for element in elements {
            self.visit(element);
        }
    }

    pub fn flush(&mut self) {
        if self.current.content.is_empty() && self.current.asset.is_empty() {
            return;
        }
        let mut current = replace(&mut self.current, MessagePack::new());
        if let Some(last) = self.current.rows.last()
            && last.is_empty()
        {
            current.rows.pop();
        }
        self.packs.push(current);
    }
}

fn extract_infos(
    elements: &[Element],
    out_emojis: &mut HashSet<i64>,
    out_usernames: &mut HashSet<String>,
    out_user_ids: &mut HashSet<i64>,
) {
    for element in elements {
        match element.tag.as_str() {
            "emoji" => {
                if let Some(id) = element.get_attr_str("id")
                    && let Ok(id) = id.parse::<i64>()
                {
                    out_emojis.insert(id);
                }
            }
            "at" => {
                if let Some(id) = element.get_attr_str("id") {
                    if let Ok(id) = id.parse::<i64>() {
                        if element.get_attr_str("name").is_none() {
                            out_user_ids.insert(id);
                        }
                    } else {
                        out_usernames.insert(id.to_string());
                    }
                }
            }
            _ => extract_infos(&element.children, out_emojis, out_usernames, out_user_ids),
        }
    }
}

pub async fn fetch_infos(
    client: &Client,
    elements: &[Element],
) -> Result<EncoderInfo, InvocationError> {
    let mut emoji_ids = HashSet::new();
    let mut usernames = HashSet::new();
    let mut user_ids = HashSet::new();
    extract_infos(elements, &mut emoji_ids, &mut usernames, &mut user_ids);
    let emojis = if emoji_ids.is_empty() {
        HashMap::new()
    } else {
        client
            .invoke(&GetCustomEmojiDocuments {
                document_id: emoji_ids.into_iter().collect(),
            })
            .await?
            .into_iter()
            .filter_map(|doc| match doc {
                DocumentEnum::Document(doc) => Some((doc.id, doc)),
                DocumentEnum::Empty(_) => None,
            })
            .collect::<HashMap<i64, Document>>()
    };
    let mut users_by_name = HashMap::new();
    for username in usernames {
        if let Some(peer) = client.resolve_username(username.as_str()).await? {
            users_by_name.insert(username.clone(), peer);
        }
    }
    let mut peer_ids_user = Vec::new();
    let mut peer_ids_chat = Vec::new();
    let mut peer_ids_channel = Vec::new();
    for id in user_ids {
        let peer_id = peer_id_from_bot_api_id(id);
        if let Some(peer_id) = peer_id {
            match peer_id.kind() {
                PeerKind::User => peer_ids_user.push(peer_id),
                PeerKind::Chat => peer_ids_chat.push(peer_id),
                PeerKind::Channel => peer_ids_channel.push(peer_id),
                PeerKind::UserSelf => unreachable!(),
            }
        }
    }
    let mut users_by_id = HashMap::new();
    if !peer_ids_user.is_empty() {
        let users = client
            .invoke(&GetUsers {
                id: peer_ids_user
                    .into_iter()
                    .map(|x| {
                        InputUserEnum::User(InputUser {
                            user_id: x.bare_id(),
                            access_hash: 0,
                        })
                    })
                    .collect(),
            })
            .await?;
        for user in users {
            users_by_id.insert(user.id(), Peer::User(User::from_raw(client, user)));
        }
    }
    if !peer_ids_chat.is_empty() {
        let chats = client
            .invoke(&GetChats {
                id: peer_ids_chat.into_iter().map(|x| x.bare_id()).collect(),
            })
            .await?;
        for chat in chats.chats() {
            users_by_id.insert(chat.id(), Peer::Group(Group::from_raw(client, chat)));
        }
    }
    if !peer_ids_channel.is_empty() {
        let channels = client
            .invoke(&GetChannels {
                id: peer_ids_channel
                    .into_iter()
                    .map(|x| {
                        InputChannelEnum::Channel(InputChannel {
                            channel_id: x.bare_id(),
                            access_hash: 0,
                        })
                    })
                    .collect(),
            })
            .await?;
        for channel in channels.chats() {
            users_by_id.insert(
                channel.id(),
                Peer::Channel(Channel::from_raw(client, channel)),
            );
        }
    }
    Ok(EncoderInfo {
        emojis,
        users_by_name,
        users_by_id,
    })
}

pub fn parse_reply(reply: Option<&str>) -> Result<Option<i32>, MyError> {
    if let Some(id) = reply {
        if let Ok(id) = id.parse::<i32>() {
            Ok(Some(id))
        } else {
            Err(MyError::new(
                StatusCode::BAD_REQUEST,
                "Bad reply id".to_string(),
            ))
        }
    } else {
        Ok(None)
    }
}

lazy_static! {
    static ref BASE64_HEADER_REGEX: Regex = Regex::new(r"^data:([\w/.+-]+);base64,").unwrap();
    static ref MIME_SPLIT_REGEX: Regex = Regex::new(r"[;,]").unwrap();
}

fn tempfile_err_to_io_err(err: async_tempfile::Error) -> io::Error {
    match err {
        async_tempfile::Error::InvalidDirectory | async_tempfile::Error::InvalidFile => {
            io::Error::new(io::ErrorKind::NotFound, err)
        }
        async_tempfile::Error::Io(err) => err,
    }
}

async fn upload_media(
    client: &Client,
    element: &Element,
    session_name: &str,
) -> Result<Uploaded, io::Error> {
    let url = element
        .get_attr_str("src")
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let name = element.get_attr_str("name");
    if let Some(header) = BASE64_HEADER_REGEX.captures(url) {
        debug!("Uploading file {}...", header.get_match().as_str());
        let data = BASE64_STANDARD
            .decode(&url[header.get_match().end()..])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let name = if let Some(name) = name
            && !name.is_empty()
        {
            name.to_string()
        } else {
            let ext = Infer::new()
                .get(&data)
                .map(|x| x.extension())
                .unwrap_or_else(|| {
                    mime_guess::get_mime_extensions_str(header.get(1).unwrap().as_str())
                        .map(|x| x[0])
                        .unwrap_or("bin")
                });
            format!("file.{}", ext)
        };
        let size = data.len();
        let mut stream = Cursor::new(data);
        client.upload_stream(&mut stream, size, name).await
    } else if url.starts_with("file:") {
        debug!("Uploading file {}", url);
        let path = Url::parse(url)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
            .to_file_path()
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        let name = if let Some(name) = name
            && !name.is_empty()
        {
            Some(name.to_string())
        } else {
            None
        };
        upload_file_custom_name(client, &path, name).await
    } else {
        debug!("Uploading file {}", url);
        let mut response = reqwest::get(url)
            .await
            .map_err(|err| io::Error::other(err))?;
        let name = if let Some(name) = name
            && !name.is_empty()
        {
            name.to_string()
        } else {
            Url::parse(url)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
                .path_segments()
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?
                .last()
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?
                .to_string()
        };
        if let Some(size) = response.content_length() {
            let stream = &mut StreamReader::new(response.bytes_stream().map_err(io::Error::other));
            client.upload_stream(stream, size as usize, name).await
        } else {
            let mut dir = PathBuf::new();
            dir.push(format!("files_{}", session_name));
            fs::create_dir_all(&dir).await?;
            let mut file = TempFile::new_in(dir)
                .await
                .map_err(tempfile_err_to_io_err)?;
            while let Some(mut item) = response.chunk().await.map_err(io::Error::other)? {
                file.write_all_buf(item.borrow_mut()).await?;
            }
            let path = file.file_path();
            upload_file_custom_name(client, &path, Some(name)).await
        }
    }
}

fn image_err_to_io_err(err: ImageError) -> io::Error {
    match err {
        ImageError::IoError(err) => err,
        _ => io::Error::other(err),
    }
}

fn extension_match(name: &str, ext: &str) -> bool {
    name.len() > ext.len() + 1
        && name.as_bytes()[name.len() - ext.len() - 1] == b'.'
        && &name[name.len() - ext.len()..] == ext
}

async fn convert_image(path: &Path, out_path: &Path) -> io::Result<()> {
    let path = path.to_owned();
    let out_path = out_path.to_owned();
    match spawn_blocking(move || -> ImageResult<()> {
        let image = ImageReader::open(path)?.with_guessed_format()?.decode()?;
        image.save_with_format(out_path, ImageFormat::Png)
    })
    .await
    {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(err)) => Err(image_err_to_io_err(err)),
        Err(err) => Err(io::Error::other(err)),
    }
}

fn image_mime_valid(mime: &str) -> bool {
    mime == "image/jpg" || mime == "image/png" || mime == "image/gif"
}

async fn upload_image(
    client: &Client,
    element: &Element,
    session_name: &str,
) -> Result<(Uploaded, String), io::Error> {
    let url = element
        .get_attr_str("src")
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let name = element.get_attr_str("name");
    if let Some(header) = BASE64_HEADER_REGEX.captures(url) {
        debug!("Uploading image {}...", header.get_match().as_str());
        let mut data = BASE64_STANDARD
            .decode(&url[header.get_match().end()..])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let (mut mime, mut ext) = Infer::new()
            .get(&data)
            .map(|x| (x.mime_type(), x.extension()))
            .unwrap_or_else(|| {
                let mime = header.get(1).unwrap().as_str();
                let ext = get_mime_extensions_str(mime).map(|x| x[0]).unwrap_or("bin");
                (mime, ext)
            });
        if !image_mime_valid(mime) {
            let image = ImageReader::new(Cursor::new(data))
                .decode()
                .map_err(image_err_to_io_err)?;
            let mut new_data = Vec::new();
            image
                .write_to(Cursor::new(&mut new_data), ImageFormat::Png)
                .map_err(image_err_to_io_err)?;
            data = new_data;
            mime = "image/png";
            ext = "png";
        }
        let name = if let Some(name) = name
            && !name.is_empty()
        {
            if extension_match(name, ext) {
                name.to_string()
            } else {
                format!("{}.{}", name, ext)
            }
        } else {
            format!("file.{}", ext)
        };
        let size = data.len();
        let mut stream = Cursor::new(data);
        Ok((
            client.upload_stream(&mut stream, size, name).await?,
            mime.to_string(),
        ))
    } else if url.starts_with("file:") {
        debug!("Uploading image {}", url);
        let mut path = &Url::parse(url)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
            .to_file_path()
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        let (mut mime, mut ext) = Infer::new()
            .get_from_path(path)?
            .map(|x| (x.mime_type(), x.extension()))
            .unwrap_or(("application/octet-stream", "bin"));
        let mut tempfile = None;
        if !image_mime_valid(mime) {
            let mut dir = PathBuf::new();
            dir.push(format!("files_{}", session_name));
            fs::create_dir_all(&dir).await?;
            tempfile = Some(
                TempFile::new_in(dir)
                    .await
                    .map_err(tempfile_err_to_io_err)?,
            );
            let new_path = tempfile.as_ref().unwrap().file_path();
            path = new_path;
            mime = "image/png";
            ext = "png";
        }
        let name = if let Some(name) = name
            && !name.is_empty()
        {
            name.to_string()
        } else {
            path.file_name().unwrap().to_string_lossy().to_string()
        };
        let name = if extension_match(&name, ext) {
            name
        } else {
            format!("{}.{}", name, ext)
        };
        let result = upload_file_custom_name(client, &path, Some(name)).await?;
        drop(tempfile);
        Ok((result, mime.to_string()))
    } else {
        debug!("Uploading image {}", url);
        let mut response = reqwest::get(url)
            .await
            .map_err(|err| io::Error::other(err))?;
        let (mut mime, mut ext) = response
            .headers()
            .get("Content-Type")
            .and_then(|x| x.to_str().ok())
            .map(|x| MIME_SPLIT_REGEX.splitn(x, 1).next().unwrap().to_string())
            .map(|x| {
                let ext = get_mime_extensions_str(&x)
                    .map(|x| x[0])
                    .unwrap_or("bin")
                    .to_string();
                (x, ext)
            })
            .unwrap_or(("application/octet-stream".to_string(), "bin".to_string()));
        let name = if let Some(name) = name
            && !name.is_empty()
        {
            name.to_string()
        } else {
            Url::parse(url)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
                .path_segments()
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?
                .last()
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?
                .to_string()
        };
        let mime_valid = image_mime_valid(&mime);
        if let Some(size) = response.content_length()
            && mime_valid
        {
            let name = if extension_match(&name, &ext) {
                name
            } else {
                format!("{}.{}", name, ext)
            };
            let stream = &mut StreamReader::new(response.bytes_stream().map_err(io::Error::other));
            Ok((
                client.upload_stream(stream, size as usize, name).await?,
                mime,
            ))
        } else {
            let mut dir = PathBuf::new();
            dir.push(format!("files_{}", session_name));
            fs::create_dir_all(&dir).await?;
            let mut file = TempFile::new_in(dir.clone())
                .await
                .map_err(tempfile_err_to_io_err)?;
            while let Some(mut item) = response.chunk().await.map_err(io::Error::other)? {
                file.write_all_buf(item.borrow_mut()).await?;
            }
            let path = file.file_path();
            if !mime_valid {
                convert_image(path, path).await?;
                mime = "image/png".to_string();
                ext = "png".to_string();
            }
            let name = if extension_match(&name, &ext) {
                name
            } else {
                format!("{}.{}", name, ext)
            };
            Ok((
                upload_file_custom_name(client, &path, Some(name)).await?,
                mime,
            ))
        }
    }
}

pub async fn upload_medias(
    client: &Client,
    elements: &[Element],
    session_name: &str,
) -> Result<Vec<InputMedia>, io::Error> {
    let mut medias = Vec::new();
    for element in elements {
        medias.push(match element.tag.as_str() {
            "img" | "image" => {
                let (media, mime) = upload_image(client, element, session_name).await?;
                if mime == "image/gif" {
                    InputMedia::new().file(media)
                } else {
                    InputMedia::new().photo(media)
                }
            }
            "audio" => {
                let media = upload_media(client, element, session_name).await?;
                InputMedia::new().document(media)
            }
            "video" => {
                let media = upload_media(client, element, session_name).await?;
                InputMedia::new().document(media)
            }
            "file" => {
                let media = upload_media(client, element, session_name).await?;
                InputMedia::new().file(media)
            }
            _ => unreachable!(),
        });
    }
    Ok(medias)
}
