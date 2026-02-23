use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use ed25519_dalek::SigningKey;
use keyring::Entry;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use spex_core::{
    aead_ad,
    cbor::from_ctap2_canonical_slice,
    error::SpexError,
    hash::{hash_bytes, hash_ctap2_cbor_value, HashId},
    log::{
        CheckpointEntry, CheckpointLog, KeyCheckpoint, LogConsistency, RecoveryKey,
        RevocationDeclaration,
    },
    pow,
    sign::{ed25519_sign_hash, ed25519_verify_hash, ed25519_verify_key},
    types::{to_fixed, ContactCard, Ctap2Cbor, GrantToken, ProtoSuite, ThreadConfig},
    validation::{self, GrantPowValidationError},
};
use spex_mls::{cfg_hash_for_thread_config, Group, GroupConfig};
use spex_transport::{
    chunking::{chunk_data, Chunk, ChunkingConfig},
    inbox::{derive_inbox_scan_key, BridgeClient, InboxScanResponse, InboxSource},
    transport::{
        manifest_payload, reassemble_chunks_with_manifest, recover_chunks_from_store,
        recover_manifest_from_gossip, ChunkDescriptor, ChunkManifest, TransportConfig,
    },
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

/// Error type returned by client operations.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cbor error: {0}")]
    Cbor(#[from] serde_cbor::Error),
    #[error("hex error: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("missing identity")]
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
    #[error("grant signature invalid")]
    GrantInvalid,
    #[error("grant expired")]
    GrantExpired,
    #[error("invalid aead payload")]
    InvalidAeadPayload,
    #[error("missing envelope signature")]
    MissingSignature,
    #[error("unknown sender: {0}")]
    UnknownSender(String),
    #[error("sender not authorized for thread")]
    UnauthorizedSender,
    #[error("invalid message encoding")]
    InvalidMessageEncoding,
    #[error("core error: {0}")]
    Core(String),
    #[error("state encryption error: {0}")]
    StateEncryption(String),
}

/// Converts core errors into client errors for unified handling.
impl From<SpexError> for ClientError {
    /// Maps core errors into a string-based client error.
    fn from(err: SpexError) -> Self {
        ClientError::Core(err.to_string())
    }
}

/// Persistent client state representation.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LocalState {
    pub identity: Option<IdentityState>,
    pub contacts: HashMap<String, ContactState>,
    pub threads: HashMap<String, ThreadState>,
    pub inbox: Vec<InboxItem>,
    pub requests: Vec<RequestState>,
    pub grants: Vec<GrantState>,
    pub checkpoint_log: CheckpointLogState,
    #[serde(default)]
    pub transport_outbox: Vec<TransportOutboxItem>,
    #[serde(default)]
    pub p2p_inbox: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub transport_gossip: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub transport_chunk_store: HashMap<String, String>,
    #[serde(default)]
    pub key_rotations: Vec<KeyRotationState>,
}

/// Encrypted local state file wrapper.
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedStateFile {
    pub format: String,
    pub version: u8,
    pub kdf: String,
    pub salt_base64: String,
    pub nonce_base64: String,
    pub ciphertext_base64: String,
}

/// Supported key sources for local state encryption.
enum StateKeySource {
    Passphrase(String),
    Keychain(Vec<u8>),
}

const STATE_ENCRYPTION_FORMAT: &str = "spex_encrypted_state";
const STATE_ENCRYPTION_VERSION: u8 = 1;
const STATE_ENCRYPTION_AAD: &[u8] = b"spex-local-state";
const STATE_KEYCHAIN_SERVICE: &str = "spex";
const STATE_KEYCHAIN_USER: &str = "local-state";
const STATE_PASSPHRASE_FILE_ENV: &str = "SPEX_STATE_PASSPHRASE_FILE";

/// Identity data stored for the local user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityState {
    pub user_id_hex: String,
    pub signing_key_hex: String,
    pub verifying_key_hex: String,
    pub device_id_hex: String,
    pub device_nonce_hex: String,
}

/// Contact metadata stored after redeeming a card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactState {
    pub user_id_hex: String,
    pub verifying_key_hex: String,
    pub fingerprint: String,
    pub device_id_hex: String,
    pub last_seen_at: u64,
}

/// Thread metadata and local MLS settings.
#[derive(Debug, Serialize, Deserialize)]
pub struct ThreadState {
    pub thread_id_hex: String,
    pub members: Vec<String>,
    pub created_at: u64,
    pub messages: Vec<MessageState>,
    #[serde(default)]
    pub proto_major: u16,
    #[serde(default)]
    pub proto_minor: u16,
    #[serde(default)]
    pub ciphersuite_id: u16,
    #[serde(default)]
    pub cfg_hash_id: u16,
    #[serde(default)]
    pub cfg_hash_hex: String,
    #[serde(default)]
    pub flags: u8,
    #[serde(default)]
    pub initial_secret_hex: String,
    #[serde(default)]
    pub next_seq: u64,
    #[serde(default)]
    pub epoch: u64,
}

/// Local record of a message sent in a thread.
#[derive(Debug, Serialize, Deserialize)]
pub struct MessageState {
    pub sender_user_id: String,
    pub text: String,
    pub sent_at: u64,
}

/// Encoded inbox item stored locally.
#[derive(Debug, Serialize, Deserialize)]
pub struct InboxItem {
    pub received_at: u64,
    pub payload_base64: String,
}

/// Decrypted inbox payload data for a thread message.
#[derive(Debug)]
pub struct DecryptedInboxItem {
    pub thread_id_hex: String,
    pub sender_user_id_hex: String,
    pub plaintext: Vec<u8>,
}

/// Decryption response containing plaintext items and the transport source.
#[derive(Debug)]
pub struct InboxDecryptionResponse {
    pub items: Vec<DecryptedInboxItem>,
    pub source: InboxSource,
}

/// Stored request tokens for outgoing grants.
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestState {
    pub token_base64: String,
    pub to_user_id: String,
    pub role: u64,
    pub created_at: u64,
    #[serde(default)]
    pub puzzle: Option<PuzzleToken>,
}

/// Stored grant tokens created locally.
#[derive(Debug, Serialize, Deserialize)]
pub struct GrantState {
    pub token_base64: String,
    pub from_user_id: String,
    pub role: u64,
    pub created_at: u64,
    #[serde(default)]
    pub signature_base64: Option<String>,
}

/// Stored checkpoint log payload.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CheckpointLogState {
    pub log_base64: String,
}

/// Plain request token data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestToken {
    pub from_user_id: String,
    pub to_user_id: String,
    pub role: u64,
    pub created_at: u64,
    pub puzzle: Option<PuzzleToken>,
}

/// PoW puzzle data attached to request tokens.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PuzzleToken {
    pub recipient_key: String,
    pub puzzle_input: String,
    pub puzzle_output: String,
    pub params: PowParamsPayload,
}

/// PoW parameters serialized into request tokens.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PowParamsPayload {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
}

/// Signed grant token payload for bridging.
#[derive(Debug, Serialize, Deserialize)]
pub struct SignedGrantToken {
    pub user_id: String,
    pub role: u64,
    pub flags: Option<u64>,
    pub expires_at: Option<u64>,
    pub verifying_key: String,
    pub signature: String,
}

/// Stored transport outbox entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransportOutboxItem {
    pub thread_id_hex: String,
    pub manifest: TransportManifestState,
    pub chunks: Vec<TransportChunkState>,
    pub published_at: u64,
}

/// Stored manifest metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransportManifestState {
    pub total_len: usize,
    pub chunks: Vec<TransportChunkDescriptorState>,
}

/// Stored chunk descriptor metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransportChunkDescriptorState {
    pub index: usize,
    pub hash_hex: String,
}

/// Stored chunk payload and metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransportChunkState {
    pub index: usize,
    pub hash_hex: String,
    pub data_base64: String,
}

/// Stored key rotation metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyRotationState {
    pub rotated_at: u64,
    pub old_verifying_key_hex: String,
    pub new_verifying_key_hex: String,
}

/// Summary of an identity rotation performed against local state.
#[derive(Debug, Clone)]
pub struct IdentityRotationOutcome {
    pub old_verifying_key_hex: String,
    pub new_identity: IdentityState,
}

/// Details captured when a contact card is redeemed into local state.
#[derive(Debug, Clone)]
pub struct ContactRedeemOutcome {
    pub contact: ContactState,
    pub previous_fingerprint: Option<String>,
    pub key_changed: bool,
}

/// Response returned after creating a request token and storing it locally.
#[derive(Debug, Clone)]
pub struct RequestCreationOutcome {
    pub request: RequestToken,
    pub token_base64: String,
}

/// Response returned after accepting a request and storing the grant locally.
#[derive(Debug, Clone)]
pub struct GrantAcceptanceOutcome {
    pub request: RequestToken,
    pub grant_token_base64: String,
}

/// Transport payloads produced for a thread message send.
#[derive(Debug)]
pub struct ThreadMessageDispatch {
    pub thread_id_hex: String,
    pub sender_user_id_hex: String,
    pub manifest: ChunkManifest,
    pub chunks: Vec<Chunk>,
    pub chunk_count: usize,
    pub recipient_inbox_keys: Vec<Vec<u8>>,
}

/// Summary of a decrypted inbox message stored in local state.
#[derive(Debug, Clone)]
pub struct InboxMessageReceipt {
    pub thread_id_hex: String,
    pub sender_user_id_hex: String,
    pub text: String,
}

/// Generates a new identity state with fresh keys and device metadata.
pub fn create_identity() -> IdentityState {
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

/// Creates a new identity and stores it on local state.
pub fn create_identity_in_state(state: &mut LocalState) -> IdentityState {
    let identity = create_identity();
    state.identity = Some(identity.clone());
    identity
}

/// Rotates the local identity signing and verification keys in place.
pub fn rotate_identity(identity: &mut IdentityState) -> &IdentityState {
    let mut seed = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed);
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    identity.signing_key_hex = hex::encode(seed);
    identity.verifying_key_hex = hex::encode(verifying_key.to_bytes());
    identity.device_nonce_hex = hex::encode(random_bytes(16));
    identity
}

/// Rotates the local identity and persists rotation metadata in state.
pub fn rotate_identity_in_state(
    state: &mut LocalState,
) -> Result<IdentityRotationOutcome, ClientError> {
    let identity = state
        .identity
        .as_mut()
        .ok_or(ClientError::MissingIdentity)?;
    let old_verifying_key_hex = identity.verifying_key_hex.clone();
    rotate_identity(identity);
    let rotated = identity.clone();
    let rotation = KeyRotationState {
        rotated_at: now_unix(),
        old_verifying_key_hex: old_verifying_key_hex.clone(),
        new_verifying_key_hex: rotated.verifying_key_hex.clone(),
    };
    state.key_rotations.push(rotation);
    append_rotation_checkpoint(state, &rotated, &old_verifying_key_hex)?;
    Ok(IdentityRotationOutcome {
        old_verifying_key_hex,
        new_identity: rotated,
    })
}

/// Builds a signing key from a hex-encoded 32-byte secret.
fn signing_key_from_hex(signing_key_hex: &str) -> Result<SigningKey, ClientError> {
    let key_bytes: [u8; 32] = hex::decode(signing_key_hex)?
        .try_into()
        .map_err(|_| ClientError::SignatureInvalid)?;
    Ok(SigningKey::from_bytes(&key_bytes))
}

/// Appends rotation metadata to the checkpoint log and revokes the previous verifying key.
pub fn append_rotation_checkpoint(
    state: &mut LocalState,
    identity: &IdentityState,
    old_verifying_key_hex: &str,
) -> Result<(), ClientError> {
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
        .map_err(|err| ClientError::Log(err.to_string()))?;
    log.append(CheckpointEntry::Key(checkpoint))
        .map_err(|err| ClientError::Log(err.to_string()))?;
    save_checkpoint_log(state, &log)?;
    Ok(())
}

/// Builds a signed contact card payload for the current identity.
pub fn build_contact_card(identity: &IdentityState) -> Result<ContactCard, ClientError> {
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
pub fn sign_contact_card(
    identity: &IdentityState,
    card: &ContactCard,
) -> Result<Vec<u8>, ClientError> {
    let signing_key = signing_key_from_hex(&identity.signing_key_hex)?;
    let payload = card.to_ctap2_canonical_bytes()?;
    let digest = hash_bytes(HashId::Sha256, &payload);
    let signature = ed25519_sign_hash(&signing_key, &digest);
    Ok(signature.to_bytes().to_vec())
}

/// Verifies the signature embedded in a contact card.
pub fn verify_contact_card_signature(
    card: &ContactCard,
    signature: &[u8],
) -> Result<(), ClientError> {
    let verifying_key_bytes: [u8; 32] = card
        .verifying_key
        .as_slice()
        .try_into()
        .map_err(|_| ClientError::SignatureInvalid)?;
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&verifying_key_bytes)
        .map_err(|_| ClientError::SignatureInvalid)?;
    let mut unsigned_card = card.clone();
    unsigned_card.signature = None;
    let payload = unsigned_card.to_ctap2_canonical_bytes()?;
    let digest = hash_bytes(HashId::Sha256, &payload);
    let signature_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| ClientError::SignatureInvalid)?;
    let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);
    ed25519_verify_hash(&verifying_key, &digest, &signature)
        .map_err(|_| ClientError::SignatureInvalid)
}

/// Parses a CBOR contact card into a typed structure.
pub fn parse_contact_card(bytes: &[u8]) -> Result<ContactCard, ClientError> {
    let value: serde_cbor::Value = serde_cbor::from_slice(bytes)?;
    let map = match value {
        serde_cbor::Value::Map(map) => map,
        _ => return Err(ClientError::InvalidCard),
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
        return Err(ClientError::InvalidCard);
    }

    Ok(card)
}

/// Extracts bytes from a CBOR value or returns an invalid card error.
fn expect_bytes(value: serde_cbor::Value) -> Result<Vec<u8>, ClientError> {
    match value {
        serde_cbor::Value::Bytes(bytes) => Ok(bytes),
        _ => Err(ClientError::InvalidCard),
    }
}

/// Extracts a u64 from a CBOR value or returns an invalid card error.
fn expect_u64(value: serde_cbor::Value) -> Result<u64, ClientError> {
    match value {
        serde_cbor::Value::Integer(v) => u64::try_from(v).map_err(|_| ClientError::InvalidCard),
        _ => Err(ClientError::InvalidCard),
    }
}

/// Parses a base64 request token into its structured representation.
pub fn parse_request_token(token: &str) -> Result<RequestToken, ClientError> {
    let payload = BASE64_STANDARD
        .decode(token.as_bytes())
        .map_err(|_| ClientError::InvalidCard)?;
    Ok(serde_json::from_slice(&payload)?)
}

/// Builds a PoW puzzle for a request token and encodes it as a payload.
pub fn build_request_puzzle(recipient_key: &[u8]) -> Result<PuzzleToken, ClientError> {
    let params = pow::PowParams::default();
    let nonce = pow::generate_pow_nonce(pow::PowNonceParams::default());
    let puzzle_input = pow::build_puzzle_input(&nonce, recipient_key);
    let puzzle_output = pow::generate_puzzle_output(recipient_key, &puzzle_input, params)
        .map_err(|err| ClientError::Crypto(err.to_string()))?;
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
pub fn validate_request_puzzle(
    puzzle: &PuzzleToken,
    recipient_key: &[u8],
) -> Result<(), ClientError> {
    let recipient_bytes = BASE64_STANDARD
        .decode(puzzle.recipient_key.as_bytes())
        .map_err(|_| ClientError::InvalidPuzzle)?;
    if recipient_bytes != recipient_key {
        return Err(ClientError::InvalidPuzzle);
    }
    let puzzle_input = BASE64_STANDARD
        .decode(puzzle.puzzle_input.as_bytes())
        .map_err(|_| ClientError::InvalidPuzzle)?;
    let puzzle_output = BASE64_STANDARD
        .decode(puzzle.puzzle_output.as_bytes())
        .map_err(|_| ClientError::InvalidPuzzle)?;
    let params = pow::PowParams {
        memory_kib: puzzle.params.memory_kib,
        iterations: puzzle.params.iterations,
        parallelism: puzzle.params.parallelism,
        output_len: puzzle.params.output_len,
    };
    match validation::validate_pow_puzzle(
        recipient_key,
        &puzzle_input,
        &puzzle_output,
        params,
        pow::PowParams::minimum(),
    ) {
        Ok(()) => Ok(()),
        Err(GrantPowValidationError::PowTooWeak | GrantPowValidationError::PowInvalid) => {
            Err(ClientError::InvalidPuzzle)
        }
        Err(err) => Err(ClientError::Crypto(err.to_string())),
    }
}

/// Validates a signed grant token payload against signature and expiration rules.
pub fn validate_signed_grant(
    grant: &SignedGrantToken,
    now: u64,
) -> Result<GrantToken, ClientError> {
    let user_id = decode_base64_field(&grant.user_id)?;
    let verifying_key_bytes = decode_fixed_base64::<32>(&grant.verifying_key)?;
    let signature_bytes = decode_fixed_base64::<64>(&grant.signature)?;
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&verifying_key_bytes)
        .map_err(|_| ClientError::GrantInvalid)?;
    let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);
    let token = GrantToken {
        user_id,
        role: grant.role,
        flags: grant.flags,
        expires_at: grant.expires_at,
        extensions: Default::default(),
    };
    match validation::validate_grant_token(now, &token, &verifying_key, &signature) {
        Ok(()) => Ok(token),
        Err(GrantPowValidationError::GrantInvalid) => Err(ClientError::GrantInvalid),
        Err(GrantPowValidationError::GrantExpired) => Err(ClientError::GrantExpired),
        Err(err) => Err(ClientError::Crypto(err.to_string())),
    }
}

/// Decodes a base64-encoded field into bytes for validation helpers.
fn decode_base64_field(value: &str) -> Result<Vec<u8>, ClientError> {
    BASE64_STANDARD
        .decode(value.as_bytes())
        .map_err(|_| ClientError::GrantInvalid)
}

/// Decodes a fixed-length base64-encoded field into an array for validation helpers.
fn decode_fixed_base64<const N: usize>(value: &str) -> Result<[u8; N], ClientError> {
    let bytes = decode_base64_field(value)?;
    bytes.try_into().map_err(|_| ClientError::GrantInvalid)
}

/// Signs a grant token and returns a payload compatible with bridge storage requests.
pub fn sign_grant(
    identity: &IdentityState,
    grant: &GrantToken,
) -> Result<SignedGrantToken, ClientError> {
    let signing_key = signing_key_from_hex(&identity.signing_key_hex)?;
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
pub fn build_thread_cfg_hash(
    thread_id_hex: &str,
    proto_suite: &ProtoSuite,
) -> Result<Vec<u8>, ClientError> {
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
        .map_err(|err| ClientError::Mls(err.to_string()))
}

/// Rebuilds an MLS group from stored thread metadata.
pub fn rebuild_thread_group(thread_state: &ThreadState) -> Result<Group, ClientError> {
    let proto_suite = ProtoSuite {
        major: thread_state.proto_major,
        minor: thread_state.proto_minor,
        ciphersuite_id: thread_state.ciphersuite_id,
    };
    let cfg_hash = hex::decode(&thread_state.cfg_hash_hex)?;
    build_group_for_members(
        &thread_state.members,
        &cfg_hash,
        &thread_state.initial_secret_hex,
        proto_suite,
    )
}

/// Builds a new MLS group and adds the provided members in order.
pub fn build_group_for_members(
    members: &[String],
    cfg_hash: &[u8],
    initial_secret_hex: &str,
    proto_suite: ProtoSuite,
) -> Result<Group, ClientError> {
    let initial_secret = hex::decode(initial_secret_hex)?;
    let mut group = Group::create(
        GroupConfig::new(proto_suite, 0, HashId::Sha256 as u16, cfg_hash.to_vec())
            .with_initial_secret(initial_secret),
    )
    .map_err(|err| ClientError::Mls(err.to_string()))?;
    for member in members {
        group
            .add_member(member.clone())
            .map_err(|err| ClientError::Mls(err.to_string()))?;
    }
    Ok(group)
}

/// Encrypts a message with MLS-derived AEAD and chunks it for transport publication.
pub fn encrypt_and_chunk_message(
    identity: &IdentityState,
    thread_state: &mut ThreadState,
    plaintext: &[u8],
) -> Result<(spex_core::types::Envelope, ChunkManifest, Vec<Chunk>), ClientError> {
    let group = rebuild_thread_group(thread_state)?;
    let proto_suite = ProtoSuite {
        major: thread_state.proto_major,
        minor: thread_state.proto_minor,
        ciphersuite_id: thread_state.ciphersuite_id,
    };
    let epoch = u32::try_from(group.epoch()).map_err(|_| ClientError::InvalidAeadPayload)?;
    let seq = thread_state.next_seq;
    thread_state.next_seq = thread_state.next_seq.saturating_add(1);
    thread_state.epoch = group.epoch();
    let thread_id_bytes = hex::decode(&thread_state.thread_id_hex)?;
    let thread_id = to_fixed::<32>(&thread_id_bytes)?;
    let cfg_hash_bytes = hex::decode(&thread_state.cfg_hash_hex)?;
    let cfg_hash = to_fixed::<32>(&cfg_hash_bytes)?;
    let sender_id_bytes = hex::decode(&identity.user_id_hex)?;
    let sender_ad_id = derive_sender_ad_id(&sender_id_bytes);
    let ad = aead_ad::build_ad(
        &thread_id,
        epoch,
        &cfg_hash,
        proto_suite,
        seq,
        &sender_ad_id,
    );
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
pub fn encrypt_payload_with_aead(
    group_secret: &[u8],
    plaintext: &[u8],
    ad: &[u8],
) -> Result<Vec<u8>, ClientError> {
    let key = hash_bytes(HashId::Sha256, group_secret);
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|err| ClientError::Crypto(err.to_string()))?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: ad,
            },
        )
        .map_err(|err| ClientError::Crypto(err.to_string()))?;
    let mut output = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Builds a sealed envelope and attaches a signature from the local identity.
pub fn build_envelope(
    identity: &IdentityState,
    thread_state: &ThreadState,
    epoch: u32,
    seq: u64,
    ciphertext: Vec<u8>,
) -> Result<spex_core::types::Envelope, ClientError> {
    let mut envelope = spex_core::types::Envelope {
        thread_id: hex::decode(&thread_state.thread_id_hex)?,
        epoch,
        seq,
        sender_user_id: hex::decode(&identity.user_id_hex)?,
        ciphertext,
        signature: None,
        extensions: Default::default(),
    };
    let signing_key = signing_key_from_hex(&identity.signing_key_hex)?;
    let hash = hash_ctap2_cbor_value(HashId::Sha256, &envelope)?;
    let signature = ed25519_sign_hash(&signing_key, &hash);
    envelope.signature = Some(signature.to_bytes().to_vec());
    Ok(envelope)
}

/// Derives a stable 20-byte sender identifier for AEAD additional data.
pub fn derive_sender_ad_id(sender_id: &[u8]) -> [u8; 20] {
    let digest = hash_bytes(HashId::Sha256, sender_id);
    let mut out = [0u8; 20];
    out.copy_from_slice(&digest[..20]);
    out
}

/// Converts a transport manifest into a serializable state entry.
pub fn manifest_state(manifest: &ChunkManifest) -> TransportManifestState {
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
pub fn chunk_states(chunks: &[Chunk]) -> Vec<TransportChunkState> {
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
pub fn stage_p2p_inbox_delivery(
    state: &mut LocalState,
    thread_state: &ThreadState,
    sender_user_id_hex: &str,
    envelope: &spex_core::types::Envelope,
) -> Result<(), ClientError> {
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

/// Stages manifest gossip payloads and chunks into the local transport cache.
pub fn stage_transport_delivery(
    state: &mut LocalState,
    thread_state: &ThreadState,
    sender_user_id_hex: &str,
    manifest: &ChunkManifest,
    chunks: &[Chunk],
) -> Result<(), ClientError> {
    stage_transport_delivery_for_members(
        state,
        &thread_state.members,
        sender_user_id_hex,
        manifest,
        chunks,
    )
}

/// Stages manifest gossip payloads and chunks into the local transport cache for members.
pub fn stage_transport_delivery_for_members(
    state: &mut LocalState,
    members: &[String],
    sender_user_id_hex: &str,
    manifest: &ChunkManifest,
    chunks: &[Chunk],
) -> Result<(), ClientError> {
    let manifest_payload =
        manifest_payload(manifest).map_err(|err| ClientError::Transport(err.to_string()))?;
    for member in members {
        if member == sender_user_id_hex {
            continue;
        }
        let member_bytes = hex::decode(member)?;
        let scan = derive_inbox_scan_key(HashId::Sha256, &member_bytes);
        let key_hex = hex::encode(scan.hashed_key);
        state
            .transport_gossip
            .entry(key_hex)
            .or_default()
            .push(BASE64_STANDARD.encode(&manifest_payload));
    }
    for chunk in chunks {
        state
            .transport_chunk_store
            .entry(hex::encode(&chunk.hash))
            .or_insert_with(|| BASE64_STANDARD.encode(&chunk.data));
    }
    Ok(())
}

/// Checks whether the local transport cache has manifests for an inbox key.
pub fn transport_inbox_has_items(state: &LocalState, inbox_scan_key: &[u8]) -> bool {
    let scan = derive_inbox_scan_key(HashId::Sha256, inbox_scan_key);
    let key_hex = hex::encode(scan.hashed_key);
    state
        .transport_gossip
        .get(&key_hex)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
}

/// Builds a chunk store map for the provided manifest from local cached chunk data.
fn build_transport_chunk_store(
    state: &LocalState,
    manifest: &ChunkManifest,
) -> Result<std::collections::HashMap<Vec<u8>, Vec<u8>>, ClientError> {
    let mut store = std::collections::HashMap::new();
    for descriptor in &manifest.chunks {
        let hash_hex = hex::encode(&descriptor.hash);
        let data_base64 = state
            .transport_chunk_store
            .get(&hash_hex)
            .ok_or_else(|| ClientError::Transport(format!("missing transport chunk {hash_hex}")))?;
        let data = BASE64_STANDARD
            .decode(data_base64.as_bytes())
            .map_err(|_| ClientError::Transport("invalid transport chunk encoding".to_string()))?;
        store.insert(descriptor.hash.clone(), data);
    }
    Ok(store)
}

/// Resolves inbox payloads from local P2P cache or the bridge fallback.
pub async fn resolve_inbox_from_state(
    state: &mut LocalState,
    inbox_scan_key: &[u8],
    bridge: Option<&BridgeClient>,
) -> Result<InboxScanResponse, ClientError> {
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
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        return Ok(response);
    }
    Ok(InboxScanResponse {
        items: Vec::new(),
        source: InboxSource::Kademlia,
    })
}

/// Resolves transport manifests, recovers chunks, and decrypts the recovered envelopes.
pub fn receive_transport_messages(
    state: &mut LocalState,
    inbox_scan_key: &[u8],
) -> Result<InboxDecryptionResponse, ClientError> {
    let scan = derive_inbox_scan_key(HashId::Sha256, inbox_scan_key);
    let key_hex = hex::encode(scan.hashed_key);
    let manifest_payloads = state.transport_gossip.remove(&key_hex).unwrap_or_default();
    let mut items = Vec::new();
    let config = TransportConfig::default();

    for payload_base64 in manifest_payloads {
        let payload = BASE64_STANDARD
            .decode(payload_base64.as_bytes())
            .map_err(|_| ClientError::Transport("invalid manifest payload encoding".to_string()))?;
        let manifest = recover_manifest_from_gossip(&[payload])
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        let store = build_transport_chunk_store(state, &manifest)?;
        let chunks = recover_chunks_from_store(&manifest, &store, &config)
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        let payload = reassemble_chunks_with_manifest(&manifest, &chunks, &config)
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        let envelope = parse_envelope_payload(&payload)?;
        let thread_id_hex = hex::encode(&envelope.thread_id);
        let thread_state = state
            .threads
            .get(&thread_id_hex)
            .ok_or(ClientError::ThreadNotFound)?;
        let plaintext = decrypt_thread_envelope(state, thread_state, &envelope)?;
        items.push(DecryptedInboxItem {
            thread_id_hex,
            sender_user_id_hex: hex::encode(&envelope.sender_user_id),
            plaintext,
        });
    }

    Ok(InboxDecryptionResponse {
        items,
        source: InboxSource::Kademlia,
    })
}

/// Computes a SHA-256 fingerprint for the provided bytes.
pub fn fingerprint_hex(bytes: &[u8]) -> String {
    let digest = hash_bytes(HashId::Sha256, bytes);
    hex::encode(digest)
}

/// Generates a random byte string encoded as hex with the requested length.
pub fn random_hex(len: usize) -> String {
    hex::encode(random_bytes(len))
}

/// Generates random bytes for identifiers and secrets.
pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut buffer);
    buffer
}

/// Returns the current UNIX timestamp in seconds.
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Loads persisted local state from disk.
pub fn load_state() -> Result<LocalState, ClientError> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(LocalState::default());
    }
    let contents = fs::read_to_string(&path)?;
    if let Some(encrypted) = parse_encrypted_state_file(&contents) {
        return decrypt_state_file(&encrypted);
    }
    Ok(serde_json::from_str(&contents)?)
}

/// Persists local state to disk in JSON format.
pub fn save_state(state: &LocalState) -> Result<(), ClientError> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = encrypt_state_file(state)?;
    fs::write(path, contents)?;
    Ok(())
}

/// Restores the checkpoint log from the persisted local state.
pub fn load_checkpoint_log(state: &LocalState) -> Result<CheckpointLog, ClientError> {
    if state.checkpoint_log.log_base64.is_empty() {
        return Ok(CheckpointLog::new());
    }
    CheckpointLog::import_base64(&state.checkpoint_log.log_base64)
        .map_err(|err| ClientError::Log(err.to_string()))
}

/// Saves a checkpoint log into the local state as base64 CBOR.
pub fn save_checkpoint_log(state: &mut LocalState, log: &CheckpointLog) -> Result<(), ClientError> {
    let encoded = log
        .export_base64()
        .map_err(|err| ClientError::Log(err.to_string()))?;
    state.checkpoint_log.log_base64 = encoded;
    Ok(())
}

/// Determines the path for the persisted local state file.
pub fn state_path() -> Result<PathBuf, ClientError> {
    if let Ok(path) = std::env::var("SPEX_STATE_PATH") {
        return Ok(PathBuf::from(path));
    }
    let home = dirs::home_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "home directory not found")
    })?;
    Ok(home.join(Path::new(".spex/state.json")))
}

/// Parses encrypted state wrapper data if it matches the expected format marker.
fn parse_encrypted_state_file(contents: &str) -> Option<EncryptedStateFile> {
    let parsed: EncryptedStateFile = serde_json::from_str(contents).ok()?;
    if parsed.format != STATE_ENCRYPTION_FORMAT {
        return None;
    }
    Some(parsed)
}

/// Encrypts local state data and returns a serialized wrapper payload.
fn encrypt_state_file(state: &LocalState) -> Result<String, ClientError> {
    let plaintext = serde_json::to_vec(state)?;
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    let key_source = resolve_state_key_source()?;
    let (kdf, salt_base64, key) = match key_source {
        StateKeySource::Passphrase(passphrase) => {
            let mut salt = [0u8; 16];
            rand::thread_rng().fill_bytes(&mut salt);
            let key = derive_key_from_passphrase(&passphrase, &salt)?;
            ("argon2id".to_string(), BASE64_STANDARD.encode(salt), key)
        }
        StateKeySource::Keychain(key_bytes) => {
            let key = parse_keychain_key(&key_bytes)?;
            ("raw".to_string(), String::new(), key)
        }
    };
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: STATE_ENCRYPTION_AAD,
            },
        )
        .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
    let wrapper = EncryptedStateFile {
        format: STATE_ENCRYPTION_FORMAT.to_string(),
        version: STATE_ENCRYPTION_VERSION,
        kdf,
        salt_base64,
        nonce_base64: BASE64_STANDARD.encode(nonce),
        ciphertext_base64: BASE64_STANDARD.encode(ciphertext),
    };
    Ok(serde_json::to_string_pretty(&wrapper)?)
}

/// Decrypts an encrypted state wrapper into the local state representation.
fn decrypt_state_file(state_file: &EncryptedStateFile) -> Result<LocalState, ClientError> {
    if state_file.version != STATE_ENCRYPTION_VERSION {
        return Err(ClientError::StateEncryption(format!(
            "unsupported state version {}",
            state_file.version
        )));
    }
    let nonce = BASE64_STANDARD
        .decode(state_file.nonce_base64.as_bytes())
        .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
    let ciphertext = BASE64_STANDARD
        .decode(state_file.ciphertext_base64.as_bytes())
        .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
    let key = match state_file.kdf.as_str() {
        "argon2id" => {
            let passphrase = resolve_passphrase_from_file()?;
            let salt = BASE64_STANDARD
                .decode(state_file.salt_base64.as_bytes())
                .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
            derive_key_from_passphrase(&passphrase, &salt)?
        }
        "raw" => {
            let key_bytes = resolve_keychain_key()?;
            parse_keychain_key(&key_bytes)?
        }
        other => return Err(ClientError::StateEncryption(format!("unknown kdf {other}"))),
    };
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: STATE_ENCRYPTION_AAD,
            },
        )
        .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

/// Reads the passphrase from the file path specified in the environment.
/// This avoids exposing the secret directly in environment variables.
fn resolve_passphrase_from_file() -> Result<String, ClientError> {
    let path = std::env::var(STATE_PASSPHRASE_FILE_ENV)
        .map_err(|_| ClientError::StateEncryption("missing passphrase file variable".to_string()))?;
    let passphrase = std::fs::read_to_string(path)
        .map_err(ClientError::Io)?
        .trim()
        .to_string();
    if passphrase.is_empty() {
        return Err(ClientError::StateEncryption("passphrase file is empty".to_string()));
    }
    Ok(passphrase)
}

/// Resolves the local state encryption key source, preferring passphrases over keychain entries.
fn resolve_state_key_source() -> Result<StateKeySource, ClientError> {
    if std::env::var(STATE_PASSPHRASE_FILE_ENV).is_ok() {
        let passphrase = resolve_passphrase_from_file()?;
        return Ok(StateKeySource::Passphrase(passphrase));
    }
    resolve_keychain_key().map(StateKeySource::Keychain)
}

/// Derives a symmetric key from a passphrase and salt using Argon2id.
fn derive_key_from_passphrase(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], ClientError> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
    Ok(key)
}

/// Loads or creates a keychain-backed encryption key for the local state.
fn resolve_keychain_key() -> Result<Vec<u8>, ClientError> {
    let entry = Entry::new(STATE_KEYCHAIN_SERVICE, STATE_KEYCHAIN_USER)
        .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
    match entry.get_password() {
        Ok(password) => BASE64_STANDARD
            .decode(password.as_bytes())
            .map_err(|err| ClientError::StateEncryption(err.to_string())),
        Err(err) => match err {
            keyring::Error::NoEntry => {
                let mut key = [0u8; 32];
                rand::thread_rng().fill_bytes(&mut key);
                let encoded = BASE64_STANDARD.encode(key);
                entry
                    .set_password(&encoded)
                    .map_err(|err| ClientError::StateEncryption(err.to_string()))?;
                Ok(key.to_vec())
            }
            _ => Err(ClientError::StateEncryption(err.to_string())),
        },
    }
}

/// Parses a raw keychain key into a fixed-length array, rejecting unsupported sizes.
fn parse_keychain_key(bytes: &[u8]) -> Result<[u8; 32], ClientError> {
    if bytes.len() != 32 {
        return Err(ClientError::StateEncryption(
            "keychain key must be 32 bytes".to_string(),
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(bytes);
    Ok(key)
}

/// Generates a base64 contact card payload for the provided identity.
pub fn create_contact_card_payload(identity: &IdentityState) -> Result<String, ClientError> {
    let card = build_contact_card(identity)?;
    let card_bytes = card.to_ctap2_canonical_bytes()?;
    Ok(BASE64_STANDARD.encode(card_bytes))
}

/// Parses and verifies a base64 contact card payload.
pub fn redeem_contact_card_payload(payload: &str) -> Result<ContactCard, ClientError> {
    let card_bytes = BASE64_STANDARD
        .decode(payload.as_bytes())
        .map_err(|_| ClientError::InvalidCard)?;
    let card = parse_contact_card(&card_bytes)?;
    if let Some(signature) = &card.signature {
        verify_contact_card_signature(&card, signature)?;
    }
    Ok(card)
}

/// Redeems a contact card payload and persists the contact in local state.
pub fn redeem_contact_card_to_state(
    state: &mut LocalState,
    payload: &str,
) -> Result<ContactRedeemOutcome, ClientError> {
    let card = redeem_contact_card_payload(payload)?;
    let user_id_hex = hex::encode(&card.user_id);
    let verifying_hex = hex::encode(&card.verifying_key);
    let fingerprint = fingerprint_hex(&card.verifying_key);
    let previous_fingerprint = state
        .contacts
        .get(&user_id_hex)
        .map(|contact| contact.fingerprint.clone());
    let key_changed = previous_fingerprint
        .as_ref()
        .map(|existing| existing != &fingerprint)
        .unwrap_or(false);
    let contact = ContactState {
        user_id_hex: user_id_hex.clone(),
        verifying_key_hex: verifying_hex,
        fingerprint: fingerprint.clone(),
        device_id_hex: hex::encode(&card.device_id),
        last_seen_at: now_unix(),
    };
    state.contacts.insert(user_id_hex, contact.clone());
    Ok(ContactRedeemOutcome {
        contact,
        previous_fingerprint,
        key_changed,
    })
}

/// Creates a new request token and returns its JSON/base64 representation.
pub fn create_request_payload(
    identity: &IdentityState,
    to_user_id_hex: &str,
    role: u64,
) -> Result<(RequestToken, String), ClientError> {
    let puzzle = build_request_puzzle(&hex::decode(to_user_id_hex)?)?;
    validate_request_puzzle(&puzzle, &hex::decode(to_user_id_hex)?)?;
    let request = RequestToken {
        from_user_id: identity.user_id_hex.clone(),
        to_user_id: to_user_id_hex.to_string(),
        role,
        created_at: now_unix(),
        puzzle: Some(puzzle),
    };
    let payload = serde_json::to_vec(&request)?;
    let token = BASE64_STANDARD.encode(payload);
    Ok((request, token))
}

/// Extracts a cloned identity from local state or returns an error.
fn clone_identity_from_state(state: &LocalState) -> Result<IdentityState, ClientError> {
    state
        .identity
        .as_ref()
        .cloned()
        .ok_or(ClientError::MissingIdentity)
}

/// Creates a request token, stores it in local state, and returns the payload.
pub fn create_request_for_state(
    state: &mut LocalState,
    to_user_id_hex: &str,
    role: u64,
) -> Result<RequestCreationOutcome, ClientError> {
    let identity = clone_identity_from_state(state)?;
    let (request, token) = create_request_payload(&identity, to_user_id_hex, role)?;
    state.requests.push(RequestState {
        token_base64: token.clone(),
        to_user_id: to_user_id_hex.to_string(),
        role,
        created_at: request.created_at,
        puzzle: request.puzzle.clone(),
    });
    Ok(RequestCreationOutcome {
        request,
        token_base64: token,
    })
}

/// Accepts a request payload and returns the signed grant token.
pub fn accept_request_payload(
    identity: &IdentityState,
    request_payload: &str,
) -> Result<(RequestToken, SignedGrantToken), ClientError> {
    let request_token = parse_request_token(request_payload)?;
    if request_token.to_user_id != identity.user_id_hex {
        return Err(ClientError::RequestTargetMismatch);
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
    Ok((request_token, signed_grant))
}

/// Accepts a request payload, stores the grant in state, and returns the grant token.
pub fn accept_request_for_state(
    state: &mut LocalState,
    request_payload: &str,
) -> Result<GrantAcceptanceOutcome, ClientError> {
    let identity = clone_identity_from_state(state)?;
    let (request_token, signed_grant) = accept_request_payload(&identity, request_payload)?;
    let grant_token = BASE64_STANDARD.encode(serde_json::to_vec(&signed_grant)?);
    state.grants.push(GrantState {
        token_base64: grant_token.clone(),
        from_user_id: request_token.from_user_id.clone(),
        role: request_token.role,
        created_at: now_unix(),
        signature_base64: Some(signed_grant.signature.clone()),
    });
    Ok(GrantAcceptanceOutcome {
        request: request_token,
        grant_token_base64: grant_token,
    })
}

/// Creates a new thread state with MLS metadata for the provided members.
pub fn create_thread_state(
    identity: &IdentityState,
    mut members: Vec<String>,
) -> Result<ThreadState, ClientError> {
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
    Ok(ThreadState {
        thread_id_hex: thread_id,
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
    })
}

/// Creates a thread state and stores it in local state.
pub fn create_thread_for_state(
    state: &mut LocalState,
    members: Vec<String>,
) -> Result<String, ClientError> {
    let identity = clone_identity_from_state(state)?;
    let thread = create_thread_state(&identity, members)?;
    let thread_id = thread.thread_id_hex.clone();
    state.threads.insert(thread_id.clone(), thread);
    Ok(thread_id)
}

/// Derives inbox scan keys for thread recipients excluding the sender.
pub fn inbox_keys_for_thread(
    thread_state: &ThreadState,
    sender_user_id_hex: &str,
) -> Result<Vec<Vec<u8>>, ClientError> {
    thread_state
        .members
        .iter()
        .filter(|member| *member != sender_user_id_hex)
        .map(|member| hex::decode(member).map_err(ClientError::from))
        .collect()
}

/// Sends a plaintext message for a thread and returns transport payloads.
pub fn send_thread_message(
    identity: &IdentityState,
    thread_state: &mut ThreadState,
    plaintext: &[u8],
) -> Result<(spex_core::types::Envelope, ChunkManifest, Vec<Chunk>), ClientError> {
    encrypt_and_chunk_message(identity, thread_state, plaintext)
}

/// Sends a thread message, persists local state changes, and returns transport payloads.
pub fn send_thread_message_for_state(
    state: &mut LocalState,
    thread_id_hex: &str,
    text: &str,
) -> Result<ThreadMessageDispatch, ClientError> {
    let identity = clone_identity_from_state(state)?;
    let (thread_id_hex, members, manifest, chunks, outbox_item, recipient_inbox_keys) = {
        let thread_state = state
            .threads
            .get_mut(thread_id_hex)
            .ok_or(ClientError::ThreadNotFound)?;
        let (_envelope, manifest, chunks, outbox_item) =
            publish_thread_message_transport(&identity, thread_state, text.as_bytes())?;
        let recipient_inbox_keys = inbox_keys_for_thread(thread_state, &identity.user_id_hex)?;
        let message = MessageState {
            sender_user_id: identity.user_id_hex.clone(),
            text: text.to_string(),
            sent_at: now_unix(),
        };
        thread_state.messages.push(message);
        (
            thread_state.thread_id_hex.clone(),
            thread_state.members.clone(),
            manifest,
            chunks,
            outbox_item,
            recipient_inbox_keys,
        )
    };
    state.transport_outbox.push(outbox_item);
    stage_transport_delivery_for_members(
        state,
        &members,
        &identity.user_id_hex,
        &manifest,
        &chunks,
    )?;
    Ok(ThreadMessageDispatch {
        thread_id_hex,
        sender_user_id_hex: identity.user_id_hex,
        chunk_count: chunks.len(),
        manifest,
        chunks,
        recipient_inbox_keys,
    })
}

/// Publishes a thread message via transport chunking and returns the envelope and outbox record.
pub fn publish_thread_message(
    identity: &IdentityState,
    thread_state: &mut ThreadState,
    plaintext: &[u8],
) -> Result<(spex_core::types::Envelope, TransportOutboxItem), ClientError> {
    let (envelope, manifest, chunks) =
        encrypt_and_chunk_message(identity, thread_state, plaintext)?;
    let outbox_item = TransportOutboxItem {
        thread_id_hex: thread_state.thread_id_hex.clone(),
        manifest: manifest_state(&manifest),
        chunks: chunk_states(&chunks),
        published_at: now_unix(),
    };
    Ok((envelope, outbox_item))
}

/// Publishes a thread message and returns transport-ready manifest and chunk payloads.
pub fn publish_thread_message_transport(
    identity: &IdentityState,
    thread_state: &mut ThreadState,
    plaintext: &[u8],
) -> Result<
    (
        spex_core::types::Envelope,
        ChunkManifest,
        Vec<Chunk>,
        TransportOutboxItem,
    ),
    ClientError,
> {
    let (envelope, manifest, chunks) =
        encrypt_and_chunk_message(identity, thread_state, plaintext)?;
    let outbox_item = TransportOutboxItem {
        thread_id_hex: thread_state.thread_id_hex.clone(),
        manifest: manifest_state(&manifest),
        chunks: chunk_states(&chunks),
        published_at: now_unix(),
    };
    Ok((envelope, manifest, chunks, outbox_item))
}

/// Resolves inbox payloads with a local cache fallback.
pub async fn receive_inbox_payloads(
    state: &mut LocalState,
    inbox_scan_key: &[u8],
    bridge: Option<&BridgeClient>,
) -> Result<InboxScanResponse, ClientError> {
    resolve_inbox_from_state(state, inbox_scan_key, bridge).await
}

/// Resolves inbox payloads, validates envelopes, and decrypts the message contents.
pub async fn receive_inbox_messages(
    state: &mut LocalState,
    inbox_scan_key: &[u8],
    bridge: Option<&BridgeClient>,
) -> Result<InboxDecryptionResponse, ClientError> {
    let response = resolve_inbox_from_state(state, inbox_scan_key, bridge).await?;
    let mut items = Vec::new();
    for payload in response.items {
        let envelope = parse_envelope_payload(&payload)?;
        let thread_id_hex = hex::encode(&envelope.thread_id);
        let thread_state = state
            .threads
            .get(&thread_id_hex)
            .ok_or(ClientError::ThreadNotFound)?;
        let plaintext = decrypt_thread_envelope(state, thread_state, &envelope)?;
        items.push(DecryptedInboxItem {
            thread_id_hex,
            sender_user_id_hex: hex::encode(&envelope.sender_user_id),
            plaintext,
        });
    }
    Ok(InboxDecryptionResponse {
        items,
        source: response.source,
    })
}

/// Decrypts envelope payloads, updates local thread state, and returns receipts.
pub fn ingest_inbox_payloads(
    state: &mut LocalState,
    payloads: Vec<Vec<u8>>,
) -> Result<Vec<InboxMessageReceipt>, ClientError> {
    let mut receipts = Vec::new();
    for payload in payloads {
        let envelope = parse_envelope_payload(&payload)?;
        let thread_id_hex = hex::encode(&envelope.thread_id);
        let plaintext = decrypt_thread_envelope(
            state,
            state
                .threads
                .get(&thread_id_hex)
                .ok_or(ClientError::ThreadNotFound)?,
            &envelope,
        )?;
        let message_text =
            String::from_utf8(plaintext).map_err(|_| ClientError::InvalidMessageEncoding)?;
        let thread_state = state
            .threads
            .get_mut(&thread_id_hex)
            .ok_or(ClientError::ThreadNotFound)?;
        thread_state.messages.push(MessageState {
            sender_user_id: hex::encode(&envelope.sender_user_id),
            text: message_text.clone(),
            sent_at: now_unix(),
        });
        receipts.push(InboxMessageReceipt {
            thread_id_hex,
            sender_user_id_hex: hex::encode(&envelope.sender_user_id),
            text: message_text,
        });
    }
    Ok(receipts)
}

/// Records decrypted inbox items into local thread state and returns receipts.
pub fn record_inbox_messages(
    state: &mut LocalState,
    items: Vec<DecryptedInboxItem>,
) -> Result<Vec<InboxMessageReceipt>, ClientError> {
    let mut receipts = Vec::new();
    for item in items {
        let message_text =
            String::from_utf8(item.plaintext).map_err(|_| ClientError::InvalidMessageEncoding)?;
        let thread_state = state
            .threads
            .get_mut(&item.thread_id_hex)
            .ok_or(ClientError::ThreadNotFound)?;
        thread_state.messages.push(MessageState {
            sender_user_id: item.sender_user_id_hex.clone(),
            text: message_text.clone(),
            sent_at: now_unix(),
        });
        receipts.push(InboxMessageReceipt {
            thread_id_hex: item.thread_id_hex,
            sender_user_id_hex: item.sender_user_id_hex,
            text: message_text,
        });
    }
    Ok(receipts)
}

/// Creates a recovery key entry for the local log.
pub fn create_recovery_entry(identity: &IdentityState) -> Result<RecoveryKey, ClientError> {
    Ok(RecoveryKey {
        user_id: hex::decode(&identity.user_id_hex)?,
        recovery_key: random_bytes(32),
        issued_at: now_unix(),
    })
}

/// Builds a key checkpoint entry for the log.
pub fn create_checkpoint_entry(identity: &IdentityState) -> Result<KeyCheckpoint, ClientError> {
    Ok(KeyCheckpoint {
        user_id: hex::decode(&identity.user_id_hex)?,
        verifying_key: hex::decode(&identity.verifying_key_hex)?,
        device_id: hex::decode(&identity.device_id_hex)?,
        issued_at: now_unix(),
    })
}

/// Builds a revocation declaration for a key.
pub fn create_revocation_entry(
    identity: &IdentityState,
    revoked_key_hex: &str,
    recovery_hex: Option<String>,
    reason: Option<String>,
) -> Result<RevocationDeclaration, ClientError> {
    let revoked_key = hex::decode(revoked_key_hex)?;
    let revoked_key_hash = hash_bytes(HashId::Sha256, &revoked_key);
    let recovery_key_hash = match recovery_hex {
        Some(value) => Some(hash_bytes(HashId::Sha256, &hex::decode(value)?)),
        None => None,
    };
    Ok(RevocationDeclaration {
        user_id: hex::decode(&identity.user_id_hex)?,
        revoked_key_hash,
        revoked_at: now_unix(),
        recovery_key_hash,
        reason,
    })
}

/// Reports consistency between two checkpoint logs.
pub fn log_consistency(local: &CheckpointLog, remote: &CheckpointLog) -> LogConsistency {
    local.compare_with(remote)
}

/// Parses a CTAP2 canonical CBOR payload into an Envelope.
pub fn parse_envelope_payload(payload: &[u8]) -> Result<spex_core::types::Envelope, ClientError> {
    Ok(from_ctap2_canonical_slice(payload)?)
}

/// Resolves a sender verifying key from local identity or contact metadata.
fn resolve_sender_verifying_key(
    state: &LocalState,
    sender_user_id_hex: &str,
) -> Result<[u8; 32], ClientError> {
    if let Some(identity) = &state.identity {
        if identity.user_id_hex == sender_user_id_hex {
            return hex::decode(&identity.verifying_key_hex)?
                .try_into()
                .map_err(|_| ClientError::SignatureInvalid);
        }
    }
    let contact = state
        .contacts
        .get(sender_user_id_hex)
        .ok_or_else(|| ClientError::UnknownSender(sender_user_id_hex.to_string()))?;
    hex::decode(&contact.verifying_key_hex)?
        .try_into()
        .map_err(|_| ClientError::SignatureInvalid)
}

/// Verifies an envelope signature against a provided verifying key.
fn verify_envelope_signature(
    envelope: &spex_core::types::Envelope,
    verifying_key_bytes: &[u8; 32],
) -> Result<(), ClientError> {
    let signature_bytes = envelope
        .signature
        .as_ref()
        .ok_or(ClientError::MissingSignature)?;
    let signature_bytes: [u8; 64] = signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ClientError::SignatureInvalid)?;
    let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(verifying_key_bytes)
        .map_err(|_| ClientError::SignatureInvalid)?;
    let mut unsigned_envelope = envelope.clone();
    unsigned_envelope.signature = None;
    let hash = hash_ctap2_cbor_value(HashId::Sha256, &unsigned_envelope)?;
    ed25519_verify_hash(&verifying_key, &hash, &signature)
        .map_err(|_| ClientError::SignatureInvalid)
}

/// Decrypts a sealed envelope payload for a thread using MLS-derived AEAD.
pub fn decrypt_thread_envelope(
    state: &LocalState,
    thread_state: &ThreadState,
    envelope: &spex_core::types::Envelope,
) -> Result<Vec<u8>, ClientError> {
    let thread_id_hex = hex::encode(&envelope.thread_id);
    if thread_state.thread_id_hex != thread_id_hex {
        return Err(ClientError::ThreadNotFound);
    }
    let sender_user_id_hex = hex::encode(&envelope.sender_user_id);
    if !thread_state.members.contains(&sender_user_id_hex) {
        return Err(ClientError::UnauthorizedSender);
    }
    let verifying_key_bytes = resolve_sender_verifying_key(state, &sender_user_id_hex)?;
    verify_envelope_signature(envelope, &verifying_key_bytes)?;
    let group = rebuild_thread_group(thread_state)?;
    let proto_suite = ProtoSuite {
        major: thread_state.proto_major,
        minor: thread_state.proto_minor,
        ciphersuite_id: thread_state.ciphersuite_id,
    };
    let thread_id_bytes = hex::decode(&thread_state.thread_id_hex)?;
    let thread_id = to_fixed::<32>(&thread_id_bytes)?;
    let cfg_hash_bytes = hex::decode(&thread_state.cfg_hash_hex)?;
    let cfg_hash = to_fixed::<32>(&cfg_hash_bytes)?;
    let sender_ad_id = derive_sender_ad_id(&envelope.sender_user_id);
    let ad = aead_ad::build_ad(
        &thread_id,
        envelope.epoch,
        &cfg_hash,
        proto_suite,
        envelope.seq,
        &sender_ad_id,
    );
    decrypt_payload_with_aead(group.group_secret(), &envelope.ciphertext, &ad)
}

/// Decrypts an AEAD payload using the group secret and additional data.
pub fn decrypt_payload_with_aead(
    group_secret: &[u8],
    ciphertext: &[u8],
    ad: &[u8],
) -> Result<Vec<u8>, ClientError> {
    if ciphertext.len() < 12 {
        return Err(ClientError::InvalidAeadPayload);
    }
    let key = hash_bytes(HashId::Sha256, group_secret);
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|err| ClientError::Crypto(err.to_string()))?;
    let (nonce_bytes, payload) = ciphertext.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: payload,
                aad: ad,
            },
        )
        .map_err(|err| ClientError::Crypto(err.to_string()))
}
