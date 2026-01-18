use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use spex_core::{
    hash::{hash_bytes, HashId},
    log::{CheckpointEntry, CheckpointLog, KeyCheckpoint, LogConsistency, RecoveryKey, RevocationDeclaration},
    sign::{ed25519_sign_hash, ed25519_verify_hash},
    types::{ContactCard, Ctap2Cbor, GrantToken},
};
use spex_transport::inbox::BridgeClient;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[derive(Debug, Error)]
enum CliError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cbor error: {0}")]
    Cbor(#[from] serde_cbor::Error),
    #[error("hex error: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("missing identity; run `spex identity new` first")]
    MissingIdentity,
    #[error("invalid card payload")]
    InvalidCard,
    #[error("signature verification failed")]
    SignatureInvalid,
    #[error("request target does not match local identity")]
    RequestTargetMismatch,
    #[error("thread not found")]
    ThreadNotFound,
    #[error("transport error: {0}")]
    Transport(String),
    #[error("log error: {0}")]
    Log(String),
}

#[derive(Parser)]
#[command(name = "spex", version, about = "SPEX CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Identity {
        #[command(subcommand)]
        command: IdentityCommand,
    },
    Card {
        #[command(subcommand)]
        command: CardCommand,
    },
    Request {
        #[command(subcommand)]
        command: RequestCommand,
    },
    Grant {
        #[command(subcommand)]
        command: GrantCommand,
    },
    Thread {
        #[command(subcommand)]
        command: ThreadCommand,
    },
    Msg {
        #[command(subcommand)]
        command: MsgCommand,
    },
    Inbox {
        #[command(subcommand)]
        command: InboxCommand,
    },
    Log {
        #[command(subcommand)]
        command: LogCommand,
    },
}

#[derive(Subcommand)]
enum IdentityCommand {
    New,
}

#[derive(Subcommand)]
enum CardCommand {
    Create,
    Redeem {
        #[arg(long)]
        card: String,
    },
}

#[derive(Subcommand)]
enum RequestCommand {
    Send {
        #[arg(long)]
        to: String,
        #[arg(long, default_value_t = 1)]
        role: u64,
    },
}

#[derive(Subcommand)]
enum GrantCommand {
    Accept {
        #[arg(long)]
        request: String,
    },
    Deny {
        #[arg(long)]
        request: String,
    },
}

#[derive(Subcommand)]
enum ThreadCommand {
    New {
        #[arg(long, value_delimiter = ',')]
        members: Vec<String>,
    },
}

#[derive(Subcommand)]
enum MsgCommand {
    Send {
        #[arg(long)]
        thread: String,
        #[arg(long)]
        text: String,
    },
}

#[derive(Subcommand)]
enum InboxCommand {
    Poll {
        #[arg(long)]
        bridge_url: Option<String>,
        #[arg(long)]
        inbox_key: Option<String>,
    },
}

#[derive(Subcommand)]
enum LogCommand {
    AppendCheckpoint,
    CreateRecoveryKey,
    RevokeKey {
        #[arg(long)]
        key_hex: String,
        #[arg(long)]
        recovery_hex: Option<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    Export {
        #[arg(long)]
        path: String,
    },
    Import {
        #[arg(long)]
        path: String,
    },
    GossipVerify {
        #[arg(long)]
        path: String,
    },
    Info,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct LocalState {
    identity: Option<IdentityState>,
    contacts: HashMap<String, ContactState>,
    threads: HashMap<String, ThreadState>,
    inbox: Vec<InboxItem>,
    requests: Vec<RequestState>,
    grants: Vec<GrantState>,
    checkpoint_log: CheckpointLogState,
}

#[derive(Debug, Serialize, Deserialize)]
struct IdentityState {
    user_id_hex: String,
    signing_key_hex: String,
    verifying_key_hex: String,
    device_id_hex: String,
    device_nonce_hex: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContactState {
    user_id_hex: String,
    verifying_key_hex: String,
    fingerprint: String,
    device_id_hex: String,
    last_seen_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThreadState {
    thread_id_hex: String,
    members: Vec<String>,
    created_at: u64,
    messages: Vec<MessageState>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MessageState {
    sender_user_id: String,
    text: String,
    sent_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct InboxItem {
    received_at: u64,
    payload_base64: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestState {
    token_base64: String,
    to_user_id: String,
    role: u64,
    created_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct GrantState {
    token_base64: String,
    from_user_id: String,
    role: u64,
    created_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct CheckpointLogState {
    log_base64: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestToken {
    from_user_id: String,
    to_user_id: String,
    role: u64,
    created_at: u64,
}

/// Runs the CLI entry point and dispatches commands.
#[tokio::main]
async fn main() -> Result<(), CliError> {
    let cli = Cli::parse();
    let mut state = load_state()?;

    match cli.command {
        Commands::Identity { command } => match command {
            IdentityCommand::New => {
                let identity = create_identity();
                let fingerprint = fingerprint_hex(&hex::decode(&identity.verifying_key_hex)?);
                println!("user_id: {}", identity.user_id_hex);
                println!("fingerprint: {}", fingerprint);
                state.identity = Some(identity);
            }
        },
        Commands::Card { command } => match command {
            CardCommand::Create => {
                let identity = state.identity.as_ref().ok_or(CliError::MissingIdentity)?;
                let card = build_contact_card(identity)?;
                let card_bytes = card.to_ctap2_canonical_bytes()?;
                let payload = BASE64_STANDARD.encode(card_bytes);
                println!("card: {}", payload);
            }
            CardCommand::Redeem { card } => {
                let card_bytes = BASE64_STANDARD
                    .decode(card.as_bytes())
                    .map_err(|_| CliError::InvalidCard)?;
                let card = parse_contact_card(&card_bytes)?;
                if let Some(signature) = &card.signature {
                    verify_contact_card_signature(&card, signature)?;
                }
                let user_id_hex = hex::encode(&card.user_id);
                let verifying_hex = hex::encode(&card.verifying_key);
                let fingerprint = fingerprint_hex(&card.verifying_key);
                if let Some(existing) = state.contacts.get(&user_id_hex) {
                    if existing.verifying_key_hex != verifying_hex {
                        println!(
                            "ALERT: key change detected for {} (old {}, new {})",
                            user_id_hex, existing.fingerprint, fingerprint
                        );
                    }
                }
                state.contacts.insert(
                    user_id_hex.clone(),
                    ContactState {
                        user_id_hex: user_id_hex.clone(),
                        verifying_key_hex: verifying_hex,
                        fingerprint: fingerprint.clone(),
                        device_id_hex: hex::encode(&card.device_id),
                        last_seen_at: now_unix(),
                    },
                );
                println!("contact saved: {}", user_id_hex);
                println!("fingerprint: {}", fingerprint);
            }
        },
        Commands::Request { command } => match command {
            RequestCommand::Send { to, role } => {
                let identity = state.identity.as_ref().ok_or(CliError::MissingIdentity)?;
                let request = RequestToken {
                    from_user_id: identity.user_id_hex.clone(),
                    to_user_id: to.clone(),
                    role,
                    created_at: now_unix(),
                };
                let payload = serde_json::to_vec(&request)?;
                let token = BASE64_STANDARD.encode(payload);
                state.requests.push(RequestState {
                    token_base64: token.clone(),
                    to_user_id: to,
                    role,
                    created_at: request.created_at,
                });
                println!("request: {}", token);
            }
        },
        Commands::Grant { command } => match command {
            GrantCommand::Accept { request } => {
                let identity = state.identity.as_ref().ok_or(CliError::MissingIdentity)?;
                let request_token = parse_request_token(&request)?;
                if request_token.to_user_id != identity.user_id_hex {
                    return Err(CliError::RequestTargetMismatch);
                }
                let grant = GrantToken {
                    user_id: hex::decode(&request_token.from_user_id)?,
                    role: request_token.role,
                    flags: None,
                    expires_at: None,
                    extensions: Default::default(),
                };
                let grant_bytes = grant.to_ctap2_canonical_bytes()?;
                let grant_token = BASE64_STANDARD.encode(grant_bytes);
                state.grants.push(GrantState {
                    token_base64: grant_token.clone(),
                    from_user_id: request_token.from_user_id,
                    role: request_token.role,
                    created_at: now_unix(),
                });
                println!("grant: {}", grant_token);
            }
            GrantCommand::Deny { request } => {
                let request_token = parse_request_token(&request)?;
                println!(
                    "request denied: {} (role {})",
                    request_token.from_user_id, request_token.role
                );
            }
        },
        Commands::Thread { command } => match command {
            ThreadCommand::New { mut members } => {
                let identity = state.identity.as_ref().ok_or(CliError::MissingIdentity)?;
                if !members.contains(&identity.user_id_hex) {
                    members.push(identity.user_id_hex.clone());
                }
                let thread_id = random_hex(32);
                let thread = ThreadState {
                    thread_id_hex: thread_id.clone(),
                    members,
                    created_at: now_unix(),
                    messages: Vec::new(),
                };
                state.threads.insert(thread_id.clone(), thread);
                println!("thread: {}", thread_id);
            }
        },
        Commands::Msg { command } => match command {
            MsgCommand::Send { thread, text } => {
                let identity = state.identity.as_ref().ok_or(CliError::MissingIdentity)?;
                let thread_state = state
                    .threads
                    .get_mut(&thread)
                    .ok_or(CliError::ThreadNotFound)?;
                let message = MessageState {
                    sender_user_id: identity.user_id_hex.clone(),
                    text: text.clone(),
                    sent_at: now_unix(),
                };
                thread_state.messages.push(message);
                println!("message queued for thread {}", thread);
            }
        },
        Commands::Inbox { command } => match command {
            InboxCommand::Poll {
                bridge_url,
                inbox_key,
            } => {
                if let (Some(bridge_url), Some(inbox_key)) = (bridge_url, inbox_key) {
                    let inbox_key_bytes = hex::decode(inbox_key)?;
                    let client = BridgeClient::new(bridge_url);
                    let response = client
                        .scan_inbox(&inbox_key_bytes)
                        .await
                        .map_err(|err| CliError::Transport(err.to_string()))?;
                    for item in response.items {
                        state.inbox.push(InboxItem {
                            received_at: now_unix(),
                            payload_base64: BASE64_STANDARD.encode(item),
                        });
                    }
                    println!(
                        "inbox: {} items (source: {:?})",
                        response.items.len(),
                        response.source
                    );
                } else {
                    println!("local inbox: {} items", state.inbox.len());
                    for item in &state.inbox {
                        println!("- {}", item.payload_base64);
                    }
                    state.inbox.clear();
                }
            }
        },
        Commands::Log { command } => match command {
            LogCommand::AppendCheckpoint => {
                let identity = state.identity.as_ref().ok_or(CliError::MissingIdentity)?;
                let entry = KeyCheckpoint {
                    user_id: hex::decode(&identity.user_id_hex)?,
                    verifying_key: hex::decode(&identity.verifying_key_hex)?,
                    device_id: hex::decode(&identity.device_id_hex)?,
                    issued_at: now_unix(),
                };
                let mut log = load_checkpoint_log(&state)?;
                log.append(CheckpointEntry::Key(entry))
                    .map_err(|err| CliError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                println!("checkpoint appended (len: {})", log.entries.len());
            }
            LogCommand::CreateRecoveryKey => {
                let identity = state.identity.as_ref().ok_or(CliError::MissingIdentity)?;
                let entry = RecoveryKey {
                    user_id: hex::decode(&identity.user_id_hex)?,
                    recovery_key: random_bytes(32),
                    issued_at: now_unix(),
                };
                let mut log = load_checkpoint_log(&state)?;
                log.append(CheckpointEntry::Recovery(entry.clone()))
                    .map_err(|err| CliError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                println!("recovery key: {}", hex::encode(entry.recovery_key));
                println!("checkpoint appended (len: {})", log.entries.len());
            }
            LogCommand::RevokeKey {
                key_hex,
                recovery_hex,
                reason,
            } => {
                let identity = state.identity.as_ref().ok_or(CliError::MissingIdentity)?;
                let revoked_key = hex::decode(&key_hex)?;
                let revoked_key_hash = hash_bytes(HashId::Sha256, &revoked_key);
                let recovery_key_hash = match recovery_hex {
                    Some(value) => Some(hash_bytes(HashId::Sha256, &hex::decode(value)?)),
                    None => None,
                };
                let entry = RevocationDeclaration {
                    user_id: hex::decode(&identity.user_id_hex)?,
                    revoked_key_hash,
                    revoked_at: now_unix(),
                    recovery_key_hash,
                    reason,
                };
                let mut log = load_checkpoint_log(&state)?;
                log.append(CheckpointEntry::Revocation(entry))
                    .map_err(|err| CliError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                println!("revocation appended (len: {})", log.entries.len());
            }
            LogCommand::Export { path } => {
                let log = load_checkpoint_log(&state)?;
                let encoded = log
                    .export_base64()
                    .map_err(|err| CliError::Log(err.to_string()))?;
                fs::write(path, encoded)?;
                println!("log exported");
            }
            LogCommand::Import { path } => {
                let encoded = fs::read_to_string(path)?;
                let log = CheckpointLog::import_base64(encoded.trim())
                    .map_err(|err| CliError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                println!("log imported (len: {})", log.entries.len());
            }
            LogCommand::GossipVerify { path } => {
                let encoded = fs::read_to_string(path)?;
                let remote = CheckpointLog::import_base64(encoded.trim())
                    .map_err(|err| CliError::Log(err.to_string()))?;
                let local = load_checkpoint_log(&state)?;
                let consistency = local.compare_with(&remote);
                let local_root = local
                    .merkle_root()
                    .map_err(|err| CliError::Log(err.to_string()))?;
                let remote_root = remote
                    .merkle_root()
                    .map_err(|err| CliError::Log(err.to_string()))?;
                println!("local entries: {}", local.entries.len());
                println!("remote entries: {}", remote.entries.len());
                println!("local root: {}", hex::encode(local_root));
                println!("remote root: {}", hex::encode(remote_root));
                println!(
                    "consistency: {}",
                    match consistency {
                        LogConsistency::Identical => "identical",
                        LogConsistency::LocalBehind => "local-behind",
                        LogConsistency::LocalAhead => "local-ahead",
                        LogConsistency::Diverged => "diverged",
                    }
                );
            }
            LogCommand::Info => {
                let log = load_checkpoint_log(&state)?;
                let root = log
                    .merkle_root()
                    .map_err(|err| CliError::Log(err.to_string()))?;
                println!("entries: {}", log.entries.len());
                println!("root: {}", hex::encode(root));
            }
        },
    }

    save_state(&state)?;
    Ok(())
}

/// Creates a new local identity with fresh keys and device metadata.
fn create_identity() -> IdentityState {
    let mut seed = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed);
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    let user_id = random_bytes(32);
    let device_id = random_bytes(16);
    let device_nonce = random_bytes(16);
    IdentityState {
        user_id_hex: hex::encode(user_id),
        signing_key_hex: hex::encode(seed),
        verifying_key_hex: hex::encode(verifying_key.to_bytes()),
        device_id_hex: hex::encode(device_id),
        device_nonce_hex: hex::encode(device_nonce),
    }
}

/// Builds a signed contact card payload for the current identity.
fn build_contact_card(identity: &IdentityState) -> Result<ContactCard, CliError> {
    let user_id = hex::decode(&identity.user_id_hex)?;
    let verifying_key = hex::decode(&identity.verifying_key_hex)?;
    let device_id = hex::decode(&identity.device_id_hex)?;
    let device_nonce = hex::decode(&identity.device_nonce_hex)?;

    let mut card = ContactCard {
        user_id,
        verifying_key,
        device_id,
        device_nonce,
        issued_at: now_unix(),
        invite: None,
        signature: None,
        extensions: Default::default(),
    };

    let signature = sign_contact_card(identity, &card)?;
    card.signature = Some(signature);
    Ok(card)
}

/// Signs a contact card using the local identity signing key.
fn sign_contact_card(identity: &IdentityState, card: &ContactCard) -> Result<Vec<u8>, CliError> {
    let signing_key = SigningKey::from_bytes(&hex::decode(&identity.signing_key_hex)?);
    let payload = card.to_ctap2_canonical_bytes()?;
    let digest = hash_bytes(HashId::Sha256, &payload);
    let signature = ed25519_sign_hash(&signing_key, &digest);
    Ok(signature.to_bytes().to_vec())
}

/// Verifies the signature embedded in a contact card.
fn verify_contact_card_signature(card: &ContactCard, signature: &[u8]) -> Result<(), CliError> {
    let verifying_key_bytes: [u8; 32] = card
        .verifying_key
        .as_slice()
        .try_into()
        .map_err(|_| CliError::SignatureInvalid)?;
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&verifying_key_bytes)
        .map_err(|_| CliError::SignatureInvalid)?;
    let mut unsigned_card = card.clone();
    unsigned_card.signature = None;
    let payload = unsigned_card.to_ctap2_canonical_bytes()?;
    let digest = hash_bytes(HashId::Sha256, &payload);
    let signature_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| CliError::SignatureInvalid)?;
    let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);
    ed25519_verify_hash(&verifying_key, &digest, &signature).map_err(|_| CliError::SignatureInvalid)
}

/// Parses a CBOR contact card into a typed structure.
fn parse_contact_card(bytes: &[u8]) -> Result<ContactCard, CliError> {
    let value: serde_cbor::Value = serde_cbor::from_slice(bytes)?;
    let map = match value {
        serde_cbor::Value::Map(map) => map,
        _ => return Err(CliError::InvalidCard),
    };

    let mut card = ContactCard::default();

    for (key, value) in map {
        let key = match key {
            serde_cbor::Value::Integer(v) => v,
            _ => continue,
        };
        match key {
            0 => card.user_id = expect_bytes(value)?,
            1 => card.verifying_key = expect_bytes(value)?,
            2 => card.device_id = expect_bytes(value)?,
            3 => card.device_nonce = expect_bytes(value)?,
            4 => card.issued_at = expect_u64(value)?,
            6 => card.signature = Some(expect_bytes(value)?),
            _ => {}
        }
    }

    if card.user_id.is_empty() || card.verifying_key.is_empty() {
        return Err(CliError::InvalidCard);
    }

    Ok(card)
}

/// Extracts bytes from a CBOR value or returns an invalid card error.
fn expect_bytes(value: serde_cbor::Value) -> Result<Vec<u8>, CliError> {
    match value {
        serde_cbor::Value::Bytes(bytes) => Ok(bytes),
        _ => Err(CliError::InvalidCard),
    }
}

/// Extracts a u64 from a CBOR value or returns an invalid card error.
fn expect_u64(value: serde_cbor::Value) -> Result<u64, CliError> {
    match value {
        serde_cbor::Value::Integer(v) => u64::try_from(v).map_err(|_| CliError::InvalidCard),
        _ => Err(CliError::InvalidCard),
    }
}

/// Parses a base64 request token into its structured representation.
fn parse_request_token(token: &str) -> Result<RequestToken, CliError> {
    let payload = BASE64_STANDARD
        .decode(token.as_bytes())
        .map_err(|_| CliError::InvalidCard)?;
    Ok(serde_json::from_slice(&payload)?)
}

/// Computes a SHA-256 fingerprint for the provided bytes.
fn fingerprint_hex(bytes: &[u8]) -> String {
    let digest = hash_bytes(HashId::Sha256, bytes);
    hex::encode(digest)
}

/// Generates a random byte string encoded as hex with the requested length.
fn random_hex(len: usize) -> String {
    hex::encode(random_bytes(len))
}

/// Generates random bytes for identifiers and secrets.
fn random_bytes(len: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut buffer);
    buffer
}

/// Returns the current UNIX timestamp in seconds.
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Loads persisted local state from disk.
fn load_state() -> Result<LocalState, CliError> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(LocalState::default());
    }
    let contents = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&contents)?)
}

/// Persists local state to disk in JSON format.
fn save_state(state: &LocalState) -> Result<(), CliError> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(state)?;
    fs::write(path, contents)?;
    Ok(())
}

/// Restores the checkpoint log from the persisted local state.
fn load_checkpoint_log(state: &LocalState) -> Result<CheckpointLog, CliError> {
    if state.checkpoint_log.log_base64.is_empty() {
        return Ok(CheckpointLog::new());
    }
    CheckpointLog::import_base64(&state.checkpoint_log.log_base64)
        .map_err(|err| CliError::Log(err.to_string()))
}

/// Saves a checkpoint log into the local state as base64 CBOR.
fn save_checkpoint_log(state: &mut LocalState, log: &CheckpointLog) -> Result<(), CliError> {
    let encoded = log
        .export_base64()
        .map_err(|err| CliError::Log(err.to_string()))?;
    state.checkpoint_log.log_base64 = encoded;
    Ok(())
}

/// Determines the path for the persisted local state file.
fn state_path() -> Result<PathBuf, CliError> {
    if let Ok(path) = std::env::var("SPEX_STATE_PATH") {
        return Ok(PathBuf::from(path));
    }
    let home = dirs::home_dir().ok_or_else(|| std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "home directory not found",
    ))?;
    Ok(home.join(Path::new(".spex/state.json")))
}
