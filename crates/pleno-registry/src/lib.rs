use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::Request,
    http::StatusCode,
    http::header::AUTHORIZATION,
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub description: String,
    pub author: String,
    pub versions: Vec<VersionInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublishRequest {
    pub name: String,
    pub version: String,
    pub git_url: String,
    pub commit: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

pub async fn list_packages(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let db = state.db.lock().unwrap();

    let mut packages = Vec::new();
    if let Some(q) = &query.q {
        // Better ranking: exact name match first, then name starts with, then description contains
        let q_lower = q.to_lowercase();
        let mut stmt = db
            .prepare(
                "SELECT name, description, author FROM packages 
             WHERE name = ? 
             OR name LIKE ? 
             OR description LIKE ?
             ORDER BY (name = ?) DESC, (name LIKE ?) DESC",
            )
            .unwrap();

        let rows = stmt
            .query_map(
                params![
                    q_lower,
                    format!("{}%", q_lower),
                    format!("%{}%", q_lower),
                    q_lower,
                    format!("{}%", q_lower)
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap();

        for row in rows.flatten() {
            packages.push(row);
        }
    } else {
        let mut stmt = db
            .prepare("SELECT name, description, author FROM packages")
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap();
        for row in rows.flatten() {
            packages.push(row);
        }
    };

    let mut result = Vec::new();
    for row in packages {
        let (name, desc, author) = row;

        let mut v_stmt = db
            .prepare("SELECT version, url, sha256 FROM versions WHERE package_name = ?")
            .unwrap();
        let v_rows = v_stmt
            .query_map([&name], |v_row| {
                Ok(VersionInfo {
                    version: v_row.get(0)?,
                    url: v_row.get(1)?,
                    sha256: v_row.get(2)?,
                })
            })
            .unwrap();

        result.push(PackageInfo {
            name,
            description: desc,
            author,
            versions: v_rows.flatten().collect(),
        });
    }

    Json(result)
}

pub async fn get_package_info(
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let db = state.db.lock().unwrap();
    let pkg_row = db.query_row(
        "SELECT name, description, author FROM packages WHERE name = ?",
        [name.clone()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    );

    match pkg_row {
        Ok((name, desc, _author)) => {
            let mut v_stmt = db
                .prepare("SELECT version, url, sha256 FROM versions WHERE package_name = ?")
                .unwrap();
            let v_rows = v_stmt
                .query_map([&name], |v_row| {
                    Ok(VersionInfo {
                        version: v_row.get(0)?,
                        url: v_row.get(1)?,
                        sha256: v_row.get(2)?,
                    })
                })
                .unwrap();

            Json(serde_json::json!({
                "name": name,
                "description": desc,
                "versions": v_rows.flatten().map(|v| (v.version.clone(), v)).collect::<std::collections::HashMap<_, _>>()
            })).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn publish_package(
    State(state): State<AppState>,
    Json(payload): Json<PublishRequest>,
) -> impl IntoResponse {
    // Validate semver
    if semver::Version::parse(&payload.version).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid semver version" })),
        )
            .into_response();
    }

    let db = state.db.lock().unwrap();

    // 1. Ensure package exists or create it
    let _ = db.execute(
        "INSERT OR IGNORE INTO packages (name, description, author) VALUES (?, ?, ?)",
        params![payload.name, "", "unknown"], // Basic info, could be updated from manifest
    );

    // 2. Insert version
    let url = format!("git+{}", payload.git_url);
    let res = db.execute(
        "INSERT INTO versions (package_name, version, url, sha256) VALUES (?, ?, ?, ?)",
        params![payload.name, payload.version, url, payload.sha256],
    );

    match res {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// Simple API Key authentication middleware
pub async fn auth_middleware(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<impl IntoResponse, StatusCode> {
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(auth) if auth.starts_with("Bearer ") => {
            let token = &auth[7..];
            // In a real app, check against DB. For now, check against env or hardcoded.
            let valid_token = std::env::var("PLENO_ADMIN_TOKEN")
                .unwrap_or_else(|_| "pleno_secret_token".to_string());
            if token == valid_token {
                Ok(next.run(req).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

pub fn init_db(db: &Connection) {
    db.execute(
        "CREATE TABLE IF NOT EXISTS packages (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            author TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )
    .unwrap();

    db.execute(
        "CREATE TABLE IF NOT EXISTS versions (
            id INTEGER PRIMARY KEY,
            package_name TEXT NOT NULL,
            version TEXT NOT NULL,
            url TEXT NOT NULL,
            sha256 TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(package_name, version),
            FOREIGN KEY(package_name) REFERENCES packages(name)
        )",
        [],
    )
    .unwrap();

    db.execute(
        "CREATE TABLE IF NOT EXISTS api_keys (
            id INTEGER PRIMARY KEY,
            key TEXT NOT NULL UNIQUE,
            owner TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )
    .unwrap();

    // Seed an initial key if table is empty
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM api_keys", [], |r| r.get(0))
        .unwrap_or(0);
    if count == 0 {
        db.execute(
            "INSERT INTO api_keys (key, owner) VALUES (?, ?)",
            params!["pleno_secret_token", "admin"],
        )
        .unwrap();
    }
}

pub fn create_router(state: AppState) -> Router {
    let auth_router = Router::new()
        .route("/packages/:name", post(publish_package))
        .layer(middleware::from_fn(auth_middleware))
        .with_state(state.clone());

    Router::new()
        .route("/packages", get(list_packages))
        .route("/packages/:name", get(get_package_info))
        .merge(auth_router)
        .with_state(state)
}
