use crate::{
    convert::user::satori_user_from_tg_peer, error::MyError, self_info_cache::SelfInfoCache,
    telegram::peer_id_from_bot_api_id,
};
use grammers_client::Client;
use grammers_session::types::{PeerAuth, PeerRef};
use ntex::{
    http::{Response, StatusCode},
    web::{self, types::State},
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Deserialize)]
struct UserGetParams {
    user_id: String,
}

#[web::post("/v1/user.get")]
async fn user_get(
    client: State<Arc<Client>>,
    self_info_cache: State<Arc<Mutex<SelfInfoCache>>>,
    params: web::types::Json<UserGetParams>,
) -> Result<Response, MyError> {
    let peer = if let Ok(id) = params.0.user_id.parse::<i64>() {
        client
            .resolve_peer(PeerRef {
                id: peer_id_from_bot_api_id(id).ok_or_else(|| {
                    MyError::new(StatusCode::BAD_REQUEST, "Invalid user ID.".to_string())
                })?,
                auth: PeerAuth::default(),
            })
            .await?
    } else {
        client
            .resolve_username(&params.0.user_id)
            .await?
            .ok_or_else(|| MyError::new(StatusCode::BAD_REQUEST, "User not found.".to_string()))?
    };
    Ok(web::HttpResponse::Ok().json(&satori_user_from_tg_peer(
        self_info_cache.lock().await.get_id().bot_api_dialog_id(),
        &peer,
    )))
}
