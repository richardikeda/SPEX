use crate::{
    error::SpexError,
    hash::{hash_ctap2_cbor_value, HashId},
    pow::{self, PowParams},
    sign::ed25519_verify_hash,
    types::GrantToken,
};
use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

/// Errors returned by shared grant and PoW validation helpers.
#[derive(Debug, Error)]
pub enum GrantPowValidationError {
    #[error("grant expired")]
    GrantExpired,
    #[error("grant signature invalid")]
    GrantInvalid,
    #[error("pow parameters below minimum")]
    PowTooWeak,
    #[error("pow puzzle invalid")]
    PowInvalid,
    #[error("core validation error: {0}")]
    Core(#[from] SpexError),
}

/// Validates a signed GrantToken against expiration and signature rules.
pub fn validate_grant_token(
    now: u64,
    grant: &GrantToken,
    verifying_key: &VerifyingKey,
    signature: &Signature,
) -> Result<(), GrantPowValidationError> {
    if let Some(expires_at) = grant.expires_at {
        if expires_at <= now {
            return Err(GrantPowValidationError::GrantExpired);
        }
    }

    let hash = hash_ctap2_cbor_value(HashId::Sha256, grant)?;
    ed25519_verify_hash(verifying_key, &hash, signature)
        .map_err(|_| GrantPowValidationError::GrantInvalid)?;
    Ok(())
}

/// Validates a PoW puzzle output against minimum PoW parameters and recipient data.
pub fn validate_pow_puzzle(
    recipient_key: &[u8],
    puzzle_input: &[u8],
    puzzle_output: &[u8],
    params: PowParams,
    minimum: PowParams,
) -> Result<(), GrantPowValidationError> {
    if params.memory_kib < minimum.memory_kib
        || params.iterations < minimum.iterations
        || params.parallelism < minimum.parallelism
        || params.output_len < minimum.output_len
    {
        return Err(GrantPowValidationError::PowTooWeak);
    }

    let valid = pow::verify_puzzle_output(recipient_key, puzzle_input, puzzle_output, params)?;
    if !valid {
        return Err(GrantPowValidationError::PowInvalid);
    }
    Ok(())
}
