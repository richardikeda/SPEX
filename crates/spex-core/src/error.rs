use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpexError {
    #[error("invalid length: {0}")]
    InvalidLength(&'static str),

    #[error("signature verification failed")]
    SigVerifyFailed,

    #[error("hex decode failed: {0}")]
    Hex(#[from] hex::FromHexError),
}
