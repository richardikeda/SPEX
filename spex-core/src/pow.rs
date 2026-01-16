use argon2::{Algorithm, Argon2, Params, Version};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::error::SpexError;

const SALT_CONTEXT: &[u8] = b"spex.pow.recipient.salt.v1";

/// Configurable Argon2id parameters for PoW generation and verification.
#[derive(Clone, Copy, Debug)]
pub struct PowParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
}

impl Default for PowParams {
    /// Returns the default Argon2id PoW parameters.
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
            output_len: 32,
        }
    }
}

impl PowParams {
    /// Builds an Argon2id instance from the configured parameters.
    fn to_argon2(self) -> Result<Argon2<'static>, SpexError> {
        let params = Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(self.output_len),
        )?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

/// Configurable parameters for PoW nonce generation and validation.
#[derive(Clone, Copy, Debug)]
pub struct PowNonceParams {
    pub nonce_len: usize,
}

impl Default for PowNonceParams {
    /// Returns the default nonce length for PoW inputs.
    fn default() -> Self {
        Self { nonce_len: 32 }
    }
}

/// Generates a recipient-derived salt using the default context label.
pub fn derive_recipient_salt(recipient_key: &[u8]) -> [u8; 32] {
    derive_recipient_salt_with_context(recipient_key, SALT_CONTEXT)
}

/// Generates a recipient-derived salt using a custom context label.
pub fn derive_recipient_salt_with_context(recipient_key: &[u8], context: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(context);
    hasher.update(recipient_key);
    let digest = hasher.finalize();
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&digest);
    salt
}

/// Generates a random PoW nonce using the provided parameters.
pub fn generate_pow_nonce(params: PowNonceParams) -> Vec<u8> {
    let mut nonce = vec![0u8; params.nonce_len];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

/// Validates a PoW nonce length against the configured parameters.
pub fn validate_pow_nonce(nonce: &[u8], params: PowNonceParams) -> Result<(), SpexError> {
    if nonce.len() != params.nonce_len {
        return Err(SpexError::InvalidLength("pow nonce"));
    }
    Ok(())
}

/// Concatenates the nonce with caller-provided puzzle input data.
pub fn build_puzzle_input(nonce: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut input = Vec::with_capacity(nonce.len() + payload.len());
    input.extend_from_slice(nonce);
    input.extend_from_slice(payload);
    input
}

/// Produces the Argon2id output for a puzzle input and recipient key.
pub fn generate_puzzle_output(
    recipient_key: &[u8],
    puzzle_input: &[u8],
    params: PowParams,
) -> Result<Vec<u8>, SpexError> {
    let salt = derive_recipient_salt(recipient_key);
    let argon2 = params.to_argon2()?;
    let mut output = vec![0u8; params.output_len];
    argon2.hash_password_into(puzzle_input, &salt, &mut output)?;
    Ok(output)
}

/// Verifies that a puzzle input produces the expected Argon2id output.
pub fn verify_puzzle_output(
    recipient_key: &[u8],
    puzzle_input: &[u8],
    expected: &[u8],
    params: PowParams,
) -> Result<bool, SpexError> {
    if expected.len() != params.output_len {
        return Err(SpexError::InvalidLength("argon2 output"));
    }
    let derived = generate_puzzle_output(recipient_key, puzzle_input, params)?;
    Ok(derived == expected)
}
