#![forbid(unsafe_code)]

//! SPEX core primitives and test vectors.
//!
//! ## Canonical-ish vs CTAP2 canonicalization
//! The hashing, signing, AEAD AD assembly, and MLS extension helpers operate over raw bytes as
//! provided and are therefore *canonical-ish* (they assume you already produced a deterministic
//! byte representation). Full CTAP2 canonical CBOR encoding is still required for cross-vendor
//! interoperability and must be applied before hashing or signing CBOR structures. The `cbor`
//! module documents this boundary explicitly.

pub mod aead_ad;
pub mod cbor;
pub mod error;
pub mod hash;
pub mod log;
pub mod mls_ext;
pub mod pow;
pub mod sign;
pub mod test_vectors;
pub mod types;
pub mod validation;

pub use error::SpexError;
