use std::sync::Arc;

use ntex::http::Response;
use ntex::web;
use tokio::sync::Mutex;

use crate::convert::login::satori_login_from_tg_user;
use crate::error::MyError;
use crate::self_info_cache::SelfInfoCache;

#[web::post("/v1/login.get")]
async fn login_get(
    self_info_manager: web::types::State<Arc<Mutex<SelfInfoCache>>>,
) -> Result<Response, MyError> {
    let user = self_info_manager.lock().await.get().await?;
    Ok(web::HttpResponse::Ok().json(&satori_login_from_tg_user(&user)))
}
