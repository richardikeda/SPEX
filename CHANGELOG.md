# Changelog

All notable changes to this project will be documented in this file.

This project follows **Semantic Versioning**:
https://semver.org/

---

## [Unreleased]

### Added
- Initial public release structure
- SPEX protocol core primitives
- Explicit grant-based permission model
- Canonical CBOR wire format
- MLS extensions for context binding
- PoW-based anti-abuse mechanism
- Non-trusted bridge + P2P transport model
- SPEX CLI reference implementation
- SPEX client SDK (initial)
- Security policy and governance documents

### Changed
- N/A

### Fixed
- N/A

### Security
- Security model formally documented
- Explicit threat model defined

---

## [0.1.0] – Initial Public Release

### Added
- Core SPEX protocol implementation
- Ed25519 identity and signature verification
- Grant tokens with expiration and capability flags
- PoW validation with minimum security parameters
- Canonical CBOR serialization (CTAP2)
- MLS integration for encrypted threads
- HTTP bridge API with explicit validation
- Chunked transport and inbox scanning
- CLI tooling for identity, cards, grants, and messaging
- Initial documentation set

### Security
- End-to-end encryption enforced
- Context binding via cfg_hash and epoch
- Transport treated as untrusted by design
- Key changes treated as critical events

---

## Versioning Notes

- Patch releases (`x.y.z`) include bug fixes and security patches
- Minor releases (`x.y`) may add features without breaking compatibility
- Major releases (`x`) may introduce breaking changes to the protocol

Breaking changes will always be documented clearly.

---

**Secure.  
Permissioned.  
Explicit.**
