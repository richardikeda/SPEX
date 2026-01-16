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
use serde::Serialize;
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

/// Serialize a CBOR value to CTAP2 canonical CBOR bytes.
pub fn ctap2_canonical_value_bytes(value: &Value) -> Result<Vec<u8>, SpexError> {
    let mut output = Vec::new();
    encode_value(value, &mut output)?;
    Ok(output)
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
        Value::Simple(simple) => encode_simple(*simple, output)?,
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

fn encode_simple(simple: u8, output: &mut Vec<u8>) -> Result<(), SpexError> {
    match simple {
        0..=23 => output.push(MAJOR_SIMPLE | simple),
        24..=0xff => {
            output.push(MAJOR_SIMPLE | 24);
            output.push(simple);
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
