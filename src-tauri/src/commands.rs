use serde::Serialize;
use tauri::State;

use crate::{AppState, readers};

#[derive(Serialize)]
pub struct OpenFileResult {
    pub frame_count: usize,
}

/// Returns the port the embedded HTTP server is listening on.
/// The frontend uses this to construct image/metadata URLs.
#[tauri::command]
pub fn get_server_port(state: State<'_, AppState>) -> u16 {
    state.server_port
}

/// Open an NXS/HDF5 file and make it the active file for the embedded server.
/// Returns the number of frames in the file.
#[tauri::command]
pub async fn open_file(
    path: String,
    state: State<'_, AppState>,
) -> Result<OpenFileResult, String> {
    tracing::info!("Opening file: {path}");

    let (reader, frame_count) = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let reader = readers::open(std::path::Path::new(&path))?;
        let frame_count = reader.frame_count()?;
        Ok((reader, frame_count))
    })
    .await
    .map_err(|e| format!("task error: {e}"))?
    .map_err(|e| format!("failed to open file: {e}"))?;

    tracing::info!("Opened file: {frame_count} frames");
    *state.reader.lock().await = Some(reader);

    Ok(OpenFileResult { frame_count })
}
