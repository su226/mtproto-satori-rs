use std::sync::Arc;

use grammers_client::Client;
use grammers_session::types::{PeerAuth, PeerRef};
use ntex::http::{Response, StatusCode};
use ntex::web;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::convert::channel::tg_peer_id_from_satori_channel_id;
use crate::convert::message_receive::satori_message_from_tg_message;
use crate::error::MyError;
use crate::self_info_cache::SelfInfoCache;

#[derive(Deserialize)]
struct MessageGetParams {
    channel_id: String,
    message_id: String,
}

#[web::post("/v1/message.get")]
async fn message_get(
    client: web::types::State<Arc<Client>>,
    self_info_cache: web::types::State<Arc<Mutex<SelfInfoCache>>>,
    params: web::types::Json<MessageGetParams>,
) -> Result<Response, MyError> {
    let (peer_id, _) = tg_peer_id_from_satori_channel_id(&*client, &params.channel_id).await?;
    let peer = PeerRef {
        id: peer_id,
        auth: PeerAuth::default(),
    };
    let message_id = params.message_id.parse::<i32>()?;
    let messages = client.get_messages_by_id(peer, &[message_id]).await?;
    let message_not_found = || {
        MyError::new(
            StatusCode::NOT_FOUND,
            "Message not found or deleted.".to_string(),
        )
    };
    let message = messages
        .first()
        .ok_or_else(message_not_found)?
        .as_ref()
        .ok_or_else(message_not_found)?;
    let self_id = self_info_cache.lock().await.get_id().bot_api_dialog_id();
    Ok(web::HttpResponse::Ok().json(&satori_message_from_tg_message(self_id, message)))
}
