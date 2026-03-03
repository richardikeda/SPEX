//! CBOR canonicalization boundaries for SPEX.
//!
//! The test vectors in v0.1.1 use a deterministic "canonical-ish" CBOR encoding: map keys are
//! ordered and lengths are deterministic, but the full CTAP2 canonical CBOR rules (especially for
//! floating point and tag normalization) are not enforced here. Any interoperable implementation
//! must apply complete CTAP2 canonicalization before hashing or signing CBOR payloads.
//!
//! The helpers in this module expose CTAP2 canonical CBOR encoding for CBOR values or any serde
//! serializable input. Use the resulting bytes as the canonical representation before hashing or
//! signing.

use half::f16;
use serde::{de::DeserializeOwned, Serialize};
use serde_cbor::Value;

use crate::error::SpexError;

/// Indicates which level of canonicalization is required by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalizationLevel {
    /// Deterministic encoding used in the current test vectors.
    CanonicalIsh,
    /// Full CTAP2 canonical CBOR (required for interop).
    Ctap2Canonical,
}

const MAJOR_UINT: u8 = 0x00;
const MAJOR_NEGINT: u8 = 0x20;
const MAJOR_BYTES: u8 = 0x40;
const MAJOR_TEXT: u8 = 0x60;
const MAJOR_ARRAY: u8 = 0x80;
const MAJOR_MAP: u8 = 0xa0;
const MAJOR_TAG: u8 = 0xc0;
const MAJOR_SIMPLE: u8 = 0xe0;

/// Serialize any value to CTAP2 canonical CBOR bytes.
///
/// This enforces CTAP2 rules: map keys are sorted by their canonical encoding, lengths are
/// minimal, floats are normalized to the shortest representation that preserves the value, and
/// tags use their minimal-width encoding.
pub fn to_ctap2_canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, SpexError> {
    let value = serde_cbor::value::to_value(value)?;
    ctap2_canonical_value_bytes(&value)
}

/// Deserialize a CTAP2 canonical CBOR payload into a typed value.
///
/// This validates canonical encoding before decoding into the target type.
pub fn from_ctap2_canonical_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SpexError> {
    let value = ctap2_canonical_value_from_slice(bytes)?;
    Ok(serde_cbor::value::from_value(value)?)
}

/// Serialize a CBOR value to CTAP2 canonical CBOR bytes.
pub fn ctap2_canonical_value_bytes(value: &Value) -> Result<Vec<u8>, SpexError> {
    let mut output = Vec::new();
    encode_value(value, &mut output)?;
    Ok(output)
}

/// Deserialize a CTAP2 canonical CBOR payload into a CBOR value.
///
/// The input must already be canonical. Non-canonical encodings are rejected.
/// Parses arbitrary CBOR payload bytes into a generic CBOR value.
///
/// This function is intended for robustness checks and boundary validation,
/// and must return explicit errors for malformed payloads.
pub fn parse_cbor_payload(bytes: &[u8]) -> Result<Value, SpexError> {
    Ok(serde_cbor::from_slice(bytes)?)
}

pub fn ctap2_canonical_value_from_slice(bytes: &[u8]) -> Result<Value, SpexError> {
    let value = serde_cbor::from_slice(bytes)?;
    let canonical = ctap2_canonical_value_bytes(&value)?;
    if canonical != bytes {
        return Err(SpexError::CborNotCanonical);
    }
    Ok(value)
}

fn encode_value(value: &Value, output: &mut Vec<u8>) -> Result<(), SpexError> {
    match value {
        Value::Null => output.push(MAJOR_SIMPLE | 22),
        Value::Bool(false) => output.push(MAJOR_SIMPLE | 20),
        Value::Bool(true) => output.push(MAJOR_SIMPLE | 21),
        Value::Integer(value) => encode_integer(*value, output)?,
        Value::Bytes(bytes) => {
            encode_length(MAJOR_BYTES, bytes.len(), output)?;
            output.extend_from_slice(bytes);
        }
        Value::Text(text) => {
            encode_length(MAJOR_TEXT, text.len(), output)?;
            output.extend_from_slice(text.as_bytes());
        }
        Value::Array(values) => {
            encode_length(MAJOR_ARRAY, values.len(), output)?;
            for item in values {
                encode_value(item, output)?;
            }
        }
        Value::Map(entries) => {
            let mut encoded_entries = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                let mut key_bytes = Vec::new();
                encode_value(key, &mut key_bytes)?;
                let mut value_bytes = Vec::new();
                encode_value(value, &mut value_bytes)?;
                encoded_entries.push((key_bytes, value_bytes));
            }
            encoded_entries.sort_by(|(left_key, _), (right_key, _)| {
                left_key
                    .len()
                    .cmp(&right_key.len())
                    .then_with(|| left_key.cmp(right_key))
            });
            encode_length(MAJOR_MAP, encoded_entries.len(), output)?;
            for (key, value) in encoded_entries {
                output.extend_from_slice(&key);
                output.extend_from_slice(&value);
            }
        }
        Value::Tag(tag, tagged) => {
            encode_u64(MAJOR_TAG, *tag, output)?;
            encode_value(tagged, output)?;
        }
        Value::Float(float) => encode_float(*float, output),
        Value::__Hidden => {
            return Err(SpexError::InvalidInput(
                "unsupported CBOR value variant".to_string(),
            ))
        }
    }
    Ok(())
}

fn encode_integer(value: i128, output: &mut Vec<u8>) -> Result<(), SpexError> {
    if value >= 0 {
        let unsigned = u64::try_from(value).map_err(|_| SpexError::CborIntegerOutOfRange)?;
        encode_u64(MAJOR_UINT, unsigned, output)?;
    } else {
        let unsigned = u64::try_from(-1 - value).map_err(|_| SpexError::CborIntegerOutOfRange)?;
        encode_u64(MAJOR_NEGINT, unsigned, output)?;
    }
    Ok(())
}

fn encode_length(major: u8, length: usize, output: &mut Vec<u8>) -> Result<(), SpexError> {
    let length = u64::try_from(length).map_err(|_| SpexError::CborIntegerOutOfRange)?;
    encode_u64(major, length, output)
}

fn encode_u64(major: u8, value: u64, output: &mut Vec<u8>) -> Result<(), SpexError> {
    match value {
        0..=23 => output.push(major | value as u8),
        24..=0xff => {
            output.push(major | 24);
            output.push(value as u8);
        }
        0x100..=0xffff => {
            output.push(major | 25);
            output.extend_from_slice(&(value as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            output.push(major | 26);
            output.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            output.push(major | 27);
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
    Ok(())
}

fn encode_float(value: f64, output: &mut Vec<u8>) {
    if value.is_nan() {
        output.push(MAJOR_SIMPLE | 25);
        output.extend_from_slice(&0x7e00u16.to_be_bytes());
        return;
    }

    let half = f16::from_f64(value);
    if half.to_f64() == value {
        output.push(MAJOR_SIMPLE | 25);
        output.extend_from_slice(&half.to_bits().to_be_bytes());
        return;
    }

    let float = value as f32;
    if f64::from(float) == value {
        output.push(MAJOR_SIMPLE | 26);
        output.extend_from_slice(&float.to_bits().to_be_bytes());
        return;
    }

    output.push(MAJOR_SIMPLE | 27);
    output.extend_from_slice(&value.to_bits().to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::{
        ctap2_canonical_value_bytes, ctap2_canonical_value_from_slice, parse_cbor_payload,
    };
    use crate::error::SpexError;
    use proptest::prelude::*;
    use serde_cbor::Value;

    fn stable_roundtrip(value: &Value) -> bool {
        let canonical = match ctap2_canonical_value_bytes(value) {
            Ok(bytes) => bytes,
            Err(_) => return true,
        };
        let decoded: Value = match serde_cbor::from_slice(&canonical) {
            Ok(decoded) => decoded,
            Err(_) => return false,
        };
        let recanonical = match ctap2_canonical_value_bytes(&decoded) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        canonical == recanonical
    }

    proptest! {
        /// Ensures CTAP2 canonicalization is idempotent for arbitrary CBOR-friendly values.
        #[test]
        fn canonicalization_is_idempotent_for_integer_arrays(values in proptest::collection::vec(any::<i64>(), 0..128)) {
            let value = Value::Array(values.into_iter().map(|item| Value::Integer(item as i128)).collect());
            prop_assert!(stable_roundtrip(&value));
        }

        /// Ensures arbitrary CBOR bytes never panic while being parsed.
        #[test]
        fn parse_cbor_payload_never_panics(input in proptest::collection::vec(any::<u8>(), 0..2048)) {
            let _ = parse_cbor_payload(&input);
        }
    }

    #[test]
    fn test_ctap2_canonical_value_from_slice() {
        // 1. Canonical input: integer 1
        let canonical_int = vec![0x01];
        let result = ctap2_canonical_value_from_slice(&canonical_int);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Integer(1));

        // 2. Non-canonical input: integer 1 with 1-byte encoding (0x18 0x01)
        let non_canonical_int = vec![0x18, 0x01];
        let result = ctap2_canonical_value_from_slice(&non_canonical_int);
        match result {
            Err(SpexError::CborNotCanonical) => (),
            _ => panic!("Expected CborNotCanonical error, got {:?}", result),
        }

        // 3. Canonical map: {0: 0, 1: 1}
        let canonical_map = vec![0xa2, 0x00, 0x00, 0x01, 0x01];
        let result = ctap2_canonical_value_from_slice(&canonical_map);
        assert!(result.is_ok());

        // 4. Non-canonical map: {1: 1, 0: 0} but encoded in that order
        let non_canonical_map = vec![0xa2, 0x01, 0x01, 0x00, 0x00];
        let result = ctap2_canonical_value_from_slice(&non_canonical_map);
        match result {
            Err(SpexError::CborNotCanonical) => (),
            _ => panic!("Expected CborNotCanonical error, got {:?}", result),
        }

        // 5. Invalid CBOR
        let invalid_cbor = vec![0xff];
        let result = ctap2_canonical_value_from_slice(&invalid_cbor);
        match result {
            Err(SpexError::Cbor(_)) => (),
            _ => panic!("Expected Cbor error, got {:?}", result),
        }
    }
}
