use crate::{
    convert::login::satori_login_from_tg_user, error::MyError, self_info_cache::SelfInfoCache,
};
use ntex::{
    http::Response,
    web::{self, types::State},
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[web::post("/v1/login.get")]
async fn login_get(
    self_info_manager: State<Arc<Mutex<SelfInfoCache>>>,
) -> Result<Response, MyError> {
    let user = self_info_manager.lock().await.get().await?;
    Ok(web::HttpResponse::Ok().json(&satori_login_from_tg_user(&user)))
}
