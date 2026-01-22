use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use spex_core::{
    pow::PowParams,
    types::GrantToken,
    validation::{self, GrantPowValidationError},
};

use crate::error::TransportError;

/// PoW parameters serialized for P2P ingestion payloads.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PowParamsPayload {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
}

impl Default for PowParamsPayload {
    /// Returns the default PoW parameters for P2P ingestion payloads.
    fn default() -> Self {
        let params = PowParams::default();
        Self {
            memory_kib: params.memory_kib,
            iterations: params.iterations,
            parallelism: params.parallelism,
            output_len: params.output_len,
        }
    }
}

impl PowParamsPayload {
    /// Converts the payload into core PoW parameters.
    fn to_params(self) -> PowParams {
        PowParams {
            memory_kib: self.memory_kib,
            iterations: self.iterations,
            parallelism: self.parallelism,
            output_len: self.output_len,
        }
    }
}

/// Signed grant payload validated during P2P ingestion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct P2pGrantPayload {
    pub user_id: String,
    pub role: u64,
    pub flags: Option<u64>,
    pub expires_at: Option<u64>,
    pub verifying_key: String,
    pub signature: String,
}

/// Puzzle payload validated during P2P ingestion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct P2pPuzzlePayload {
    pub recipient_key: String,
    pub puzzle_input: String,
    pub puzzle_output: String,
    pub params: Option<PowParamsPayload>,
}

/// Validates a signed grant payload for P2P ingestion.
pub fn validate_p2p_grant_payload(
    now: u64,
    payload: &P2pGrantPayload,
) -> Result<(), TransportError> {
    let user_id = decode_base64(&payload.user_id)?;
    let verifying_key = decode_verifying_key(&payload.verifying_key)?;
    let signature = decode_signature(&payload.signature)?;
    let grant = GrantToken {
        user_id,
        role: payload.role,
        flags: payload.flags,
        expires_at: payload.expires_at,
        extensions: Default::default(),
    };
    validation::validate_grant_token(now, &grant, &verifying_key, &signature)
        .map_err(map_validation_error)
}

/// Validates a PoW puzzle payload for P2P ingestion.
pub fn validate_p2p_puzzle_payload(payload: &P2pPuzzlePayload) -> Result<(), TransportError> {
    let recipient_key = decode_base64(&payload.recipient_key)?;
    let puzzle_input = decode_base64(&payload.puzzle_input)?;
    let puzzle_output = decode_base64(&payload.puzzle_output)?;
    let params = payload.params.unwrap_or_default().to_params();
    validation::validate_pow_puzzle(
        &recipient_key,
        &puzzle_input,
        &puzzle_output,
        params,
        PowParams::minimum(),
    )
    .map_err(map_validation_error)
}

/// Converts a shared validation error into a transport-layer error.
fn map_validation_error(err: GrantPowValidationError) -> TransportError {
    match err {
        GrantPowValidationError::GrantExpired => TransportError::GrantExpired,
        GrantPowValidationError::GrantInvalid => TransportError::GrantInvalid,
        GrantPowValidationError::PowTooWeak => TransportError::PowTooWeak,
        GrantPowValidationError::PowInvalid => TransportError::PowInvalid,
        GrantPowValidationError::Core(err) => TransportError::CborDecode(err),
    }
}

/// Decodes a base64 string into raw bytes for validation payloads.
fn decode_base64(value: &str) -> Result<Vec<u8>, TransportError> {
    BASE64_STANDARD
        .decode(value)
        .map_err(|err| TransportError::InvalidPayload(err.to_string()))
}

/// Decodes a base64 string into a fixed-size byte array.
fn decode_fixed_bytes<const N: usize>(value: &str) -> Result<[u8; N], TransportError> {
    let bytes = decode_base64(value)?;
    bytes
        .try_into()
        .map_err(|_| TransportError::InvalidPayload("invalid length".to_string()))
}

/// Decodes a base64-encoded Ed25519 signature.
fn decode_signature(value: &str) -> Result<Signature, TransportError> {
    let bytes: [u8; 64] = decode_fixed_bytes(value)?;
    Ok(Signature::from_bytes(&bytes))
}

/// Decodes a base64-encoded Ed25519 verifying key.
fn decode_verifying_key(value: &str) -> Result<VerifyingKey, TransportError> {
    let bytes: [u8; 32] = decode_fixed_bytes(value)?;
    VerifyingKey::from_bytes(&bytes).map_err(|err| TransportError::InvalidPayload(err.to_string()))
}
