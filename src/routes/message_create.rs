use std::mem::take;
use std::sync::Arc;

use grammers_client::Client;
use grammers_client::message::InputMessage;
use grammers_session::types::{PeerAuth, PeerRef};
use log::{debug, trace};
use ntex::http::{Response, StatusCode};
use ntex::web;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::convert::channel::tg_peer_id_from_satori_channel_id;
use crate::convert::message_receive::satori_message_from_tg_message;
use crate::convert::message_send::{MessageEncoder, fetch_infos, parse_reply, upload_medias};
use crate::error::MyError;
use crate::satori::element::parse;
use crate::satori::types::Message;
use crate::self_info_cache::SelfInfoCache;
use crate::session::SessionName;
use crate::telegram::add_reply_markup;

#[derive(Deserialize)]
struct MessageCreateParams {
    channel_id: String,
    content: String,
}

#[web::post("/v1/message.create")]
async fn message_create(
    client: web::types::State<Arc<Client>>,
    self_info_cache: web::types::State<Arc<Mutex<SelfInfoCache>>>,
    session_name: web::types::State<Arc<SessionName>>,
    params: web::types::Json<MessageCreateParams>,
) -> Result<Response, MyError> {
    let (peer_id, thread_id) =
        tg_peer_id_from_satori_channel_id(&*client, &params.0.channel_id).await?;
    debug!("Sending message to {}:{:?}", peer_id, thread_id);
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
    let mut results = Vec::new();
    for pack in encoder.packs {
        let reply = parse_reply(pack.reply.as_deref())?;
        trace!("Reply to {:?}", reply);
        if pack.asset.is_empty() {
            let markup = pack.reply_markup();
            let message = InputMessage::new()
                .html(pack.content)
                .reply_to(reply.or(thread_id));
            let message = add_reply_markup(message, markup);
            results.push(client.send_message(peer, message).await?);
        } else {
            let mut medias = upload_medias(&*client, &pack.asset, &session_name.0).await?;
            let buttons = pack.reply_markup();
            if let Some(buttons) = buttons {
                medias[0] = take(&mut medias[0]).reply_to(reply.or(thread_id));
                results.extend(client.send_album(peer, medias).await?.into_iter().flatten());
                results.push(
                    client
                        .send_message(
                            peer,
                            InputMessage::new()
                                .html(pack.content)
                                .reply_to(Some(results[0].id()))
                                .reply_markup(buttons),
                        )
                        .await?,
                );
            } else {
                medias[0] = take(&mut medias[0])
                    .html(pack.content)
                    .reply_to(reply.or(thread_id));
                results.extend(client.send_album(peer, medias).await?.into_iter().flatten());
            }
        }
    }
    let self_id = self_info_cache.lock().await.get_id().bot_api_dialog_id();
    Ok(Response::Ok().json(
        &results
            .into_iter()
            .map(|message| satori_message_from_tg_message(self_id, &message))
            .collect::<Vec<Message>>(),
    ))
}
