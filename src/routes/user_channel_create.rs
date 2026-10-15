use ntex::http::{Response, StatusCode};
use ntex::web;
use presence_rs::Presence;
use serde::Deserialize;

use crate::error::AppError;
use crate::satori::types::{Channel, ChannelType};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct UserChannelCreateParams {
    user_id: String,
    #[allow(unused)]
    guild_id: Option<String>,
}

pub async fn user_channel_create(
    state: &AppState,
    _request_state: (),
    params: web::types::Json<UserChannelCreateParams>,
) -> Result<Response, AppError> {
    let peer_id = if let Ok(id) = params.0.user_id.parse::<i64>() {
        id
    } else {
        state
            .client
            .resolve_username(&params.0.user_id)
            .await?
            .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "User not found.".to_string()))?
            .id()
            .bot_api_dialog_id()
            .unwrap_or_default()
    };
    Ok(web::HttpResponse::Ok().json(&Channel {
        id: peer_id.to_string(),
        channel_type: ChannelType::Direct,
        name: Presence::Absent,
        parent_id: Presence::Null,
    }))
}
