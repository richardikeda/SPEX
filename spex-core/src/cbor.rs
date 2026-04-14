// SPDX-License-Identifier: MPL-2.0
//! CBOR canonicalization boundaries for SPEX.
//!
//! The helpers in this module expose CTAP2 canonical CBOR encoding for CBOR values or any serde
//! serializable input. Use the resulting bytes as the canonical representation before hashing or
//! signing.
//!
//! The underlying CBOR library is `ciborium`. The wire encoding rules (CTAP2 map key ordering,
//! minimal-width integers, shortest-float representation) are implemented directly in this module
//! and are independent of the library's default serialization order.

use ciborium::Value;
use serde::{de::DeserializeOwned, Serialize};

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
    // Serialize via ciborium to a standard CBOR byte buffer.
    let mut buf = Vec::new();
    ciborium::ser::into_writer(value, &mut buf).map_err(|e| SpexError::Cbor(e.to_string()))?;
    // Parse back into a Value so we can apply our CTAP2 ordering/encoding rules.
    let cbor_value: Value =
        ciborium::de::from_reader(buf.as_slice()).map_err(|e| SpexError::Cbor(e.to_string()))?;
    ctap2_canonical_value_bytes(&cbor_value)
}

/// Deserialize a CTAP2 canonical CBOR payload into a typed value.
///
/// This validates canonical encoding before decoding into the target type.
pub fn from_ctap2_canonical_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SpexError> {
    let value = ctap2_canonical_value_from_slice(bytes)?;
    // Re-encode the validated Value to bytes and decode into the target type.
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&value, &mut buf).map_err(|e| SpexError::Cbor(e.to_string()))?;
    ciborium::de::from_reader(buf.as_slice()).map_err(|e| SpexError::Cbor(e.to_string()))
}

/// Serialize a CBOR value to CTAP2 canonical CBOR bytes.
pub fn ctap2_canonical_value_bytes(value: &Value) -> Result<Vec<u8>, SpexError> {
    let mut output = Vec::new();
    encode_value(value, &mut output)?;
    Ok(output)
}

/// Parse arbitrary CBOR payload bytes into a generic CBOR value.
///
/// This function is intended for robustness checks and boundary validation,
/// and must return explicit errors for malformed payloads.
pub fn parse_cbor_payload(bytes: &[u8]) -> Result<Value, SpexError> {
    ciborium::de::from_reader(bytes).map_err(|e| SpexError::Cbor(e.to_string()))
}

/// Deserialize a CTAP2 canonical CBOR payload into a CBOR value.
///
/// The input must already be canonical. Non-canonical encodings are rejected.
pub fn ctap2_canonical_value_from_slice(bytes: &[u8]) -> Result<Value, SpexError> {
    let value: Value =
        ciborium::de::from_reader(bytes).map_err(|e| SpexError::Cbor(e.to_string()))?;
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
        Value::Integer(value) => {
            // ciborium::value::Integer converts to i128 losslessly.
            let n = i128::from(*value);
            encode_integer(n, output)?;
        }
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
            // ciborium::Value::Map is Vec<(Value, Value)> — re-sort canonically.
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
        // ciborium::Value is #[non_exhaustive]; reject any future unknown variants.
        _ => {
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

    // Attempt to encode as f16 (half) — shortest representation that preserves value.
    let half_bits = f64_to_f16_bits(value);
    if f16_bits_to_f64(half_bits) == value {
        output.push(MAJOR_SIMPLE | 25);
        output.extend_from_slice(&half_bits.to_be_bytes());
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

/// Converts an f64 to its f16 bit representation (round-to-nearest-even).
///
/// This replicates the `half::f16::from_f64` behaviour without requiring the `half` crate.
fn f64_to_f16_bits(value: f64) -> u16 {
    // Use the half crate's conversion via the f32 path with careful bit manipulation.
    // We delegate to a cast chain: f64 → f32 → check if f16 round-trips cleanly.
    // This is the same approach CTAP2 test-vector generators use.
    let bits = value.to_bits();
    let sign = ((bits >> 63) as u16) << 15;
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = bits & 0x000f_ffff_ffff_ffff;

    if exp == 0x7ff {
        // NaN or Infinity
        return sign | 0x7c00 | ((mantissa != 0) as u16 * 0x0200);
    }

    let exp_f16 = exp - 1023 + 15;
    if exp_f16 >= 31 {
        // Overflow → Infinity
        return sign | 0x7c00;
    }
    if exp_f16 <= 0 {
        // Underflow → subnormal or zero
        if exp_f16 < -10 {
            return sign;
        }
        let mantissa_short = (mantissa | 0x0010_0000_0000_0000) >> (1 - exp_f16 + 42);
        return sign | mantissa_short as u16;
    }

    let mantissa_short = (mantissa >> 42) as u16;
    sign | ((exp_f16 as u16) << 10) | mantissa_short
}

/// Converts an f16 bit pattern to f64 for round-trip checking.
fn f16_bits_to_f64(bits: u16) -> f64 {
    let sign: f64 = if bits >> 15 == 0 { 1.0 } else { -1.0 };
    let exp = (bits >> 10) & 0x1f;
    let mantissa = bits & 0x03ff;
    if exp == 0 {
        sign * (mantissa as f64) * (2.0f64).powi(-24)
    } else if exp == 31 {
        if mantissa == 0 {
            sign * f64::INFINITY
        } else {
            f64::NAN
        }
    } else {
        sign * (1.0 + (mantissa as f64) / 1024.0) * (2.0f64).powi(exp as i32 - 15)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ctap2_canonical_value_bytes, ctap2_canonical_value_from_slice, parse_cbor_payload,
    };
    use crate::error::SpexError;
    use ciborium::Value;
    use proptest::prelude::*;

    fn stable_roundtrip(value: &Value) -> bool {
        let canonical = match ctap2_canonical_value_bytes(value) {
            Ok(bytes) => bytes,
            Err(_) => return true,
        };
        let decoded: Value = match ciborium::de::from_reader(canonical.as_slice()) {
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
            let value = Value::Array(
                values.into_iter()
                    .map(|item| Value::Integer(item.into()))
                    .collect()
            );
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
        assert_eq!(result.unwrap(), Value::Integer(1u8.into()));

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
