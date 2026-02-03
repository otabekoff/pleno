use pleno_registry::{AppState, create_router, init_db};
use rusqlite::Connection;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("PLENO_REGISTRY_DB").unwrap_or_else(|_| "registry.db".to_string());
    let host = std::env::var("PLENO_REGISTRY_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PLENO_REGISTRY_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("Invalid PLENO_REGISTRY_PORT");

    let conn = Connection::open(db_path).expect("Failed to open database");
    init_db(&conn);

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
    };

    let app = create_router(state);

    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("Invalid address");
    println!("Pleno Registry listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
