use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::put,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use spex_core::{
    hash,
    pow::PowParams,
    types::GrantToken,
    validation::{self, GrantPowValidationError},
};
use std::{
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::task;

#[derive(Clone)]
pub struct AppState {
    db_path: PathBuf,
    clock: Arc<dyn Clock + Send + Sync>,
    limits: RateLimitConfig,
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

/// Describes rate limiting and adaptive PoW requirements for requests.
#[derive(Clone, Copy)]
pub struct RateLimitConfig {
    window_seconds: u64,
    max_requests: u64,
    max_bytes: u64,
    difficulty_step_requests: u64,
    difficulty_step_bytes: u64,
    reputation_step: i32,
}

impl Default for RateLimitConfig {
    /// Returns the default rate-limiting configuration for the bridge.
    fn default() -> Self {
        Self {
            window_seconds: 60,
            max_requests: 120,
            max_bytes: 1_048_576,
            difficulty_step_requests: 20,
            difficulty_step_bytes: 262_144,
            reputation_step: 25,
        }
    }
}

/// Captures recent request volume and local reputation for an identity.
struct RateLimitSnapshot {
    recent_requests: u64,
    recent_bytes: u64,
    reputation_score: i32,
}

/// Describes the kind of storage request being processed.
#[derive(Clone, Copy)]
enum RequestKind {
    Card,
    Slot,
    Inbox,
}

impl RequestKind {
    /// Returns the string label used to persist the request kind.
    fn as_str(&self) -> &'static str {
        match self {
            RequestKind::Card => "card",
            RequestKind::Slot => "slot",
            RequestKind::Inbox => "inbox",
        }
    }
}

/// Describes the outcome of a stored request for auditing.
#[derive(Clone, Copy)]
enum RequestOutcome {
    Accepted,
    Rejected,
}

impl RequestOutcome {
    /// Returns the string label used to persist the request outcome.
    fn as_str(&self) -> &'static str {
        match self {
            RequestOutcome::Accepted => "accepted",
            RequestOutcome::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Deserialize)]
struct StorageRequest {
    data: String,
    grant: GrantTokenPayload,
    puzzle: PuzzlePayload,
}

#[derive(Debug, Deserialize)]
struct InboxStoreRequest {
    data: String,
    grant: GrantTokenPayload,
    puzzle: PuzzlePayload,
    ttl_seconds: Option<u64>,
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
    next_cursor: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct InboxQuery {
    limit: Option<usize>,
    cursor: Option<i64>,
    max_bytes: Option<usize>,
}

#[derive(Debug)]
struct InboxPage {
    items: Vec<Vec<u8>>,
    next_cursor: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
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
    #[error("rate limit exceeded")]
    RateLimited,
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
            BridgeError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
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
        .route("/inbox/:key", put(put_inbox).get(get_inbox))
        .with_state(state)
}

/// Initializes application state and ensures the database schema is ready.
pub fn init_state(db_path: impl Into<PathBuf>) -> Result<AppState, BridgeError> {
    let db_path = db_path.into();
    init_db(&db_path).map_err(|err| BridgeError::Storage(err.to_string()))?;
    Ok(AppState {
        db_path,
        clock: Arc::new(SystemClock),
        limits: RateLimitConfig::default(),
    })
}

/// Initializes application state with a custom clock implementation.
pub fn init_state_with_clock(
    db_path: impl Into<PathBuf>,
    clock: Arc<dyn Clock + Send + Sync>,
) -> Result<AppState, BridgeError> {
    let db_path = db_path.into();
    init_db(&db_path).map_err(|err| BridgeError::Storage(err.to_string()))?;
    Ok(AppState {
        db_path,
        clock,
        limits: RateLimitConfig::default(),
    })
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
            item BLOB NOT NULL,
            expires_at INTEGER
        )",
        [],
    )?;
    ensure_inbox_items_expiration_column(&conn)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS request_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            identity TEXT NOT NULL,
            ip TEXT NOT NULL,
            slot_id TEXT,
            bytes INTEGER NOT NULL,
            request_kind TEXT NOT NULL,
            outcome TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS identity_reputation (
            identity TEXT PRIMARY KEY,
            score INTEGER NOT NULL
        )",
        [],
    )?;
    Ok(())
}

/// Ensures the inbox_items table has the expires_at column for expiry filtering.
fn ensure_inbox_items_expiration_column(conn: &Connection) -> rusqlite::Result<()> {
    if inbox_items_has_expires_column(conn)? {
        return Ok(());
    }
    conn.execute("ALTER TABLE inbox_items ADD COLUMN expires_at INTEGER", [])?;
    Ok(())
}

/// Checks whether the inbox_items table already contains the expires_at column.
fn inbox_items_has_expires_column(conn: &Connection) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare("PRAGMA table_info(inbox_items)")?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "expires_at" {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Stores a card payload after validating its grant, puzzle, and hash.
async fn put_card(
    State(state): State<AppState>,
    Path(card_hash): Path<String>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(payload): Json<StorageRequest>,
) -> Result<StatusCode, BridgeError> {
    let data = decode_base64(&payload.data)?;
    let data_len = data.len();
    let identity = identity_from_grant(&payload.grant)?;
    let now = state.clock.now();
    let ip = extract_ip(connect_info);
    let snapshot = load_rate_limit_snapshot(&state.db_path, &identity, now, state.limits).await?;
    let minimum_pow = required_pow_params(&snapshot, &state.limits);
    let result = (|| -> Result<(), BridgeError> {
        enforce_rate_limits(&snapshot, &state.limits, data_len)?;
        validate_grant_and_puzzle(&state, &payload.grant, &payload.puzzle, &minimum_pow, true)?;
        validate_hash(&card_hash, &data)?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            store_entry(&state.db_path, "cards", "card_hash", &card_hash, data).await?;
            record_request_log(
                &state.db_path,
                now,
                &identity,
                &ip,
                None,
                data_len,
                RequestKind::Card,
                RequestOutcome::Accepted,
            )
            .await?;
            update_reputation(&state.db_path, &identity, RequestOutcome::Accepted).await?;
            Ok(StatusCode::NO_CONTENT)
        }
        Err(err) => {
            record_request_log(
                &state.db_path,
                now,
                &identity,
                &ip,
                None,
                data_len,
                RequestKind::Card,
                RequestOutcome::Rejected,
            )
            .await?;
            update_reputation(&state.db_path, &identity, RequestOutcome::Rejected).await?;
            Err(err)
        }
    }
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
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(payload): Json<StorageRequest>,
) -> Result<StatusCode, BridgeError> {
    let data = decode_base64(&payload.data)?;
    let data_len = data.len();
    let now = state.clock.now();
    let ip = extract_ip(connect_info);
    let identity = identity_from_grant(&payload.grant).unwrap_or_else(|_| "unknown".to_string());
    if let Err(err) = validate_grant(now, &payload.grant) {
        tracing::warn!(
            slot_id = %slot_id,
            ip = %ip,
            error = %err,
            "invalid grant for slot request"
        );
        record_request_log(
            &state.db_path,
            now,
            &identity,
            &ip,
            Some(&slot_id),
            data_len,
            RequestKind::Slot,
            RequestOutcome::Rejected,
        )
        .await?;
        update_reputation(&state.db_path, &identity, RequestOutcome::Rejected).await?;
        return Err(err);
    }
    let snapshot = load_rate_limit_snapshot(&state.db_path, &identity, now, state.limits).await?;
    let minimum_pow = required_pow_params(&snapshot, &state.limits);
    let result = (|| -> Result<(), BridgeError> {
        enforce_rate_limits(&snapshot, &state.limits, data_len)?;
        validate_grant_and_puzzle(&state, &payload.grant, &payload.puzzle, &minimum_pow, false)?;
        validate_hash(&slot_id, &data)?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            store_entry(&state.db_path, "slots", "slot_id", &slot_id, data).await?;
            record_request_log(
                &state.db_path,
                now,
                &identity,
                &ip,
                Some(&slot_id),
                data_len,
                RequestKind::Slot,
                RequestOutcome::Accepted,
            )
            .await?;
            update_reputation(&state.db_path, &identity, RequestOutcome::Accepted).await?;
            Ok(StatusCode::NO_CONTENT)
        }
        Err(err) => {
            record_request_log(
                &state.db_path,
                now,
                &identity,
                &ip,
                Some(&slot_id),
                data_len,
                RequestKind::Slot,
                RequestOutcome::Rejected,
            )
            .await?;
            update_reputation(&state.db_path, &identity, RequestOutcome::Rejected).await?;
            Err(err)
        }
    }
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

/// Stores an inbox envelope for a given inbox key with a bounded time-to-live.
async fn put_inbox(
    State(state): State<AppState>,
    Path(inbox_key): Path<String>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(payload): Json<InboxStoreRequest>,
) -> Result<StatusCode, BridgeError> {
    let data = decode_base64(&payload.data)?;
    let data_len = data.len();
    let identity = identity_from_grant(&payload.grant)?;
    let now = state.clock.now();
    let ip = extract_ip(connect_info);
    let snapshot = load_rate_limit_snapshot(&state.db_path, &identity, now, state.limits).await?;
    let minimum_pow = required_pow_params(&snapshot, &state.limits);
    let result = (|| -> Result<u64, BridgeError> {
        validate_inbox_item_size(data_len)?;
        enforce_rate_limits(&snapshot, &state.limits, data_len)?;
        validate_grant_and_puzzle(&state, &payload.grant, &payload.puzzle, &minimum_pow, true)?;
        compute_inbox_expiration(now, payload.ttl_seconds)
    })();

    match result {
        Ok(expires_at) => {
            store_inbox_item(&state.db_path, &inbox_key, data, expires_at).await?;
            record_request_log(
                &state.db_path,
                now,
                &identity,
                &ip,
                None,
                data_len,
                RequestKind::Inbox,
                RequestOutcome::Accepted,
            )
            .await?;
            update_reputation(&state.db_path, &identity, RequestOutcome::Accepted).await?;
            Ok(StatusCode::NO_CONTENT)
        }
        Err(err) => {
            record_request_log(
                &state.db_path,
                now,
                &identity,
                &ip,
                None,
                data_len,
                RequestKind::Inbox,
                RequestOutcome::Rejected,
            )
            .await?;
            update_reputation(&state.db_path, &identity, RequestOutcome::Rejected).await?;
            Err(err)
        }
    }
}

/// Returns inbox items for the provided inbox scan key.
async fn get_inbox(
    State(state): State<AppState>,
    Path(inbox_key): Path<String>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<InboxResponse>, BridgeError> {
    let now = state.clock.now();
    let limit = normalize_inbox_limit(query.limit);
    let max_bytes = normalize_inbox_max_bytes(query.max_bytes, state.limits.max_bytes);
    let page = load_inbox_items(
        &state.db_path,
        &inbox_key,
        now,
        query.cursor,
        limit,
        max_bytes,
    )
    .await?
    .ok_or(BridgeError::NotFound)?;
    Ok(Json(InboxResponse {
        items: page
            .items
            .into_iter()
            .map(|item| BASE64.encode(item))
            .collect(),
        next_cursor: page.next_cursor,
    }))
}

/// Validates grant and puzzle data before persistence.
fn validate_grant_and_puzzle(
    state: &AppState,
    grant: &GrantTokenPayload,
    puzzle: &PuzzlePayload,
    minimum_pow: &PowParams,
    validate_grant_token: bool,
) -> Result<(), BridgeError> {
    if validate_grant_token {
        validate_grant(state.clock.now(), grant)?;
    }
    validate_puzzle(puzzle, minimum_pow)?;
    Ok(())
}

/// Ensures the grant token is well-formed and not expired.
fn validate_grant(now: u64, grant: &GrantTokenPayload) -> Result<(), BridgeError> {
    let user_id = decode_base64(&grant.user_id)?;
    let verify_key = decode_verifying_key(&grant.verifying_key)?;
    let signature = decode_signature(&grant.signature)?;
    let token = GrantToken {
        user_id,
        role: grant.role,
        flags: grant.flags,
        expires_at: grant.expires_at,
        extensions: Default::default(),
    };
    match validation::validate_grant_token(now, &token, &verify_key, &signature) {
        Ok(()) => Ok(()),
        Err(GrantPowValidationError::GrantExpired) => Err(BridgeError::GrantExpired),
        Err(GrantPowValidationError::GrantInvalid) => Err(BridgeError::GrantInvalid),
        Err(err) => Err(BridgeError::InvalidRequest(err.to_string())),
    }
}

/// Validates the provided puzzle output with spex-core PoW verification.
fn validate_puzzle(payload: &PuzzlePayload, minimum: &PowParams) -> Result<(), BridgeError> {
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
    match validation::validate_pow_puzzle(
        &recipient_key,
        &puzzle_input,
        &puzzle_output,
        params,
        *minimum,
    ) {
        Ok(()) => Ok(()),
        Err(GrantPowValidationError::PowTooWeak | GrantPowValidationError::PowInvalid) => {
            Err(BridgeError::PuzzleInvalid)
        }
        Err(err) => Err(BridgeError::InvalidRequest(err.to_string())),
    }
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
    VerifyingKey::from_bytes(&bytes).map_err(|err| BridgeError::InvalidRequest(err.to_string()))
}

/// Derives a stable identity string from the grant payload.
fn identity_from_grant(grant: &GrantTokenPayload) -> Result<String, BridgeError> {
    let user_id = decode_base64(&grant.user_id)?;
    Ok(hex::encode(user_id))
}

/// Extracts a client IP string from connection info when available.
fn extract_ip(connect_info: Option<ConnectInfo<SocketAddr>>) -> String {
    connect_info
        .map(|info| info.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Loads recent request volume and reputation for rate limiting.
async fn load_rate_limit_snapshot(
    db_path: &PathBuf,
    identity: &str,
    now: u64,
    limits: RateLimitConfig,
) -> Result<RateLimitSnapshot, BridgeError> {
    let db_path = db_path.clone();
    let identity = identity.to_string();
    task::spawn_blocking(move || {
        let conn = Connection::open(db_path)?;
        let window_start = now.saturating_sub(limits.window_seconds) as i64;
        let (count, bytes): (u64, u64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(bytes), 0) FROM request_logs \
             WHERE identity = ?1 AND timestamp >= ?2",
            params![identity, window_start],
            |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64)),
        )?;
        let reputation: i32 = conn
            .query_row(
                "SELECT score FROM identity_reputation WHERE identity = ?1",
                params![identity],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);
        Ok::<RateLimitSnapshot, rusqlite::Error>(RateLimitSnapshot {
            recent_requests: count,
            recent_bytes: bytes,
            reputation_score: reputation,
        })
    })
    .await
    .map_err(|err| BridgeError::Storage(err.to_string()))?
    .map_err(|err: rusqlite::Error| BridgeError::Storage(err.to_string()))
}

/// Enforces the per-identity message and byte limits for the active window.
fn enforce_rate_limits(
    snapshot: &RateLimitSnapshot,
    limits: &RateLimitConfig,
    incoming_bytes: usize,
) -> Result<(), BridgeError> {
    if snapshot.recent_requests + 1 > limits.max_requests {
        return Err(BridgeError::RateLimited);
    }
    if snapshot.recent_bytes + incoming_bytes as u64 > limits.max_bytes {
        return Err(BridgeError::RateLimited);
    }
    Ok(())
}

/// Computes the minimum PoW parameters required for the request volume and reputation.
fn required_pow_params(snapshot: &RateLimitSnapshot, limits: &RateLimitConfig) -> PowParams {
    let base = PowParams::default();
    let request_steps = snapshot.recent_requests / limits.difficulty_step_requests;
    let byte_steps = snapshot.recent_bytes / limits.difficulty_step_bytes;
    let volume_steps = request_steps.max(byte_steps);
    let reputation_bonus =
        (snapshot.reputation_score.max(0) as u64) / limits.reputation_step as u64;
    let steps = volume_steps.saturating_sub(reputation_bonus);
    PowParams {
        memory_kib: base
            .memory_kib
            .saturating_mul((steps as u32).saturating_add(1)),
        iterations: base.iterations.saturating_add(steps as u32),
        parallelism: base.parallelism,
        output_len: base.output_len,
    }
}

/// Persists an audit log entry for a request attempt.
async fn record_request_log(
    db_path: &PathBuf,
    timestamp: u64,
    identity: &str,
    ip: &str,
    slot_id: Option<&str>,
    bytes: usize,
    kind: RequestKind,
    outcome: RequestOutcome,
) -> Result<(), BridgeError> {
    let db_path = db_path.clone();
    let identity = identity.to_string();
    let ip = ip.to_string();
    let slot_id = slot_id.map(|value| value.to_string());
    let kind = kind.as_str().to_string();
    let outcome = outcome.as_str().to_string();
    task::spawn_blocking(move || {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "INSERT INTO request_logs (timestamp, identity, ip, slot_id, bytes, request_kind, outcome) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![timestamp as i64, identity, ip, slot_id, bytes as i64, kind, outcome],
        )?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    .map_err(|err| BridgeError::Storage(err.to_string()))?
    .map_err(|err: rusqlite::Error| BridgeError::Storage(err.to_string()))
}

/// Updates the local reputation score based on request outcome.
async fn update_reputation(
    db_path: &PathBuf,
    identity: &str,
    outcome: RequestOutcome,
) -> Result<(), BridgeError> {
    let db_path = db_path.clone();
    let identity = identity.to_string();
    task::spawn_blocking(move || {
        let conn = Connection::open(db_path)?;
        let current: i32 = conn
            .query_row(
                "SELECT score FROM identity_reputation WHERE identity = ?1",
                params![identity.clone()],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);
        let next = match outcome {
            RequestOutcome::Accepted => (current + 1).min(100),
            RequestOutcome::Rejected => (current - 1).max(-20),
        };
        conn.execute(
            "INSERT INTO identity_reputation (identity, score) VALUES (?1, ?2) \
             ON CONFLICT(identity) DO UPDATE SET score = excluded.score",
            params![identity, next],
        )?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    .map_err(|err| BridgeError::Storage(err.to_string()))?
    .map_err(|err: rusqlite::Error| BridgeError::Storage(err.to_string()))
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
    .map_err(|err: rusqlite::Error| BridgeError::Storage(err.to_string()))
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
        let mut stmt = conn.prepare(&format!("SELECT data FROM {table} WHERE {key_col} = ?1"))?;
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
    .map_err(|err: rusqlite::Error| BridgeError::Storage(err.to_string()))
}

const DEFAULT_INBOX_PAGE_LIMIT: usize = 100;
const MAX_INBOX_PAGE_LIMIT: usize = 500;
const DEFAULT_INBOX_TTL_SECONDS: u64 = 86_400;
const MAX_INBOX_TTL_SECONDS: u64 = 604_800;
const MAX_INBOX_ITEM_BYTES: usize = 262_144;

/// Normalizes the inbox page size to a safe default and maximum cap.
fn normalize_inbox_limit(limit: Option<usize>) -> usize {
    match limit {
        Some(value) if value > 0 => value.min(MAX_INBOX_PAGE_LIMIT),
        _ => DEFAULT_INBOX_PAGE_LIMIT,
    }
}

/// Normalizes the inbox max bytes value to a bounded response size.
fn normalize_inbox_max_bytes(max_bytes: Option<usize>, max_allowed: u64) -> usize {
    let max_allowed = max_allowed.min(usize::MAX as u64) as usize;
    match max_bytes {
        Some(value) if value > 0 => value.min(max_allowed),
        _ => max_allowed,
    }
}

/// Loads inbox items for the given key, returning None when the inbox is missing.
async fn load_inbox_items(
    db_path: &PathBuf,
    inbox_key: &str,
    now: u64,
    cursor: Option<i64>,
    limit: usize,
    max_bytes: usize,
) -> Result<Option<InboxPage>, BridgeError> {
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
        let cursor = cursor.unwrap_or(0);
        let fetch_limit = limit.saturating_add(1) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, item FROM inbox_items \
             WHERE inbox_key = ?1 AND (expires_at IS NULL OR expires_at > ?2) AND id > ?3 \
             ORDER BY id ASC LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![inbox_key, now, cursor, fetch_limit], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut items = Vec::new();
        let mut total_bytes = 0usize;
        let mut last_id = None;
        let mut has_more = false;
        for row in rows {
            let (id, item) = row?;
            if items.len() >= limit {
                has_more = true;
                break;
            }
            if total_bytes + item.len() > max_bytes {
                has_more = true;
                break;
            }
            total_bytes += item.len();
            last_id = Some(id);
            items.push(item);
        }
        let next_cursor = if has_more { last_id } else { None };
        Ok(Some(InboxPage { items, next_cursor }))
    })
    .await
    .map_err(|err| BridgeError::Storage(err.to_string()))?
    .map_err(|err: rusqlite::Error| BridgeError::Storage(err.to_string()))
}

/// Calculates the expiration timestamp for an inbox item from a requested TTL.
fn compute_inbox_expiration(now: u64, ttl_seconds: Option<u64>) -> Result<u64, BridgeError> {
    let ttl = ttl_seconds.unwrap_or(DEFAULT_INBOX_TTL_SECONDS);
    if ttl == 0 || ttl > MAX_INBOX_TTL_SECONDS {
        return Err(BridgeError::InvalidRequest("invalid inbox ttl".to_string()));
    }
    now.checked_add(ttl)
        .ok_or_else(|| BridgeError::InvalidRequest("invalid inbox ttl".to_string()))
}

/// Ensures inbox envelopes remain within the maximum allowed size.
fn validate_inbox_item_size(data_len: usize) -> Result<(), BridgeError> {
    if data_len > MAX_INBOX_ITEM_BYTES {
        return Err(BridgeError::InvalidRequest(
            "inbox item too large".to_string(),
        ));
    }
    Ok(())
}

/// Persists an inbox item and ensures the inbox key exists.
async fn store_inbox_item(
    db_path: &PathBuf,
    inbox_key: &str,
    item: Vec<u8>,
    expires_at: u64,
) -> Result<(), BridgeError> {
    let db_path = db_path.clone();
    let inbox_key = inbox_key.to_string();
    task::spawn_blocking(move || {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "INSERT OR IGNORE INTO inbox_keys (inbox_key) VALUES (?1)",
            params![inbox_key],
        )?;
        conn.execute(
            "INSERT INTO inbox_items (inbox_key, item, expires_at) VALUES (?1, ?2, ?3)",
            params![inbox_key, item, expires_at as i64],
        )?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    .map_err(|err| BridgeError::Storage(err.to_string()))?
    .map_err(|err: rusqlite::Error| BridgeError::Storage(err.to_string()))
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
