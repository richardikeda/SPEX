use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use spex_core::{
    aead_ad,
    hash::{hash_bytes, hash_ctap2_cbor_value, HashId},
    log::{CheckpointEntry, CheckpointLog, KeyCheckpoint, LogConsistency, RecoveryKey, RevocationDeclaration},
    pow,
    sign::{ed25519_sign_hash, ed25519_verify_hash, ed25519_verify_key},
    types::{to_fixed, ContactCard, Ctap2Cbor, GrantToken, ProtoSuite, ThreadConfig},
};
use spex_mls::{cfg_hash_for_thread_config, Group, GroupConfig};
use spex_transport::{
    chunking::{chunk_data, Chunk, ChunkingConfig},
    inbox::{derive_inbox_scan_key, BridgeClient, InboxScanResponse, InboxSource},
    transport::{ChunkDescriptor, ChunkManifest},
};
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
    #[error("mls error: {0}")]
    Mls(String),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("invalid request puzzle")]
    InvalidPuzzle,
    #[error("invalid aead payload")]
    InvalidAeadPayload,
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
    Rotate,
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
    #[serde(default)]
    transport_outbox: Vec<TransportOutboxItem>,
    #[serde(default)]
    p2p_inbox: HashMap<String, Vec<String>>,
    #[serde(default)]
    key_rotations: Vec<KeyRotationState>,
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
    #[serde(default)]
    proto_major: u16,
    #[serde(default)]
    proto_minor: u16,
    #[serde(default)]
    ciphersuite_id: u16,
    #[serde(default)]
    cfg_hash_id: u16,
    #[serde(default)]
    cfg_hash_hex: String,
    #[serde(default)]
    flags: u8,
    #[serde(default)]
    initial_secret_hex: String,
    #[serde(default)]
    next_seq: u64,
    #[serde(default)]
    epoch: u64,
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
    #[serde(default)]
    puzzle: Option<PuzzleToken>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GrantState {
    token_base64: String,
    from_user_id: String,
    role: u64,
    created_at: u64,
    #[serde(default)]
    signature_base64: Option<String>,
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
    puzzle: Option<PuzzleToken>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PuzzleToken {
    recipient_key: String,
    puzzle_input: String,
    puzzle_output: String,
    params: PowParamsPayload,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PowParamsPayload {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    output_len: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct SignedGrantToken {
    user_id: String,
    role: u64,
    flags: Option<u64>,
    expires_at: Option<u64>,
    verifying_key: String,
    signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TransportOutboxItem {
    thread_id_hex: String,
    manifest: TransportManifestState,
    chunks: Vec<TransportChunkState>,
    published_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct TransportManifestState {
    total_len: usize,
    chunks: Vec<TransportChunkDescriptorState>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TransportChunkDescriptorState {
    index: usize,
    hash_hex: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TransportChunkState {
    index: usize,
    hash_hex: String,
    data_base64: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct KeyRotationState {
    rotated_at: u64,
    old_verifying_key_hex: String,
    new_verifying_key_hex: String,
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
            IdentityCommand::Rotate => {
                let identity = state.identity.as_mut().ok_or(CliError::MissingIdentity)?;
                let old_verifying_key_hex = identity.verifying_key_hex.clone();
                let rotated = rotate_identity(identity);
                println!("new user_id: {}", rotated.user_id_hex);
                println!(
                    "new fingerprint: {}",
                    fingerprint_hex(&hex::decode(&rotated.verifying_key_hex)?)
                );
                let rotation = KeyRotationState {
                    rotated_at: now_unix(),
                    old_verifying_key_hex,
                    new_verifying_key_hex: rotated.verifying_key_hex.clone(),
                };
                state.key_rotations.push(rotation);
                append_rotation_checkpoint(&mut state, rotated, &old_verifying_key_hex)?;
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
                let puzzle = build_request_puzzle(&hex::decode(&to)?)?;
                validate_request_puzzle(&puzzle, &hex::decode(&to)?)?;
                let request = RequestToken {
                    from_user_id: identity.user_id_hex.clone(),
                    to_user_id: to.clone(),
                    role,
                    created_at: now_unix(),
                    puzzle: Some(puzzle.clone()),
                };
                let payload = serde_json::to_vec(&request)?;
                let token = BASE64_STANDARD.encode(payload);
                state.requests.push(RequestState {
                    token_base64: token.clone(),
                    to_user_id: to,
                    role,
                    created_at: request.created_at,
                    puzzle: Some(puzzle),
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
                if let Some(puzzle) = &request_token.puzzle {
                    validate_request_puzzle(puzzle, &hex::decode(&identity.user_id_hex)?)?;
                }
                let grant = GrantToken {
                    user_id: hex::decode(&request_token.from_user_id)?,
                    role: request_token.role,
                    flags: None,
                    expires_at: None,
                    extensions: Default::default(),
                };
                let signed_grant = sign_grant(identity, &grant)?;
                let grant_token = BASE64_STANDARD.encode(serde_json::to_vec(&signed_grant)?);
                state.grants.push(GrantState {
                    token_base64: grant_token.clone(),
                    from_user_id: request_token.from_user_id,
                    role: request_token.role,
                    created_at: now_unix(),
                    signature_base64: Some(signed_grant.signature.clone()),
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
                let proto_suite = ProtoSuite {
                    major: 1,
                    minor: 1,
                    ciphersuite_id: 0x0001,
                };
                let cfg_hash = build_thread_cfg_hash(&thread_id, &proto_suite)?;
                let initial_secret = random_hex(32);
                let group = build_group_for_members(&members, &cfg_hash, &initial_secret, proto_suite)?;
                let thread = ThreadState {
                    thread_id_hex: thread_id.clone(),
                    members,
                    created_at: now_unix(),
                    messages: Vec::new(),
                    proto_major: proto_suite.major,
                    proto_minor: proto_suite.minor,
                    ciphersuite_id: proto_suite.ciphersuite_id,
                    cfg_hash_id: HashId::Sha256 as u16,
                    cfg_hash_hex: hex::encode(cfg_hash),
                    flags: 0,
                    initial_secret_hex: initial_secret,
                    next_seq: 0,
                    epoch: group.epoch(),
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
                let (envelope, manifest, chunks) =
                    encrypt_and_chunk_message(identity, thread_state, text.as_bytes())?;
                let message = MessageState {
                    sender_user_id: identity.user_id_hex.clone(),
                    text: text.clone(),
                    sent_at: now_unix(),
                };
                thread_state.messages.push(message);
                let outbox_item = TransportOutboxItem {
                    thread_id_hex: thread_state.thread_id_hex.clone(),
                    manifest: manifest_state(&manifest),
                    chunks: chunk_states(&chunks),
                    published_at: now_unix(),
                };
                state.transport_outbox.push(outbox_item);
                stage_p2p_inbox_delivery(&mut state, thread_state, &identity.user_id_hex, &envelope)?;
                println!(
                    "message published via transport ({} chunks)",
                    chunks.len()
                );
            }
        },
        Commands::Inbox { command } => match command {
            InboxCommand::Poll {
                bridge_url,
                inbox_key,
            } => {
                if let Some(inbox_key) = inbox_key {
                    let inbox_key_bytes = hex::decode(inbox_key)?;
                    let bridge = bridge_url.map(BridgeClient::new);
                    let response = resolve_inbox_from_state(
                        &mut state,
                        &inbox_key_bytes,
                        bridge.as_ref(),
                    )
                    .await?;
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

/// Rotates the local identity signing and verification keys in place.
fn rotate_identity(identity: &mut IdentityState) -> &IdentityState {
    let mut seed = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed);
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    identity.signing_key_hex = hex::encode(seed);
    identity.verifying_key_hex = hex::encode(verifying_key.to_bytes());
    identity.device_nonce_hex = hex::encode(random_bytes(16));
    identity
}

/// Appends rotation metadata to the checkpoint log and revokes the previous verifying key.
fn append_rotation_checkpoint(
    state: &mut LocalState,
    identity: &IdentityState,
    old_verifying_key_hex: &str,
) -> Result<(), CliError> {
    let revoked_key = hex::decode(old_verifying_key_hex)?;
    let revoked_key_hash = hash_bytes(HashId::Sha256, &revoked_key);
    let revocation = RevocationDeclaration {
        user_id: hex::decode(&identity.user_id_hex)?,
        revoked_key_hash,
        revoked_at: now_unix(),
        recovery_key_hash: None,
        reason: Some("key rotation".to_string()),
    };
    let checkpoint = KeyCheckpoint {
        user_id: hex::decode(&identity.user_id_hex)?,
        verifying_key: hex::decode(&identity.verifying_key_hex)?,
        device_id: hex::decode(&identity.device_id_hex)?,
        issued_at: now_unix(),
    };
    let mut log = load_checkpoint_log(state)?;
    log.append(CheckpointEntry::Revocation(revocation))
        .map_err(|err| CliError::Log(err.to_string()))?;
    log.append(CheckpointEntry::Key(checkpoint))
        .map_err(|err| CliError::Log(err.to_string()))?;
    save_checkpoint_log(state, &log)?;
    Ok(())
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

/// Builds a PoW puzzle for a request token and encodes it as a payload.
fn build_request_puzzle(recipient_key: &[u8]) -> Result<PuzzleToken, CliError> {
    let params = pow::PowParams::default();
    let nonce = pow::generate_pow_nonce(pow::PowNonceParams::default());
    let puzzle_input = pow::build_puzzle_input(&nonce, recipient_key);
    let puzzle_output = pow::generate_puzzle_output(recipient_key, &puzzle_input, params)
        .map_err(|err| CliError::Crypto(err.to_string()))?;
    Ok(PuzzleToken {
        recipient_key: BASE64_STANDARD.encode(recipient_key),
        puzzle_input: BASE64_STANDARD.encode(&puzzle_input),
        puzzle_output: BASE64_STANDARD.encode(&puzzle_output),
        params: PowParamsPayload {
            memory_kib: params.memory_kib,
            iterations: params.iterations,
            parallelism: params.parallelism,
            output_len: params.output_len,
        },
    })
}

/// Validates the PoW puzzle embedded in a request token.
fn validate_request_puzzle(puzzle: &PuzzleToken, recipient_key: &[u8]) -> Result<(), CliError> {
    let recipient_bytes = BASE64_STANDARD
        .decode(puzzle.recipient_key.as_bytes())
        .map_err(|_| CliError::InvalidPuzzle)?;
    if recipient_bytes != recipient_key {
        return Err(CliError::InvalidPuzzle);
    }
    let puzzle_input = BASE64_STANDARD
        .decode(puzzle.puzzle_input.as_bytes())
        .map_err(|_| CliError::InvalidPuzzle)?;
    let puzzle_output = BASE64_STANDARD
        .decode(puzzle.puzzle_output.as_bytes())
        .map_err(|_| CliError::InvalidPuzzle)?;
    let params = pow::PowParams {
        memory_kib: puzzle.params.memory_kib,
        iterations: puzzle.params.iterations,
        parallelism: puzzle.params.parallelism,
        output_len: puzzle.params.output_len,
    };
    let valid = pow::verify_puzzle_output(recipient_key, &puzzle_input, &puzzle_output, params)
        .map_err(|err| CliError::Crypto(err.to_string()))?;
    if !valid {
        return Err(CliError::InvalidPuzzle);
    }
    Ok(())
}

/// Signs a grant token and returns a payload compatible with bridge storage requests.
fn sign_grant(identity: &IdentityState, grant: &GrantToken) -> Result<SignedGrantToken, CliError> {
    let signing_key = SigningKey::from_bytes(&hex::decode(&identity.signing_key_hex)?);
    let verifying_key = ed25519_verify_key(&signing_key);
    let hash = hash_ctap2_cbor_value(HashId::Sha256, grant)?;
    let signature = ed25519_sign_hash(&signing_key, &hash);
    Ok(SignedGrantToken {
        user_id: BASE64_STANDARD.encode(&grant.user_id),
        role: grant.role,
        flags: grant.flags,
        expires_at: grant.expires_at,
        verifying_key: BASE64_STANDARD.encode(verifying_key.to_bytes()),
        signature: BASE64_STANDARD.encode(signature.to_bytes()),
    })
}

/// Builds a thread configuration hash for MLS metadata.
fn build_thread_cfg_hash(thread_id_hex: &str, proto_suite: &ProtoSuite) -> Result<Vec<u8>, CliError> {
    let thread_id = hex::decode(thread_id_hex)?;
    let thread_config = ThreadConfig {
        proto_major: proto_suite.major,
        proto_minor: proto_suite.minor,
        ciphersuite_id: proto_suite.ciphersuite_id,
        flags: 0,
        thread_id,
        grants: Vec::new(),
        extensions: Default::default(),
    };
    cfg_hash_for_thread_config(HashId::Sha256, &thread_config)
        .map_err(|err| CliError::Mls(err.to_string()))
}

/// Rebuilds an MLS group from stored thread metadata.
fn rebuild_thread_group(thread_state: &ThreadState) -> Result<Group, CliError> {
    let proto_suite = ProtoSuite {
        major: thread_state.proto_major,
        minor: thread_state.proto_minor,
        ciphersuite_id: thread_state.ciphersuite_id,
    };
    let cfg_hash = hex::decode(&thread_state.cfg_hash_hex)?;
    build_group_for_members(&thread_state.members, &cfg_hash, &thread_state.initial_secret_hex, proto_suite)
}

/// Builds a new MLS group and adds the provided members in order.
fn build_group_for_members(
    members: &[String],
    cfg_hash: &[u8],
    initial_secret_hex: &str,
    proto_suite: ProtoSuite,
) -> Result<Group, CliError> {
    let initial_secret = hex::decode(initial_secret_hex)?;
    let mut group = Group::create(
        GroupConfig::new(
            proto_suite,
            0,
            HashId::Sha256 as u16,
            cfg_hash.to_vec(),
        )
        .with_initial_secret(initial_secret),
    )
    .map_err(|err| CliError::Mls(err.to_string()))?;
    for member in members {
        group
            .add_member(member.clone())
            .map_err(|err| CliError::Mls(err.to_string()))?;
    }
    Ok(group)
}

/// Encrypts a message with MLS-derived AEAD and chunks it for transport publication.
fn encrypt_and_chunk_message(
    identity: &IdentityState,
    thread_state: &mut ThreadState,
    plaintext: &[u8],
) -> Result<(spex_core::types::Envelope, ChunkManifest, Vec<Chunk>), CliError> {
    let group = rebuild_thread_group(thread_state)?;
    let proto_suite = ProtoSuite {
        major: thread_state.proto_major,
        minor: thread_state.proto_minor,
        ciphersuite_id: thread_state.ciphersuite_id,
    };
    let epoch = u32::try_from(group.epoch()).map_err(|_| CliError::InvalidAeadPayload)?;
    let seq = thread_state.next_seq;
    thread_state.next_seq = thread_state.next_seq.saturating_add(1);
    thread_state.epoch = group.epoch();
    let thread_id_bytes = hex::decode(&thread_state.thread_id_hex)?;
    let thread_id = to_fixed::<32>(&thread_id_bytes)?;
    let cfg_hash_bytes = hex::decode(&thread_state.cfg_hash_hex)?;
    let cfg_hash = to_fixed::<32>(&cfg_hash_bytes)?;
    let sender_id_bytes = hex::decode(&identity.user_id_hex)?;
    let sender_ad_id = derive_sender_ad_id(&sender_id_bytes);
    let ad = aead_ad::build_ad(&thread_id, epoch, &cfg_hash, proto_suite, seq, &sender_ad_id);
    let ciphertext = encrypt_payload_with_aead(group.group_secret(), plaintext, &ad)?;
    let envelope = build_envelope(identity, thread_state, epoch, seq, ciphertext)?;
    let payload = envelope.to_ctap2_canonical_bytes()?;
    let chunks = chunk_data(&ChunkingConfig::default(), &payload);
    let manifest = ChunkManifest {
        chunks: chunks
            .iter()
            .map(|chunk| ChunkDescriptor {
                index: chunk.index,
                hash: chunk.hash.clone(),
            })
            .collect(),
        total_len: payload.len(),
    };
    Ok((envelope, manifest, chunks))
}

/// Encrypts a plaintext payload using a group secret and AEAD additional data.
fn encrypt_payload_with_aead(
    group_secret: &[u8],
    plaintext: &[u8],
    ad: &[u8],
) -> Result<Vec<u8>, CliError> {
    let key = hash_bytes(HashId::Sha256, group_secret);
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|err| CliError::Crypto(err.to_string()))?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, Payload { msg: plaintext, aad: ad })
        .map_err(|err| CliError::Crypto(err.to_string()))?;
    let mut output = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Builds a sealed envelope and attaches a signature from the local identity.
fn build_envelope(
    identity: &IdentityState,
    thread_state: &ThreadState,
    epoch: u32,
    seq: u64,
    ciphertext: Vec<u8>,
) -> Result<spex_core::types::Envelope, CliError> {
    let mut envelope = spex_core::types::Envelope {
        thread_id: hex::decode(&thread_state.thread_id_hex)?,
        epoch,
        seq,
        sender_user_id: hex::decode(&identity.user_id_hex)?,
        ciphertext,
        signature: None,
        extensions: Default::default(),
    };
    let signing_key = SigningKey::from_bytes(&hex::decode(&identity.signing_key_hex)?);
    let hash = hash_ctap2_cbor_value(HashId::Sha256, &envelope)?;
    let signature = ed25519_sign_hash(&signing_key, &hash);
    envelope.signature = Some(signature.to_bytes().to_vec());
    Ok(envelope)
}

/// Derives a stable 20-byte sender identifier for AEAD additional data.
fn derive_sender_ad_id(sender_id: &[u8]) -> [u8; 20] {
    let digest = hash_bytes(HashId::Sha256, sender_id);
    let mut out = [0u8; 20];
    out.copy_from_slice(&digest[..20]);
    out
}

/// Converts a transport manifest into a serializable state entry.
fn manifest_state(manifest: &ChunkManifest) -> TransportManifestState {
    TransportManifestState {
        total_len: manifest.total_len,
        chunks: manifest
            .chunks
            .iter()
            .map(|chunk| TransportChunkDescriptorState {
                index: chunk.index,
                hash_hex: hex::encode(&chunk.hash),
            })
            .collect(),
    }
}

/// Converts transport chunk data into a serializable state entry.
fn chunk_states(chunks: &[Chunk]) -> Vec<TransportChunkState> {
    chunks
        .iter()
        .map(|chunk| TransportChunkState {
            index: chunk.index,
            hash_hex: hex::encode(&chunk.hash),
            data_base64: BASE64_STANDARD.encode(&chunk.data),
        })
        .collect()
}

/// Stages encrypted payloads into the local P2P inbox cache for recipients.
fn stage_p2p_inbox_delivery(
    state: &mut LocalState,
    thread_state: &ThreadState,
    sender_user_id_hex: &str,
    envelope: &spex_core::types::Envelope,
) -> Result<(), CliError> {
    let payload = envelope.to_ctap2_canonical_bytes()?;
    for member in &thread_state.members {
        if member == sender_user_id_hex {
            continue;
        }
        let member_bytes = hex::decode(member)?;
        let scan = derive_inbox_scan_key(HashId::Sha256, &member_bytes);
        let key_hex = hex::encode(scan.hashed_key);
        state
            .p2p_inbox
            .entry(key_hex)
            .or_default()
            .push(BASE64_STANDARD.encode(&payload));
    }
    Ok(())
}

/// Resolves inbox payloads from local P2P cache or the bridge fallback.
async fn resolve_inbox_from_state(
    state: &mut LocalState,
    inbox_scan_key: &[u8],
    bridge: Option<&BridgeClient>,
) -> Result<InboxScanResponse, CliError> {
    let scan = derive_inbox_scan_key(HashId::Sha256, inbox_scan_key);
    let key_hex = hex::encode(scan.hashed_key);
    if let Some(items) = state.p2p_inbox.remove(&key_hex) {
        let decoded = items
            .into_iter()
            .filter_map(|item| BASE64_STANDARD.decode(item.as_bytes()).ok())
            .collect();
        return Ok(InboxScanResponse {
            items: decoded,
            source: InboxSource::Kademlia,
        });
    }
    if let Some(client) = bridge {
        let response = client
            .scan_inbox(inbox_scan_key)
            .await
            .map_err(|err| CliError::Transport(err.to_string()))?;
        return Ok(response);
    }
    Ok(InboxScanResponse {
        items: Vec::new(),
        source: InboxSource::Kademlia,
    })
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
