use std::sync::Arc;

use grammers_client::Client;
use grammers_client::message::InputMessage;
use grammers_session::types::{PeerAuth, PeerRef};
use log::debug;
use ntex::http::{Response, StatusCode};
use ntex::web;
use serde::Deserialize;

use crate::convert::channel::tg_peer_id_from_satori_channel_id;
use crate::convert::message_send::{
    MessageEncoder,
    MessagePack,
    attach_media,
    fetch_infos,
    to_reply_markup,
};
use crate::error::WebError;
use crate::satori::element::parse;
use crate::session::SessionName;
use crate::telegram::add_reply_markup;

#[derive(Deserialize)]
struct MessageUpdateParams {
    channel_id: String,
    message_id: String,
    content: String,
}

#[web::post("/v1/message.update")]
async fn message_update(
    client: web::types::State<Arc<Client>>,
    session_name: web::types::State<Arc<SessionName>>,
    params: web::types::Json<MessageUpdateParams>,
) -> Result<Response, WebError> {
    let (peer_id, thread_id) =
        tg_peer_id_from_satori_channel_id(&client, &params.0.channel_id).await?;
    let message_id = params
        .0
        .message_id
        .parse::<i32>()
        .map_err(|_| WebError::new(StatusCode::BAD_REQUEST, "Bad message ID.".to_string()))?;
    debug!(
        "Updating message {} from {}:{:?}",
        message_id, peer_id, thread_id
    );
    let peer = PeerRef {
        id: peer_id,
        auth: PeerAuth::default(),
    };
    let elements = parse(&params.0.content)
        .ok_or_else(|| WebError::new(StatusCode::BAD_REQUEST, "Bad message.".to_string()))?;
    let infos = fetch_infos(&client, &elements).await?;
    let mut encoder = MessageEncoder::new(infos);
    encoder.render(&elements);
    encoder.flush();
    let pack = MessagePack::merge(encoder.packs);
    let mut message = InputMessage::new()
        .text(pack.content)
        .fmt_entities(pack.entities);
    if let Some(media) = pack.assets.first() {
        message = attach_media(message, &client, media, &session_name.0).await?;
    }
    let message = add_reply_markup(message, to_reply_markup(&pack.buttons));
    client.edit_message(peer, message_id, message).await?;
    Ok(Response::Ok().finish())
}
