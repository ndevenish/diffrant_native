mod commands;
mod readers;
mod server;

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub type SharedReader = Arc<Mutex<Option<Box<dyn readers::Reader>>>>;

#[derive(Clone)]
pub struct AppState {
    pub reader: SharedReader,
    pub server_port: u16,
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "diffrant_native=info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let reader: SharedReader = Arc::new(Mutex::new(None));

            // Bind on an OS-assigned port before starting the async server.
            // Must be set to non-blocking before handing to tokio.
            let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            let port = std_listener.local_addr()?.port();
            std_listener.set_nonblocking(true)?;
            tracing::info!("Starting embedded HTTP server on port {port}");

            let router = server::create_router(reader.clone());
            tauri::async_runtime::spawn(async move {
                let listener = tokio::net::TcpListener::from_std(std_listener)
                    .expect("failed to convert TcpListener");
                axum::serve(listener, router)
                    .await
                    .expect("embedded server error");
            });

            let state = AppState {
                reader,
                server_port: port,
            };
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_server_port,
            commands::open_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
