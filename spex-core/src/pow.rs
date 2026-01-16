use argon2::{Algorithm, Argon2, Params, Version};
use sha2::{Digest, Sha256};

use crate::error::SpexError;

const SALT_CONTEXT: &[u8] = b"spex.pow.recipient.salt.v1";

#[derive(Clone, Copy, Debug)]
pub struct PowParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
}

impl Default for PowParams {
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

pub fn derive_recipient_salt(recipient_key: &[u8]) -> [u8; 32] {
    derive_recipient_salt_with_context(recipient_key, SALT_CONTEXT)
}

pub fn derive_recipient_salt_with_context(recipient_key: &[u8], context: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(context);
    hasher.update(recipient_key);
    let digest = hasher.finalize();
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&digest);
    salt
}

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
