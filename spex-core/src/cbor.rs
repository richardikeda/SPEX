//! CBOR canonicalization boundaries for SPEX.
//!
//! The test vectors in v0.1.1 use a deterministic "canonical-ish" CBOR encoding: map keys are
//! ordered and lengths are deterministic, but the full CTAP2 canonical CBOR rules (especially for
//! floating point and tag normalization) are not enforced here. Any interoperable implementation
//! must apply complete CTAP2 canonicalization before hashing or signing CBOR payloads.

/// Indicates which level of canonicalization is required by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalizationLevel {
    /// Deterministic encoding used in the current test vectors.
    CanonicalIsh,
    /// Full CTAP2 canonical CBOR (required for interop).
    Ctap2Canonical,
}
