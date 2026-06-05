//! Dedicated stream downloader module using bundled yt-dlp and ffmpeg sidecars.
//!
//! Handles resolving sidecar paths, setting up temporary flat ffmpeg directories,
//! spawning yt-dlp, and emitting progress updates to the frontend.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_shell::ShellExt;
use serde::Serialize;
use crate::services::http_api::{StreamTaskRequest, StreamHeaderMap};

static TASK_COUNTER: AtomicUsize = AtomicUsize::new(1);
static RUNNING_PROCESSES: OnceLock<Mutex<HashMap<String, tauri_plugin_shell::process::CommandChild>>> = OnceLock::new();

fn get_running_processes() -> &'static Mutex<HashMap<String, tauri_plugin_shell::process::CommandChild>> {
    RUNNING_PROCESSES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Serialize, Clone)]
struct ProgressPayload {
    #[serde(rename = "taskId")]
    task_id: String,
    percent: f32,
    speed: String,
    eta: String,
    status: String,
}

#[derive(Serialize, Clone)]
struct StatusPayload {
    #[serde(rename = "taskId")]
    task_id: String,
    status: String,
    #[serde(rename = "outputPath")]
    output_path: String,
}

/// Helper to copy architecture-suffixed ffmpeg sidecar to a flat ffmpeg binary inside the cache directory.
async fn prepare_ffmpeg_binary(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let target_triple = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else {
        return Err("Unsupported OS / CPU Architecture for stream downloads".to_string());
    };

    let ext = if cfg!(target_os = "windows") { ".exe" } else { "" };
    
    // 1. Locate bundled sidecar path relative to the current running executable directory.
    // In dev mode, it is named "ffmpeg-<triple>", whereas in production, Tauri removes the suffix.
    let app_dir = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {e}"))?
        .parent()
        .ok_or_else(|| "Failed to get parent directory of executable".to_string())?
        .to_path_buf();

    let suffix_name = format!("ffmpeg-{}{}", target_triple, ext);
    let mut ffmpeg_sidecar = app_dir.join(&suffix_name);

    if !ffmpeg_sidecar.exists() {
        let plain_name = format!("ffmpeg{}", ext);
        ffmpeg_sidecar = app_dir.join(&plain_name);
    }

    if !ffmpeg_sidecar.exists() {
        // Fallback: check resource directory
        let sidecar_filename = format!("binaries/ffmpeg-{}{}", target_triple, ext);
        if let Ok(res_path) = app_handle.path().resolve(&sidecar_filename, tauri::path::BaseDirectory::Resource) {
            if res_path.exists() {
                ffmpeg_sidecar = res_path;
            }
        }
    }

    if !ffmpeg_sidecar.exists() {
        return Err(format!(
            "Bundled ffmpeg sidecar not found. Checked: {:?}/{} and {:?}/ffmpeg{}",
            app_dir, suffix_name, app_dir, ext
        ));
    }

    // 2. Prepare destination path in app cache directory
    let cache_dir = app_handle.path().app_cache_dir().map_err(|e| e.to_string())?;
    let bin_dir = cache_dir.join("stream_bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;

    let dest_filename = format!("ffmpeg{}", ext);
    let ffmpeg_dest = bin_dir.join(dest_filename);

    // 3. Copy if not exists or size differs
    if !ffmpeg_dest.exists() {
        std::fs::copy(&ffmpeg_sidecar, &ffmpeg_dest).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&ffmpeg_dest).map_err(|e| e.to_string())?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&ffmpeg_dest, perms).map_err(|e| e.to_string())?;
        }
    }

    Ok(bin_dir)
}

pub async fn execute_stream_download(
    app_handle: AppHandle,
    task_id: Option<String>,
    task: StreamTaskRequest,
) -> Result<(), String> {
    let task_id = task_id.unwrap_or_else(|| {
        let index = TASK_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("stream-{}", index)
    });
    
    // Resolve output target directory (Downloads)
    let download_dir = app_handle
        .path()
        .download_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    let safe_title = sanitize_filename::sanitize(&task.title);
    let output_file = download_dir.join(format!("{}.mp4", safe_title));
    let output_file_str = output_file.to_string_lossy().to_string();

    // Prepare temp bin folder containing un-suffixed ffmpeg
    let ffmpeg_location = prepare_ffmpeg_binary(&app_handle).await?;
    let ffmpeg_location_str = ffmpeg_location.to_string_lossy().to_string();

    // Map quality constraint to format selector
    let mut format_selector = "bestvideo+bestaudio/best".to_string();
    if task.quality != "Best Quality" {
        // e.g. "1080p Track" -> parse numbers
        let numbers: String = task.quality.chars().filter(|c| c.is_ascii_digit()).collect();
        if !numbers.is_empty() {
            if let Ok(height) = numbers.parse::<u32>() {
                format_selector = format!("bestvideo[height<={}]+bestaudio/best", height);
            }
        }
    }

    let mut args = vec![
        "--newline".to_string(),
        "--ffmpeg-location".to_string(),
        ffmpeg_location_str,
        "-f".to_string(),
        format_selector,
        "--merge-output-format".to_string(),
        "mp4".to_string(),
        "-o".to_string(),
        output_file_str.clone(),
    ];

    // Inject headers passed down from extension to authorize requests
    if let Some(ref headers) = task.headers {
        if let Some(ref ua) = headers.user_agent {
            args.push("--user-agent".to_string());
            args.push(ua.clone());
        }
        if let Some(ref ref_url) = headers.referer {
            args.push("--referer".to_string());
            args.push(ref_url.clone());
        }
        if let Some(ref cookie) = headers.cookie {
            args.push("--add-header".to_string());
            args.push(format!("Cookie:{}", cookie));
        }
    }

    args.push(task.url.clone());

    // Emit initialization info (including URL and headers so frontend store can persist it)
    let _ = app_handle.emit("stream-added", serde_json::json!({
        "id": task_id,
        "title": task.title,
        "quality": task.quality,
        "status": "Starting",
        "url": task.url,
        "headers": task.headers
    }));

    log::info!("stream_downloader: Spawning yt-dlp sidecar for task {}", task_id);

    // Spawn sidecar yt-dlp
    let command = app_handle
        .shell()
        .sidecar("yt-dlp")
        .map_err(|e| e.to_string())?
        .args(args);

    let (mut rx, child) = command.spawn().map_err(|e| e.to_string())?;

    // Insert child handle into running processes
    get_running_processes().lock().unwrap().insert(task_id.clone(), child);

    while let Some(event) = rx.recv().await {
        if let tauri_plugin_shell::process::CommandEvent::Stdout(line_bytes) = event {
            let line = String::from_utf8_lossy(&line_bytes);
            let trimmed = line.trim();
            if trimmed.starts_with("[download]") {
                let content = trimmed["[download]".len()..].trim();
                let tokens: Vec<&str> = content.split_whitespace().collect();
                if !tokens.is_empty() && tokens[0].ends_with('%') {
                    let percent: f32 = tokens[0].trim_end_matches('%').parse().unwrap_or(0.0);
                    
                    let speed = if tokens.len() >= 5 && tokens[3] == "at" {
                        tokens[4].to_string()
                    } else {
                        "0 B/s".to_string()
                    };

                    let eta = if tokens.len() >= 7 && tokens[5] == "ETA" {
                        tokens[6].to_string()
                    } else {
                        "--:--".to_string()
                    };

                    let _ = app_handle.emit("stream-progress", ProgressPayload {
                        task_id: task_id.clone(),
                        percent,
                        speed,
                        eta,
                        status: "Downloading".to_string(),
                    });
                }
            }
        } else if let tauri_plugin_shell::process::CommandEvent::Stderr(line_bytes) = event {
            let line = String::from_utf8_lossy(&line_bytes);
            log::warn!("stream_downloader [stderr]: {}", line.trim());
        }
    }

    // After loop, check if it was paused/deleted or completed
    let mut processes = get_running_processes().lock().unwrap();
    if processes.remove(&task_id).is_some() {
        // It finished on its own (was not removed by pause/delete commands)
        let success = std::path::Path::new(&output_file_str).exists();
        if success {
            log::info!("stream_downloader: Task {} completed, output file: {}", task_id, output_file_str);
            let _ = app_handle.emit("stream-status", StatusPayload {
                task_id,
                status: "Completed".to_string(),
                output_path: output_file_str,
            });
        } else {
            log::error!("stream_downloader: Task {} failed", task_id);
            let _ = app_handle.emit("stream-status", StatusPayload {
                task_id,
                status: "Error".to_string(),
                output_path: String::new(),
            });
        }
    }
    Ok(())
}

pub async fn pause_stream_download(task_id: String) -> Result<(), String> {
    let mut processes = get_running_processes().lock().unwrap();
    if let Some(child) = processes.remove(&task_id) {
        let _ = child.kill();
        log::info!("stream_downloader: Task {} paused (killed process)", task_id);
        Ok(())
    } else {
        Err("Task is not active or already paused".to_string())
    }
}

pub async fn resume_stream_download(
    app_handle: AppHandle,
    task_id: String,
    url: String,
    title: String,
    quality: String,
    headers: Option<StreamHeaderMap>,
) -> Result<(), String> {
    let task = StreamTaskRequest {
        url,
        title,
        quality,
        headers,
    };
    // Re-execute with the same task ID
    tokio::spawn(async move {
        if let Err(e) = execute_stream_download(app_handle, Some(task_id), task).await {
            log::error!("stream_downloader: Failed to resume task: {e}");
        }
    });
    Ok(())
}

pub async fn delete_stream_download(
    task_id: String,
    delete_file: bool,
    output_path: Option<String>,
) -> Result<(), String> {
    // 1. Kill the process if running
    let mut processes = get_running_processes().lock().unwrap();
    if let Some(child) = processes.remove(&task_id) {
        let _ = child.kill();
        log::info!("stream_downloader: Killed active task {} for deletion", task_id);
    }

    // 2. Delete output files if requested
    if delete_file {
        if let Some(path_str) = output_path {
            if !path_str.is_empty() {
                let path = std::path::Path::new(&path_str);
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                    log::info!("stream_downloader: Deleted output file: {}", path_str);
                }
                
                // Also check and delete .part file if exists
                let part_path_str = format!("{}.part", path_str);
                let part_path = std::path::Path::new(&part_path_str);
                if part_path.exists() {
                    let _ = std::fs::remove_file(part_path);
                    log::info!("stream_downloader: Deleted partial file: {}", part_path_str);
                }
                
                // Also check and delete .ytdl file if exists
                let ytdl_path_str = format!("{}.ytdl", path_str);
                let ytdl_path = std::path::Path::new(&ytdl_path_str);
                if ytdl_path.exists() {
                    let _ = std::fs::remove_file(ytdl_path);
                    log::info!("stream_downloader: Deleted ytdl temp file: {}", ytdl_path_str);
                }
            }
        }
    }
    Ok(())
}
