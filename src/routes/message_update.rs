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
use crate::error::AppError;
use crate::satori::element::parse;
use crate::state::AppState;
use crate::telegram::add_reply_markup;

#[derive(Deserialize)]
pub struct MessageUpdateParams {
    channel_id: String,
    message_id: String,
    content: String,
}

pub async fn message_update(
    state: &AppState,
    _request_state: (),
    params: web::types::Json<MessageUpdateParams>,
) -> Result<Response, AppError> {
    let (peer_id, thread_id) =
        tg_peer_id_from_satori_channel_id(&state.client, &params.0.channel_id).await?;
    let message_id = params
        .0
        .message_id
        .parse::<i32>()
        .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "Bad message ID.".to_string()))?;
    debug!(
        "Updating message {} from {}:{:?}",
        message_id, peer_id, thread_id
    );
    let peer = PeerRef {
        id: peer_id,
        auth: PeerAuth::default(),
    };
    let elements = parse(&params.0.content)
        .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, "Bad message.".to_string()))?;
    let infos = fetch_infos(&state.client, &elements).await?;
    let mut encoder = MessageEncoder::new(infos);
    encoder.render(&elements);
    encoder.flush();
    let pack = MessagePack::merge(encoder.packs);
    let mut message = InputMessage::new()
        .text(pack.content)
        .fmt_entities(pack.entities);
    if let Some(media) = pack.assets.first() {
        let self_id = state
            .self_info_cache
            .lock()
            .await
            .get_id()
            .bot_api_dialog_id()
            .unwrap_or_default();
        message = attach_media(message, &state.client, self_id, media, &state.names).await?;
    }
    let message = add_reply_markup(message, to_reply_markup(&pack.buttons));
    state.client.edit_message(peer, message_id, message).await?;
    Ok(Response::Ok().build())
}
