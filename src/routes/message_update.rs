use std::sync::Arc;

use grammers_client::Client;
use grammers_client::message::InputMessage;
use grammers_session::types::{PeerAuth, PeerRef};
use log::debug;
use ntex::{
    http::{Response, StatusCode},
    web::{self, types::State},
};
use serde::Deserialize;

use crate::{
    convert::{
        channel::tg_peer_id_from_satori_channel_id,
        message_send::{MessageEncoder, fetch_infos},
    },
    error::MyError,
    satori::element::parse,
};

#[derive(Deserialize)]
struct MessageUpdateParams {
    channel_id: String,
    message_id: String,
    content: String,
}

#[web::post("/v1/message.update")]
async fn message_update(
    client: State<Arc<Client>>,
    params: web::types::Json<MessageUpdateParams>,
) -> Result<Response, MyError> {
    let (peer_id, thread_id) =
        tg_peer_id_from_satori_channel_id(&*client, &params.0.channel_id).await?;
    let message_id = params
        .0
        .message_id
        .parse::<i32>()
        .map_err(|_| MyError::new(StatusCode::BAD_REQUEST, "Bad message ID.".to_string()))?;
    debug!(
        "Updating message {} from {}:{:?}",
        message_id, peer_id, thread_id
    );
    let peer = PeerRef {
        id: peer_id,
        auth: PeerAuth::default(),
    };
    let elements = parse(&params.0.content)
        .ok_or_else(|| MyError::new(StatusCode::BAD_REQUEST, "Bad message.".to_string()))?;
    let infos = fetch_infos(&*client, &elements).await?;
    let mut encoder = MessageEncoder::new(infos);
    encoder.render(&elements);
    encoder.flush();
    let content = encoder
        .packs
        .into_iter()
        .map(|x| x.content)
        .collect::<String>();
    client
        .edit_message(peer, message_id, InputMessage::new().html(content))
        .await?;
    Ok(Response::Ok().finish())
}
