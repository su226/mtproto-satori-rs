use async_stream::try_stream;
use futures_util::Stream;
use grammers_client::InvocationError;
use grammers_client::client::DownloadIter;
use ntex::http::Response;
use ntex::util::Bytes;
use ntex::web;

use crate::error::AppError;
use crate::state::AppState;
use crate::telegram::file_id::FileId;

fn download_stream(mut iter: DownloadIter) -> impl Stream<Item = Result<Bytes, InvocationError>> {
    try_stream! {
        while let Some(item) = iter.next().await? {
            yield Bytes::from(item);
        }
    }
}

pub async fn internal(
    state: &AppState,
    _request_state: (),
    path: web::types::Path<(i64, String)>,
) -> Result<Response, AppError> {
    let self_id = state
        .self_info_cache
        .lock()
        .await
        .get_id()
        .bot_api_dialog_id()
        .unwrap_or_default();
    let (user_id, path) = path.into_inner();
    if user_id != self_id {
        return Ok(web::HttpResponse::NotFound().body("Invalid user ID."));
    }
    match FileId::decode(&path) {
        Some(file_id) => Ok(web::HttpResponse::Ok().streaming(Box::pin(download_stream(
            state.client.iter_download(&file_id),
        )))),
        None => Ok(web::HttpResponse::NotFound().body("Invalid internal link.")),
    }
}
