use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_tempfile::TempFile;
use futures_util::StreamExt;
use log::{debug, warn};
use ntex::http::Response;
use ntex::time::sleep;
use ntex::web;
use ntex_multipart::Multipart;
use reqwest::StatusCode;
use tokio::fs::{self, DirEntry};
use tokio::io::AsyncWriteExt;

use crate::convert::message_send::{extension_match, get_mime_extension};
use crate::error::AppError;
use crate::state::{AppState, SessionName};

pub async fn upload_create(
    state: &AppState,
    _request_state: (),
    mut params: Multipart,
) -> Result<Response, AppError> {
    let mut files = HashMap::<String, String>::new();
    let self_id = state
        .self_info_cache
        .lock()
        .await
        .get_id()
        .bot_api_dialog_id()
        .unwrap_or_default();
    fs::create_dir_all(&state.names.files_dir_path).await?;
    while let Some(item) = params.next().await {
        // Uploading to Telegram requires size to be known, which is impossible in multipart.
        // Save to temp file instead.
        let mut field = item?;
        let content_type = field.content_type().ok_or(AppError::new(
            StatusCode::BAD_REQUEST,
            "Content-Type is required.".to_string(),
        ))?;
        let content_disposition = field.content_disposition().ok_or(AppError::new(
            StatusCode::BAD_REQUEST,
            "Content-Disposition is required.".to_string(),
        ))?;
        let name = content_disposition
            .get_name()
            .ok_or(AppError::new(
                StatusCode::BAD_REQUEST,
                "Name is required.".to_string(),
            ))?
            .to_string();
        let suffix = if let Some(filename) = content_disposition.get_filename() {
            format!("-{}", filename)
        } else {
            let ext = get_mime_extension(content_type.essence_str());
            if extension_match(&name, ext) {
                format!("-{}", name)
            } else {
                format!("-{}.{}", name, ext)
            }
        };
        let mut tempfile = TempFile::builder()
            .dir(&state.names.files_dir_path)
            .suffix(suffix)
            .create()
            .await?;
        let filename = tempfile
            .file_path()
            .file_name()
            .ok_or(AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create temporary file.".to_string(),
            ))?
            .to_str()
            .ok_or(AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create temporary file.".to_string(),
            ))?
            .to_string();
        while let Some(chunk) = field.next().await {
            tempfile.write_all(&chunk?).await?;
        }
        tempfile.keep();
        files.insert(
            name.clone(),
            format!("internal:telegram/{}/_tmp/{}", self_id, filename),
        );
    }
    Ok(web::HttpResponse::Ok().json(&files))
}

async fn clean_entry(entry: &DirEntry) -> io::Result<()> {
    let stat = entry.metadata().await?;
    let modified = stat.modified()?;
    let duration = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default();
    if duration > Duration::from_mins(5) {
        debug!("Cleaning uploaded file {:?}", entry);
        fs::remove_file(entry.path()).await?;
    }
    Ok(())
}

async fn do_clean_uploads(dir: &Path) -> io::Result<()> {
    let mut entries = fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if let Err(err) = clean_entry(&entry).await {
            warn!("Clean uploaded file {:?} failed: {:?}", entry, err);
        }
    }
    Ok(())
}

pub async fn clean_uploads(session_name: Arc<SessionName>) {
    loop {
        if let Err(err) = do_clean_uploads(&session_name.files_dir_path).await {
            warn!("Clean uploads failed: {:?}", err);
        }
        sleep(Duration::from_mins(5)).await;
    }
}
