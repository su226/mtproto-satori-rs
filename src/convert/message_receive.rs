use std::collections::HashMap;

use grammers_client::media::Media;
use grammers_client::tl::enums::{MessageEntity, MessageReplyHeader as MessageReplyHeaderEnum};
use grammers_session::types::PeerId;
use log::trace;

use crate::convert::channel::satori_channel_from_tg_peer;
use crate::convert::guild::satori_guild_from_tg_peer;
use crate::convert::link::{satori_link_from_tg_document, satori_link_from_tg_photo};
use crate::convert::user::satori_user_from_tg_peer;
use crate::satori::element::{AttrValue, Element, dump};
use crate::satori::types::Message;
use crate::telegram::is_audio;

#[derive(PartialEq)]
enum BreakpointMode {
    Start,
    End,
}

struct Breakpoint {
    mode: BreakpointMode,
    pos: usize,
    entity: Option<MessageEntity>,
}

struct ParserState {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    code: bool,
    pre: bool,
    spoiler: bool,
    mention: bool,
    link: Option<String>,
    user: Option<i64>,
    emoji: Option<i64>,
}

pub fn satori_element_from_tg_entities(
    text: &str,
    entities: Option<&[MessageEntity]>,
) -> Vec<Element> {
    let utf16 = text.encode_utf16().collect::<Vec<u16>>();
    let mut breakpoints = Vec::<Breakpoint>::new();
    if let Some(entities) = entities {
        for entity in entities {
            match entity {
                MessageEntity::Bold(_)
                | MessageEntity::Italic(_)
                | MessageEntity::Underline(_)
                | MessageEntity::Strike(_)
                | MessageEntity::Code(_)
                | MessageEntity::Pre(_)
                | MessageEntity::Spoiler(_)
                | MessageEntity::Mention(_)
                | MessageEntity::TextUrl(_)
                | MessageEntity::MentionName(_)
                | MessageEntity::CustomEmoji(_) => {
                    let offset = entity.offset() as usize;
                    let length = entity.length() as usize;
                    breakpoints.push(Breakpoint {
                        mode: BreakpointMode::Start,
                        pos: offset,
                        entity: Some(entity.clone()),
                    });
                    breakpoints.push(Breakpoint {
                        mode: BreakpointMode::End,
                        pos: offset + length,
                        entity: Some(entity.clone()),
                    });
                }
                _ => {
                    trace!("Unsupported entity: {:?}", entity);
                }
            }
        }
    }
    for (i, ch) in utf16.iter().enumerate() {
        if *ch == 10 {
            breakpoints.push(Breakpoint {
                mode: BreakpointMode::Start,
                pos: i,
                entity: None,
            });
            breakpoints.push(Breakpoint {
                mode: BreakpointMode::End,
                pos: i + 1,
                entity: None,
            });
        }
    }
    breakpoints.sort_by_key(|bp| bp.pos);

    let mut state = ParserState {
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
        code: false,
        pre: false,
        spoiler: false,
        mention: false,
        link: None,
        user: None,
        emoji: None,
    };
    let mut elements = Vec::<Element>::new();
    let mut last_pos: usize = 0;
    for breakpoint in breakpoints {
        if breakpoint.pos > last_pos {
            let content = String::from_utf16_lossy(&utf16[last_pos..breakpoint.pos]);
            let mut element = Element::text(content.to_string());
            if state.bold {
                element = Element {
                    tag: "b".to_string(),
                    attrs: HashMap::new(),
                    children: vec![element],
                };
            }
            if state.italic {
                element = Element {
                    tag: "i".to_string(),
                    attrs: HashMap::new(),
                    children: vec![element],
                };
            }
            if state.underline {
                element = Element {
                    tag: "u".to_string(),
                    attrs: HashMap::new(),
                    children: vec![element],
                };
            }
            if state.strikethrough {
                element = Element {
                    tag: "s".to_string(),
                    attrs: HashMap::new(),
                    children: vec![element],
                };
            }
            if state.code {
                element = Element {
                    tag: "code".to_string(),
                    attrs: HashMap::new(),
                    children: vec![element],
                };
            }
            if state.pre {
                element = Element {
                    tag: "pre".to_string(),
                    attrs: HashMap::new(),
                    children: vec![element],
                };
            }
            if state.spoiler {
                element = Element {
                    tag: "spl".to_string(),
                    attrs: HashMap::new(),
                    children: vec![element],
                };
            }
            if state.mention {
                element = Element {
                    tag: "at".to_string(),
                    attrs: {
                        let mut map = HashMap::new();
                        map.insert("id".to_string(), AttrValue::Str(content[1..].to_string()));
                        map.insert("name".to_string(), AttrValue::Str(content[1..].to_string()));
                        map
                    },
                    children: Vec::new(),
                };
            }
            if let Some(ref href) = state.link {
                element = Element {
                    tag: "a".to_string(),
                    attrs: {
                        let mut map = HashMap::new();
                        map.insert("href".to_string(), AttrValue::Str(href.clone()));
                        map
                    },
                    children: vec![element],
                };
            }
            if let Some(user_id) = state.user {
                element = Element {
                    tag: "at".to_string(),
                    attrs: {
                        let mut map = HashMap::new();
                        map.insert("id".to_string(), AttrValue::Str(user_id.to_string()));
                        map.insert("name".to_string(), AttrValue::Str(content.to_string()));
                        map
                    },
                    children: Vec::new(),
                };
            }
            if let Some(emoji_id) = state.emoji {
                element = Element {
                    tag: "emoji".to_string(),
                    attrs: {
                        let mut map = HashMap::new();
                        map.insert("id".to_string(), AttrValue::Str(emoji_id.to_string()));
                        map
                    },
                    children: Vec::new(),
                };
            }
            if content == "\n" {
                element = Element {
                    tag: "br".to_string(),
                    attrs: HashMap::new(),
                    children: Vec::new(),
                };
            }
            elements.push(element);
        }
        match breakpoint.entity {
            Some(MessageEntity::Bold(_)) => {
                state.bold = breakpoint.mode == BreakpointMode::Start;
            }
            Some(MessageEntity::Italic(_)) => {
                state.italic = breakpoint.mode == BreakpointMode::Start;
            }
            Some(MessageEntity::Underline(_)) => {
                state.underline = breakpoint.mode == BreakpointMode::Start;
            }
            Some(MessageEntity::Strike(_)) => {
                state.strikethrough = breakpoint.mode == BreakpointMode::Start;
            }
            Some(MessageEntity::Code(_)) => {
                state.code = breakpoint.mode == BreakpointMode::Start;
            }
            Some(MessageEntity::Pre(_)) => {
                state.pre = breakpoint.mode == BreakpointMode::Start;
            }
            Some(MessageEntity::Spoiler(_)) => {
                state.spoiler = breakpoint.mode == BreakpointMode::Start;
            }
            Some(MessageEntity::Mention(_)) => {
                state.mention = breakpoint.mode == BreakpointMode::Start;
            }
            Some(MessageEntity::TextUrl(url)) => {
                state.link = match breakpoint.mode {
                    BreakpointMode::Start => Some(url.url.to_string()),
                    BreakpointMode::End => None,
                };
            }
            Some(MessageEntity::MentionName(mention)) => {
                state.user = match breakpoint.mode {
                    BreakpointMode::Start => Some(mention.user_id),
                    BreakpointMode::End => None,
                };
            }
            Some(MessageEntity::CustomEmoji(emoji)) => {
                state.emoji = match breakpoint.mode {
                    BreakpointMode::Start => Some(emoji.document_id),
                    BreakpointMode::End => None,
                };
            }
            Some(_) => unreachable!(),
            None => (),
        }
        last_pos = breakpoint.pos;
    }
    if last_pos < utf16.len() {
        elements.push(Element::text(String::from_utf16_lossy(&utf16[last_pos..])));
    }
    elements
}

fn extract_thread_id(message: &grammers_client::message::Message) -> Option<i32> {
    match message.reply_header() {
        Some(MessageReplyHeaderEnum::Header(header)) if header.forum_topic => {
            header.reply_to_top_id.or(header.reply_to_msg_id)
        }
        _ => None,
    }
}

struct ReplyInfo {
    msg_id: i32,
    peer_id: i64,
    quote: Option<(String, Option<Vec<MessageEntity>>)>,
}

fn extract_reply_info(message: &grammers_client::message::Message) -> Option<ReplyInfo> {
    match message.reply_header() {
        Some(MessageReplyHeaderEnum::Header(header))
            if !header.forum_topic || header.reply_to_top_id.is_some() =>
        {
            Some(ReplyInfo {
                msg_id: header.reply_to_msg_id?,
                peer_id: PeerId::from(header.reply_to_peer_id?).bot_api_dialog_id(),
                quote: header.quote_text.map(|text| (text, header.quote_entities)),
            })
        }
        _ => None,
    }
}

pub fn satori_elements_from_tg_message(
    self_id: i64,
    message: &grammers_client::message::Message,
) -> Vec<Element> {
    let mut elements = Vec::<Element>::new();

    if let Some(info) = extract_reply_info(message) {
        let mut quote_elements = vec![Element {
            tag: "author".to_string(),
            attrs: {
                let mut map = HashMap::new();
                map.insert("id".to_string(), AttrValue::Str(info.peer_id.to_string()));
                map
            },
            children: Vec::new(),
        }];
        if let Some(quote) = info.quote {
            quote_elements.append(&mut satori_element_from_tg_entities(
                &quote.0,
                quote.1.as_deref(),
            ));
        }
        elements.push(Element {
            tag: "quote".to_string(),
            attrs: {
                let mut map = HashMap::new();
                map.insert("id".to_string(), AttrValue::Str(info.msg_id.to_string()));
                map
            },
            children: quote_elements,
        });
    }

    elements.append(&mut satori_element_from_tg_entities(
        message.text(),
        message.fmt_entities().map(Vec::as_slice),
    ));

    match message.media() {
        Some(Media::Geo(location)) => {
            elements.push(Element {
                tag: "location".to_string(),
                attrs: {
                    let mut map = HashMap::new();
                    map.insert(
                        "lat".to_string(),
                        AttrValue::Str(location.latitue().to_string()),
                    );
                    map.insert(
                        "lon".to_string(),
                        AttrValue::Str(location.longitude().to_string()),
                    );
                    map
                },
                children: Vec::new(),
            });
        }
        Some(Media::Photo(photo)) => {
            if let Some(src) = satori_link_from_tg_photo(self_id, &photo) {
                elements.push(Element {
                    tag: "img".to_string(),
                    attrs: {
                        let mut map = HashMap::new();
                        map.insert("src".to_string(), AttrValue::Str(src));
                        map
                    },
                    children: Vec::new(),
                });
            }
        }
        Some(Media::Sticker(sticker)) => {
            if let Some(src) = satori_link_from_tg_document(self_id, &sticker.document) {
                elements.push(Element {
                    tag: "img".to_string(),
                    attrs: {
                        let mut map = HashMap::new();
                        map.insert("src".to_string(), AttrValue::Str(src));
                        map.insert(
                            "title".to_string(),
                            AttrValue::Str(sticker.emoji().to_string()),
                        );
                        map
                    },
                    children: Vec::new(),
                });
            }
        }
        Some(Media::Document(document)) => {
            if let Some(src) = satori_link_from_tg_document(self_id, &document) {
                let tag = if document.is_animated() || document.raw.video || document.raw.round {
                    "video"
                } else if is_audio(&document.raw) || document.raw.voice {
                    "audio"
                } else {
                    "file"
                };
                elements.push(Element {
                    tag: tag.to_string(),
                    attrs: {
                        let mut map = HashMap::new();
                        map.insert("src".to_string(), AttrValue::Str(src));
                        if let Some(name) = document.name() {
                            map.insert("title".to_string(), AttrValue::Str(name.to_string()));
                        }
                        map
                    },
                    children: Vec::new(),
                });
            }
        }
        _ => {}
    }

    elements
}

pub fn satori_message_from_tg_message(
    self_id: i64,
    message: &grammers_client::message::Message,
) -> Message {
    Message {
        id: message.id().to_string(),
        content: dump(satori_elements_from_tg_message(self_id, message)),
        channel: message
            .peer()
            .map(|peer| satori_channel_from_tg_peer(peer, extract_thread_id(message))),
        guild: message
            .peer()
            .and_then(|peer| satori_guild_from_tg_peer(self_id, peer)),
        member: None,
        user: message
            .sender()
            .or_else(|| message.peer()) // Channel posts may have no sender
            .map(|peer| satori_user_from_tg_peer(self_id, peer)),
        created_at: Some(message.date().timestamp() as f64),
        updated_at: message.edit_date().map(|x| x.timestamp() as f64),
    }
}
