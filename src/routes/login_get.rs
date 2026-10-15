use ntex::http::Response;
use ntex::web;

use crate::convert::login::satori_login_from_tg_user;
use crate::error::AppError;
use crate::state::AppState;

pub async fn login_get(state: &AppState, _request_state: ()) -> Result<Response, AppError> {
    let user = state.self_info_cache.lock().await.get().await?;
    Ok(web::HttpResponse::Ok().json(&satori_login_from_tg_user(&user)))
}
