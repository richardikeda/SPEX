use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use spex_core::{hash, pow, pow::PowParams};
use std::{
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::task;

#[derive(Clone)]
pub struct AppState {
    db_path: PathBuf,
    clock: Arc<dyn Clock + Send + Sync>,
}

pub trait Clock {
    fn now(&self) -> u64;
}

#[derive(Clone)]
struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[derive(Debug, Deserialize)]
struct StorageRequest {
    data: String,
    grant: GrantTokenPayload,
    puzzle: PuzzlePayload,
}

#[derive(Debug, Deserialize)]
struct GrantTokenPayload {
    user_id: String,
    role: u64,
    flags: Option<u64>,
    expires_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PuzzlePayload {
    recipient_key: String,
    puzzle_input: String,
    puzzle_output: String,
    params: Option<PowParamsPayload>,
}

#[derive(Debug, Deserialize)]
struct PowParamsPayload {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    output_len: usize,
}

#[derive(Debug, Serialize)]
struct StoredResponse {
    data: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, thiserror::Error)]
enum BridgeError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("puzzle validation failed")]
    PuzzleInvalid,
    #[error("grant expired")]
    GrantExpired,
    #[error("hash mismatch")]
    HashMismatch,
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not found")]
    NotFound,
}

impl IntoResponse for BridgeError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            BridgeError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            BridgeError::PuzzleInvalid => StatusCode::UNAUTHORIZED,
            BridgeError::GrantExpired => StatusCode::UNAUTHORIZED,
            BridgeError::HashMismatch => StatusCode::BAD_REQUEST,
            BridgeError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::NotFound => StatusCode::NOT_FOUND,
        };
        let body = Json(ErrorResponse {
            error: self.to_string(),
        });
        (status, body).into_response()
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/cards/:card_hash", put(put_card).get(get_card))
        .route("/slot/:slot_id", put(put_slot).get(get_slot))
        .with_state(state)
}

pub fn init_state(db_path: impl Into<PathBuf>) -> Result<AppState, BridgeError> {
    let db_path = db_path.into();
    init_db(&db_path).map_err(|err| BridgeError::Storage(err.to_string()))?;
    Ok(AppState {
        db_path,
        clock: Arc::new(SystemClock),
    })
}

pub fn init_state_with_clock(
    db_path: impl Into<PathBuf>,
    clock: Arc<dyn Clock + Send + Sync>,
) -> Result<AppState, BridgeError> {
    let db_path = db_path.into();
    init_db(&db_path).map_err(|err| BridgeError::Storage(err.to_string()))?;
    Ok(AppState { db_path, clock })
}

fn init_db(path: &FsPath) -> rusqlite::Result<()> {
    let conn = Connection::open(path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cards (card_hash TEXT PRIMARY KEY, data BLOB NOT NULL)",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS slots (slot_id TEXT PRIMARY KEY, data BLOB NOT NULL)",
        [],
    )?;
    Ok(())
}

async fn put_card(
    State(state): State<AppState>,
    Path(card_hash): Path<String>,
    Json(payload): Json<StorageRequest>,
) -> Result<StatusCode, BridgeError> {
    validate_storage_request(&state, &payload)?;
    let data = decode_base64(&payload.data)?;
    validate_hash(&card_hash, &data)?;

    store_entry(&state.db_path, "cards", "card_hash", &card_hash, data).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_card(
    State(state): State<AppState>,
    Path(card_hash): Path<String>,
) -> Result<Json<StoredResponse>, BridgeError> {
    let data = load_entry(&state.db_path, "cards", "card_hash", &card_hash)
        .await?
        .ok_or(BridgeError::NotFound)?;
    Ok(Json(StoredResponse {
        data: BASE64.encode(data),
    }))
}

async fn put_slot(
    State(state): State<AppState>,
    Path(slot_id): Path<String>,
    Json(payload): Json<StorageRequest>,
) -> Result<StatusCode, BridgeError> {
    validate_storage_request(&state, &payload)?;
    let data = decode_base64(&payload.data)?;

    store_entry(&state.db_path, "slots", "slot_id", &slot_id, data).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_slot(
    State(state): State<AppState>,
    Path(slot_id): Path<String>,
) -> Result<Json<StoredResponse>, BridgeError> {
    let data = load_entry(&state.db_path, "slots", "slot_id", &slot_id)
        .await?
        .ok_or(BridgeError::NotFound)?;
    Ok(Json(StoredResponse {
        data: BASE64.encode(data),
    }))
}

fn validate_storage_request(state: &AppState, payload: &StorageRequest) -> Result<(), BridgeError> {
    validate_grant(state.clock.now(), &payload.grant)?;
    validate_puzzle(&payload.puzzle)?;
    Ok(())
}

fn validate_grant(now: u64, grant: &GrantTokenPayload) -> Result<(), BridgeError> {
    let _user_id = decode_base64(&grant.user_id)?;
    if let Some(expires_at) = grant.expires_at {
        if expires_at <= now {
            return Err(BridgeError::GrantExpired);
        }
    }
    let _ = grant.role;
    let _ = grant.flags;
    Ok(())
}

fn validate_puzzle(payload: &PuzzlePayload) -> Result<(), BridgeError> {
    let recipient_key = decode_base64(&payload.recipient_key)?;
    let puzzle_input = decode_base64(&payload.puzzle_input)?;
    let puzzle_output = decode_base64(&payload.puzzle_output)?;
    let params = payload
        .params
        .as_ref()
        .map(|custom| PowParams {
            memory_kib: custom.memory_kib,
            iterations: custom.iterations,
            parallelism: custom.parallelism,
            output_len: custom.output_len,
        })
        .unwrap_or_default();

    let valid = pow::verify_puzzle_output(&recipient_key, &puzzle_input, &puzzle_output, params)
        .map_err(|err| BridgeError::InvalidRequest(err.to_string()))?;
    if !valid {
        return Err(BridgeError::PuzzleInvalid);
    }
    Ok(())
}

fn validate_hash(card_hash: &str, data: &[u8]) -> Result<(), BridgeError> {
    let expected = hash::hash_bytes(hash::HashId::Sha256, data);
    let expected_hex = hex::encode(expected);
    if expected_hex != card_hash {
        return Err(BridgeError::HashMismatch);
    }
    Ok(())
}

fn decode_base64(value: &str) -> Result<Vec<u8>, BridgeError> {
    BASE64
        .decode(value)
        .map_err(|err| BridgeError::InvalidRequest(err.to_string()))
}

async fn store_entry(
    db_path: &PathBuf,
    table: &str,
    key_col: &str,
    key: &str,
    data: Vec<u8>,
) -> Result<(), BridgeError> {
    let db_path = db_path.clone();
    let table = table.to_string();
    let key_col = key_col.to_string();
    let key = key.to_string();
    task::spawn_blocking(move || {
        let conn = Connection::open(db_path)?;
        conn.execute(
            &format!(
                "INSERT INTO {table} ({key_col}, data) VALUES (?1, ?2) \
                 ON CONFLICT({key_col}) DO UPDATE SET data = excluded.data"
            ),
            params![key, data],
        )?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    .map_err(|err| BridgeError::Storage(err.to_string()))?
    .map_err(|err| BridgeError::Storage(err.to_string()))
}

async fn load_entry(
    db_path: &PathBuf,
    table: &str,
    key_col: &str,
    key: &str,
) -> Result<Option<Vec<u8>>, BridgeError> {
    let db_path = db_path.clone();
    let table = table.to_string();
    let key_col = key_col.to_string();
    let key = key.to_string();
    task::spawn_blocking(move || {
        let conn = Connection::open(db_path)?;
        let mut stmt = conn.prepare(&format!(
            "SELECT data FROM {table} WHERE {key_col} = ?1"
        ))?;
        let mut rows = stmt.query([key])?;
        if let Some(row) = rows.next()? {
            let data: Vec<u8> = row.get(0)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    })
    .await
    .map_err(|err| BridgeError::Storage(err.to_string()))?
    .map_err(|err| BridgeError::Storage(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_hash_accepts_matching_hex() {
        let data = b"hello";
        let expected = hex::encode(hash::hash_bytes(hash::HashId::Sha256, data));
        assert!(validate_hash(&expected, data).is_ok());
    }
}
