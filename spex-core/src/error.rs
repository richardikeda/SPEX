use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpexError {
    #[error("invalid length: {0}")]
    InvalidLength(&'static str),

    #[error("signature verification failed")]
    SigVerifyFailed,

    #[error("argon2 error: {0}")]
    Argon2(#[from] argon2::Error),

    #[error("argon2 password hash error: {0}")]
    Argon2PasswordHash(#[from] argon2::password_hash::Error),

    #[error("hex decode failed: {0}")]
    Hex(#[from] hex::FromHexError),

    #[error("CBOR canonicalization failed: {0}")]
    Cbor(#[from] serde_cbor::Error),

    #[error("CBOR integer out of range for canonical encoding")]
    CborIntegerOutOfRange,

    #[error("CBOR is not CTAP2 canonical")]
    CborNotCanonical,
}
