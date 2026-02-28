use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use tower_http::cors::{Any, CorsLayer};

use crate::SharedReader;

#[derive(Clone)]
struct ServerState {
    reader: SharedReader,
}

pub fn create_router(reader: SharedReader) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/metadata", axum::routing::get(get_metadata))
        .route("/image/{frame}", axum::routing::get(get_image))
        .with_state(ServerState { reader })
        .layer(cors)
}

/// Return detector metadata for the currently-open file as JSON.
/// The `?v=...` query param used by the frontend for cache-busting is ignored.
async fn get_metadata(State(state): State<ServerState>) -> impl IntoResponse {
    let reader_arc = state.reader.clone();

    let result = tokio::task::spawn_blocking(move || {
        let guard = reader_arc.blocking_lock();
        let Some(reader) = guard.as_ref() else {
            return Err("No file open".to_string());
        };
        reader.metadata().map_err(|e| e.to_string())
    })
    .await;

    match result {
        Ok(Ok(meta)) => Json(meta).into_response(),
        Ok(Err(e)) => {
            tracing::error!("metadata read error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "No file open").into_response(),
    }
}

/// Return a raw frame as little-endian u16 bytes (application/octet-stream).
/// `:frame` is a 0-based frame index.
async fn get_image(
    State(state): State<ServerState>,
    Path(frame): Path<usize>,
) -> impl IntoResponse {
    let reader_arc = state.reader.clone();

    let result = tokio::task::spawn_blocking(move || {
        let guard = reader_arc.blocking_lock();
        let Some(reader) = guard.as_ref() else {
            return Err("No file open".to_string());
        };
        reader.read_frame(frame).map_err(|e| e.to_string())
    })
    .await;

    match result {
        Ok(Ok((pixels, _width, _height))) => {
            let bytes: Vec<u8> = pixels.iter().flat_map(|&v| v.to_le_bytes()).collect();
            (
                [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
                bytes,
            )
                .into_response()
        }
        Ok(Err(e)) => {
            tracing::error!("frame read error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
        Err(e) => {
            tracing::error!("spawn_blocking panicked: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
