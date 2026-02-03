use pleno_registry::{AppState, create_router, init_db};
use rusqlite::Connection;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let conn = Connection::open("registry.db").expect("Failed to open database");
    init_db(&conn);

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
    };

    let app = create_router(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Pleno Registry listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
