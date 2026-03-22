use std::sync::Arc;

use async_stream::try_stream;
use futures_util::Stream;
use grammers_client::client::DownloadIter;
use grammers_client::{Client, InvocationError};
use ntex::http::Response;
use ntex::util::Bytes;
use ntex::web;
use tokio::sync::Mutex;

use crate::error::WebError;
use crate::self_info_cache::SelfInfoCache;
use crate::telegram::file_id::FileId;

fn download_stream(mut iter: DownloadIter) -> impl Stream<Item = Result<Bytes, InvocationError>> {
    try_stream! {
        while let Some(item) = iter.next().await? {
            yield Bytes::from(item);
        }
    }
}

#[web::get("/v1/proxy/internal:telegram/{user_id}/{path}")]
async fn internal(
    client: web::types::State<Arc<Client>>,
    self_info_manager: web::types::State<Arc<Mutex<SelfInfoCache>>>,
    path: web::types::Path<(i64, String)>,
) -> Result<Response, WebError> {
    let self_id = self_info_manager.lock().await.get_id().bot_api_dialog_id();
    let (user_id, path) = path.into_inner();
    if user_id != self_id {
        return Ok(web::HttpResponse::NotFound().body("Invalid user ID."));
    }
    match FileId::decode(&path) {
        Some(file_id) => Ok(web::HttpResponse::Ok()
            .streaming(Box::pin(download_stream(client.iter_download(&file_id))))),
        None => Ok(web::HttpResponse::NotFound().body("Invalid internal link.")),
    }
}
