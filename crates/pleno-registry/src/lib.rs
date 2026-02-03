use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

pub async fn list_packages(State(state): State<AppState>) -> impl IntoResponse {
    let db = state.db.lock().unwrap();
    let mut stmt = db
        .prepare("SELECT name, version, description, author FROM packages")
        .unwrap();
    let rows = stmt
        .query_map([], |row| {
            Ok(PackageInfo {
                name: row.get(0)?,
                version: row.get(1)?,
                description: row.get(2)?,
                author: row.get(3)?,
            })
        })
        .unwrap();

    let packages: Vec<PackageInfo> = rows.filter_map(Result::ok).collect();
    Json(packages)
}

pub async fn get_package_info(
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let db = state.db.lock().unwrap();
    let mut stmt = db
        .prepare("SELECT name, version, description, author FROM packages WHERE name = ?")
        .unwrap();
    let row = stmt.query_row([name], |row| {
        Ok(PackageInfo {
            name: row.get(0)?,
            version: row.get(1)?,
            description: row.get(2)?,
            author: row.get(3)?,
        })
    });

    match row {
        Ok(pkg) => (StatusCode::OK, Json(pkg)).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn init_db(db: &Connection) {
    db.execute(
        "CREATE TABLE IF NOT EXISTS packages (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            version TEXT NOT NULL,
            description TEXT,
            author TEXT
        )",
        [],
    )
    .unwrap();
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/packages", get(list_packages))
        .route("/packages/:name", get(get_package_info))
        .with_state(state)
}
