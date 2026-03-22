use std::sync::Arc;

use grammers_client::Client;
use ntex::http::{Response, StatusCode};
use ntex::web;
use serde::Deserialize;

use crate::error::MyError;
use crate::satori::types::{Channel, ChannelType};

#[derive(Deserialize)]
struct UserChannelCreateParams {
    user_id: String,
    #[allow(unused)]
    guild_id: Option<String>,
}

#[web::post("/v1/user.channel.create")]
async fn user_channel_create(
    client: web::types::State<Arc<Client>>,
    params: web::types::Json<UserChannelCreateParams>,
) -> Result<Response, MyError> {
    let peer_id = if let Ok(id) = params.0.user_id.parse::<i64>() {
        id
    } else {
        client
            .resolve_username(&params.0.user_id)
            .await?
            .ok_or_else(|| MyError::new(StatusCode::NOT_FOUND, "User not found.".to_string()))?
            .id()
            .bot_api_dialog_id()
    };
    Ok(web::HttpResponse::Ok().json(&Channel {
        id: peer_id.to_string(),
        channel_type: ChannelType::Direct,
        name: None,
        parent_id: None,
    }))
}
