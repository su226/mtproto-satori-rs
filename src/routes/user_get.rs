use std::sync::Arc;

use grammers_client::Client;
use grammers_session::types::{PeerAuth, PeerRef};
use ntex::http::{Response, StatusCode};
use ntex::web;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::convert::user::satori_user_from_tg_peer;
use crate::error::WebError;
use crate::self_info_cache::SelfInfoCache;
use crate::telegram::peer_id_from_bot_api_id;

#[derive(Deserialize)]
struct UserGetParams {
    user_id: String,
}

#[web::post("/v1/user.get")]
async fn user_get(
    client: web::types::State<Arc<Client>>,
    self_info_cache: web::types::State<Arc<Mutex<SelfInfoCache>>>,
    params: web::types::Json<UserGetParams>,
) -> Result<Response, WebError> {
    let peer = if let Ok(id) = params.0.user_id.parse::<i64>() {
        client
            .resolve_peer(PeerRef {
                id: peer_id_from_bot_api_id(id).ok_or_else(|| {
                    WebError::new(StatusCode::BAD_REQUEST, "Invalid user ID.".to_string())
                })?,
                auth: PeerAuth::default(),
            })
            .await?
    } else {
        client
            .resolve_username(&params.0.user_id)
            .await?
            .ok_or_else(|| WebError::new(StatusCode::BAD_REQUEST, "User not found.".to_string()))?
    };
    Ok(web::HttpResponse::Ok().json(&satori_user_from_tg_peer(
        self_info_cache.lock().await.get_id().bot_api_dialog_id(),
        &peer,
    )))
}
