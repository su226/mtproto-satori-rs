use grammers_session::types::{PeerAuth, PeerId, PeerRef};
use ntex::http::{Response, StatusCode};
use ntex::web;
use serde::Deserialize;

use crate::convert::user::satori_user_from_tg_peer;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct UserGetParams {
    user_id: String,
}

pub async fn user_get(
    state: &AppState,
    _request_state: (),
    params: web::types::Json<UserGetParams>,
) -> Result<Response, AppError> {
    let peer = if let Ok(id) = params.0.user_id.parse::<i64>() {
        state
            .client
            .resolve_peer(PeerRef {
                id: PeerId::from_bot_api_dialog_id(id).ok_or_else(|| {
                    AppError::new(StatusCode::BAD_REQUEST, "Invalid user ID.".to_string())
                })?,
                auth: PeerAuth::default(),
            })
            .await?
    } else {
        state
            .client
            .resolve_username(&params.0.user_id)
            .await?
            .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, "User not found.".to_string()))?
    };
    let self_id = state
        .self_info_cache
        .lock()
        .await
        .get_id()
        .bot_api_dialog_id()
        .unwrap_or_default();
    Ok(web::HttpResponse::Ok().json(&satori_user_from_tg_peer(self_id, &peer)))
}
