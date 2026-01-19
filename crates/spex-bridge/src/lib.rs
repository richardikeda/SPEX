use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use spex_core::{hash, pow, pow::PowParams, sign, types::GrantToken};
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

/// Provides the current time source for grant validation.
pub trait Clock {
    /// Returns the current UNIX timestamp in seconds.
    fn now(&self) -> u64;
}

#[derive(Clone)]
struct SystemClock;

impl Clock for SystemClock {
    /// Returns the system UNIX timestamp in seconds.
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
    verifying_key: String,
    signature: String,
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
struct InboxResponse {
    items: Vec<String>,
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
    #[error("grant signature invalid")]
    GrantInvalid,
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
    /// Converts the bridge error into an HTTP response.
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            BridgeError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            BridgeError::PuzzleInvalid => StatusCode::UNAUTHORIZED,
            BridgeError::GrantInvalid => StatusCode::UNAUTHORIZED,
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

/// Builds the Axum router for the bridge API.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/cards/:card_hash", put(put_card).get(get_card))
        .route("/slot/:slot_id", put(put_slot).get(get_slot))
        .route("/inbox/:key", get(get_inbox))
        .with_state(state)
}

/// Initializes application state and ensures the database schema is ready.
pub fn init_state(db_path: impl Into<PathBuf>) -> Result<AppState, BridgeError> {
    let db_path = db_path.into();
    init_db(&db_path).map_err(|err| BridgeError::Storage(err.to_string()))?;
    Ok(AppState {
        db_path,
        clock: Arc::new(SystemClock),
    })
}

/// Initializes application state with a custom clock implementation.
pub fn init_state_with_clock(
    db_path: impl Into<PathBuf>,
    clock: Arc<dyn Clock + Send + Sync>,
) -> Result<AppState, BridgeError> {
    let db_path = db_path.into();
    init_db(&db_path).map_err(|err| BridgeError::Storage(err.to_string()))?;
    Ok(AppState { db_path, clock })
}

/// Creates the SQLite schema for cards and slots storage.
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
    conn.execute(
        "CREATE TABLE IF NOT EXISTS inbox_keys (inbox_key TEXT PRIMARY KEY)",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS inbox_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            inbox_key TEXT NOT NULL,
            item BLOB NOT NULL
        )",
        [],
    )?;
    Ok(())
}

/// Stores a card payload after validating its grant, puzzle, and hash.
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

/// Fetches a stored card payload by hash.
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

/// Stores a slot payload after validating its grant and puzzle.
async fn put_slot(
    State(state): State<AppState>,
    Path(slot_id): Path<String>,
    Json(payload): Json<StorageRequest>,
) -> Result<StatusCode, BridgeError> {
    validate_storage_request(&state, &payload)?;
    let data = decode_base64(&payload.data)?;
    validate_hash(&slot_id, &data)?;

    store_entry(&state.db_path, "slots", "slot_id", &slot_id, data).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fetches a stored slot payload by slot identifier.
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

/// Returns inbox items for the provided inbox scan key.
async fn get_inbox(
    State(state): State<AppState>,
    Path(inbox_key): Path<String>,
) -> Result<Json<InboxResponse>, BridgeError> {
    let items = load_inbox_items(&state.db_path, &inbox_key)
        .await?
        .ok_or(BridgeError::NotFound)?;
    Ok(Json(InboxResponse {
        items: items.into_iter().map(|item| BASE64.encode(item)).collect(),
    }))
}

/// Validates grant and puzzle information before persistence.
fn validate_storage_request(state: &AppState, payload: &StorageRequest) -> Result<(), BridgeError> {
    validate_grant(state.clock.now(), &payload.grant)?;
    validate_puzzle(&payload.puzzle)?;
    Ok(())
}

/// Ensures the grant token is well-formed and not expired.
fn validate_grant(now: u64, grant: &GrantTokenPayload) -> Result<(), BridgeError> {
    let user_id = decode_base64(&grant.user_id)?;
    let verify_key = decode_verifying_key(&grant.verifying_key)?;
    let signature = decode_signature(&grant.signature)?;
    if let Some(expires_at) = grant.expires_at {
        if expires_at <= now {
            return Err(BridgeError::GrantExpired);
        }
    }
    let token = GrantToken {
        user_id,
        role: grant.role,
        flags: grant.flags,
        expires_at: grant.expires_at,
        extensions: Default::default(),
    };
    let hash = hash::hash_ctap2_cbor_value(hash::HashId::Sha256, &token)
        .map_err(|err| BridgeError::InvalidRequest(err.to_string()))?;
    sign::ed25519_verify_hash(&verify_key, &hash, &signature)
        .map_err(|_| BridgeError::GrantInvalid)?;
    Ok(())
}

/// Validates the provided puzzle output with spex-core PoW verification.
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
    if let Some(custom) = payload.params.as_ref() {
        validate_pow_params(custom)?;
    }

    let valid = pow::verify_puzzle_output(&recipient_key, &puzzle_input, &puzzle_output, params)
        .map_err(|err| BridgeError::InvalidRequest(err.to_string()))?;
    if !valid {
        return Err(BridgeError::PuzzleInvalid);
    }
    Ok(())
}

/// Verifies the provided hash matches the SHA-256 hash of data.
fn validate_hash(card_hash: &str, data: &[u8]) -> Result<(), BridgeError> {
    let expected = hash::hash_bytes(hash::HashId::Sha256, data);
    let expected_hex = hex::encode(expected);
    if expected_hex != card_hash {
        return Err(BridgeError::HashMismatch);
    }
    Ok(())
}

/// Decodes a base64 string into raw bytes.
fn decode_base64(value: &str) -> Result<Vec<u8>, BridgeError> {
    BASE64
        .decode(value)
        .map_err(|err| BridgeError::InvalidRequest(err.to_string()))
}

/// Decodes a base64 string into a fixed-size byte array.
fn decode_fixed_bytes<const N: usize>(value: &str) -> Result<[u8; N], BridgeError> {
    let bytes = decode_base64(value)?;
    bytes
        .try_into()
        .map_err(|_| BridgeError::InvalidRequest("invalid length".to_string()))
}

/// Decodes a base64 signature into an Ed25519 signature struct.
fn decode_signature(value: &str) -> Result<Signature, BridgeError> {
    let bytes: [u8; 64] = decode_fixed_bytes(value)?;
    Ok(Signature::from_bytes(&bytes))
}

/// Decodes a base64 verifying key into an Ed25519 verifying key struct.
fn decode_verifying_key(value: &str) -> Result<VerifyingKey, BridgeError> {
    let bytes: [u8; 32] = decode_fixed_bytes(value)?;
    VerifyingKey::from_bytes(&bytes)
        .map_err(|err| BridgeError::InvalidRequest(err.to_string()))
}

/// Validates that the provided PoW parameters meet minimum requirements.
fn validate_pow_params(params: &PowParamsPayload) -> Result<(), BridgeError> {
    let minimum = PowParams::default();
    if params.memory_kib < minimum.memory_kib || params.iterations < minimum.iterations {
        return Err(BridgeError::PuzzleInvalid);
    }
    Ok(())
}

/// Stores binary data in the requested table under the provided key.
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

/// Loads binary data for the requested key from the database.
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

/// Loads inbox items for the given key, returning None when the inbox is missing.
async fn load_inbox_items(
    db_path: &PathBuf,
    inbox_key: &str,
) -> Result<Option<Vec<Vec<u8>>>, BridgeError> {
    let db_path = db_path.clone();
    let inbox_key = inbox_key.to_string();
    task::spawn_blocking(move || {
        let conn = Connection::open(db_path)?;
        let exists: Option<String> = conn
            .query_row(
                "SELECT inbox_key FROM inbox_keys WHERE inbox_key = ?1",
                [inbox_key.clone()],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Ok(None);
        }
        let mut stmt = conn.prepare(
            "SELECT item FROM inbox_items WHERE inbox_key = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([inbox_key], |row| row.get(0))?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(Some(items))
    })
    .await
    .map_err(|err| BridgeError::Storage(err.to_string()))?
    .map_err(|err| BridgeError::Storage(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensures hash validation accepts a matching SHA-256 hex string.
    #[test]
    fn validate_hash_accepts_matching_hex() {
        let data = b"hello";
        let expected = hex::encode(hash::hash_bytes(hash::HashId::Sha256, data));
        assert!(validate_hash(&expected, data).is_ok());
    }
}
