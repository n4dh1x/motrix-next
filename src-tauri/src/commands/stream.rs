use crate::error::AppError;
use crate::services::http_api::StreamHeaderMap;
use crate::services::stream_downloader;
use tauri::AppHandle;

/// Pause an active stream download task by terminating its process.
#[tauri::command]
pub async fn stream_pause(task_id: String) -> Result<(), AppError> {
    stream_downloader::pause_stream_download(task_id)
        .await
        .map_err(|e| AppError::Engine(e))
}

/// Resume a paused stream download task.
#[tauri::command]
pub async fn stream_resume(
    app_handle: AppHandle,
    task_id: String,
    url: String,
    title: String,
    quality: String,
    headers: Option<StreamHeaderMap>,
) -> Result<(), AppError> {
    stream_downloader::resume_stream_download(app_handle, task_id, url, title, quality, headers)
        .await
        .map_err(|e| AppError::Engine(e))
}

/// Delete a stream task and optionally its files from disk.
#[tauri::command]
pub async fn stream_delete(
    task_id: String,
    delete_file: Option<bool>,
    output_path: Option<String>,
) -> Result<(), AppError> {
    stream_downloader::delete_stream_download(task_id, delete_file.unwrap_or(false), output_path)
        .await
        .map_err(|e| AppError::Engine(e))
}
