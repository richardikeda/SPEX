// SPDX-License-Identifier: MPL-2.0
use crate::{cbor, error::SpexError};
use ciborium::Value;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Converts a u64 BTreeMap key to a ciborium CBOR integer value.
///
/// All SPEX field IDs are non-negative u64 values, so this conversion is lossless.
fn key_to_cbor(k: u64) -> Value {
    Value::Integer(k.into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtoSuite {
    pub major: u16,
    pub minor: u16,
    pub ciphersuite_id: u16,
}

/// Helper trait for SPEX CBOR-encoded structures that must be canonicalized via CTAP2.
pub trait Ctap2Cbor {
    fn to_cbor_value(&self) -> Value;

    fn to_ctap2_canonical_bytes(&self) -> Result<Vec<u8>, SpexError> {
        cbor::ctap2_canonical_value_bytes(&self.to_cbor_value())
    }
}

/// Represents a contact card (user metadata + verification material).
// Eq is not derived because ciborium::Value (used in extensions) does not implement Eq.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContactCard {
    pub user_id: Vec<u8>,
    pub verifying_key: Vec<u8>,
    pub device_id: Vec<u8>,
    pub device_nonce: Vec<u8>,
    pub issued_at: u64,
    pub invite: Option<InviteToken>,
    pub signature: Option<Vec<u8>>,
    pub extensions: BTreeMap<u64, Value>,
}

impl Ctap2Cbor for ContactCard {
    fn to_cbor_value(&self) -> Value {
        // Use a u64-keyed BTreeMap for ordering before converting to Vec<(Value, Value)>.
        let mut map: BTreeMap<u64, Value> = BTreeMap::new();
        map.insert(0, Value::Bytes(self.user_id.clone()));
        map.insert(1, Value::Bytes(self.verifying_key.clone()));
        map.insert(2, Value::Bytes(self.device_id.clone()));
        map.insert(3, Value::Bytes(self.device_nonce.clone()));
        map.insert(4, Value::Integer(self.issued_at.into()));
        if let Some(invite) = &self.invite {
            map.insert(5, invite.to_cbor_value());
        }
        if let Some(signature) = &self.signature {
            map.insert(6, Value::Bytes(signature.clone()));
        }
        for (key, value) in &self.extensions {
            map.insert(*key, value.clone());
        }
        Value::Map(map.into_iter().map(|(k, v)| (key_to_cbor(k), v)).collect())
    }
}

impl ContactCard {
    /// Decodes a CTAP2-canonical CBOR payload into a ContactCard.
    ///
    /// This function never panics for malformed input and always reports
    /// decoding failures as explicit `SpexError` values.
    pub fn decode_ctap2(bytes: &[u8]) -> Result<Self, SpexError> {
        cbor::from_ctap2_canonical_slice(bytes)
    }
}

/// Represents an invite token (often embedded in cards or bridge messages).
// Eq is not derived because ciborium::Value (used in extensions) does not implement Eq.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteToken {
    pub major: u16,
    pub minor: u16,
    pub requires_puzzle: bool,
    pub extensions: BTreeMap<u64, Value>,
}

impl Ctap2Cbor for InviteToken {
    fn to_cbor_value(&self) -> Value {
        let mut map: BTreeMap<u64, Value> = BTreeMap::new();
        map.insert(0, Value::Integer((self.major as u64).into()));
        map.insert(1, Value::Integer((self.minor as u64).into()));
        map.insert(2, Value::Bool(self.requires_puzzle));
        for (key, value) in &self.extensions {
            map.insert(*key, value.clone());
        }
        Value::Map(map.into_iter().map(|(k, v)| (key_to_cbor(k), v)).collect())
    }
}

/// Represents a grant token (membership/permission entry).
// Eq is not derived because ciborium::Value (used in extensions) does not implement Eq.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GrantToken {
    pub user_id: Vec<u8>,
    pub role: u64,
    pub flags: Option<u64>,
    pub expires_at: Option<u64>,
    pub extensions: BTreeMap<u64, Value>,
}

impl Ctap2Cbor for GrantToken {
    fn to_cbor_value(&self) -> Value {
        let mut map: BTreeMap<u64, Value> = BTreeMap::new();
        map.insert(0, Value::Bytes(self.user_id.clone()));
        map.insert(1, Value::Integer(self.role.into()));
        if let Some(flags) = self.flags {
            map.insert(2, Value::Integer(flags.into()));
        }
        if let Some(expires_at) = self.expires_at {
            map.insert(3, Value::Integer(expires_at.into()));
        }
        for (key, value) in &self.extensions {
            map.insert(*key, value.clone());
        }
        Value::Map(map.into_iter().map(|(k, v)| (key_to_cbor(k), v)).collect())
    }
}

impl GrantToken {
    /// Decodes a CTAP2-canonical CBOR payload into a GrantToken.
    ///
    /// This function preserves validation boundaries by rejecting
    /// non-canonical and structurally invalid payloads with explicit errors.
    pub fn decode_ctap2(bytes: &[u8]) -> Result<Self, SpexError> {
        cbor::from_ctap2_canonical_slice(bytes)
    }
}

/// Represents the per-thread configuration blob.
// Eq is not derived because ciborium::Value (used in extensions) does not implement Eq.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadConfig {
    pub proto_major: u16,
    pub proto_minor: u16,
    pub ciphersuite_id: u16,
    pub flags: u8,
    pub thread_id: Vec<u8>,
    pub grants: Vec<GrantToken>,
    pub extensions: BTreeMap<u64, Value>,
}

impl Ctap2Cbor for ThreadConfig {
    fn to_cbor_value(&self) -> Value {
        let mut map: BTreeMap<u64, Value> = BTreeMap::new();
        map.insert(0, Value::Integer((self.proto_major as u64).into()));
        map.insert(1, Value::Integer((self.proto_minor as u64).into()));
        map.insert(2, Value::Integer((self.ciphersuite_id as u64).into()));
        map.insert(3, Value::Integer((self.flags as u64).into()));
        map.insert(4, Value::Bytes(self.thread_id.clone()));
        let grants = self
            .grants
            .iter()
            .map(|grant| grant.to_cbor_value())
            .collect();
        map.insert(5, Value::Array(grants));
        for (key, value) in &self.extensions {
            map.insert(*key, value.clone());
        }
        Value::Map(map.into_iter().map(|(k, v)| (key_to_cbor(k), v)).collect())
    }
}

/// Represents a sealed message envelope.
// Eq is not derived because ciborium::Value (used in extensions) does not implement Eq.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Envelope {
    pub thread_id: Vec<u8>,
    pub epoch: u32,
    pub seq: u64,
    pub sender_user_id: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub signature: Option<Vec<u8>>,
    pub extensions: BTreeMap<u64, Value>,
}

impl<'de> Deserialize<'de> for Envelope {
    /// Deserializes a CTAP2 canonical CBOR map into an Envelope.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(EnvelopeVisitor)
    }
}

struct EnvelopeVisitor;

impl<'de> Visitor<'de> for EnvelopeVisitor {
    type Value = Envelope;

    /// Describes the expected CBOR map structure for an Envelope.
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a CBOR map with integer keys for Envelope fields")
    }

    /// Parses the CBOR map entries and builds an Envelope value.
    fn visit_map<M>(self, mut map: M) -> Result<Envelope, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut thread_id = None;
        let mut epoch = None;
        let mut seq = None;
        let mut sender_user_id = None;
        let mut ciphertext = None;
        let mut signature = None;
        let mut extensions = BTreeMap::new();

        while let Some(key) = map.next_key::<i128>()? {
            let value: Value = map.next_value()?;
            match key {
                0 => thread_id = Some(value_as_bytes(value, "thread_id")?),
                1 => epoch = Some(value_as_u32(value, "epoch")?),
                2 => seq = Some(value_as_u64(value, "seq")?),
                3 => sender_user_id = Some(value_as_bytes(value, "sender_user_id")?),
                4 => ciphertext = Some(value_as_bytes(value, "ciphertext")?),
                5 => signature = Some(value_as_bytes(value, "signature")?),
                key if key >= 0 => {
                    extensions.insert(key as u64, value);
                }
                _ => {
                    return Err(de::Error::custom("negative extension key"));
                }
            }
        }

        Ok(Envelope {
            thread_id: thread_id.ok_or_else(|| de::Error::missing_field("thread_id"))?,
            epoch: epoch.ok_or_else(|| de::Error::missing_field("epoch"))?,
            seq: seq.ok_or_else(|| de::Error::missing_field("seq"))?,
            sender_user_id: sender_user_id
                .ok_or_else(|| de::Error::missing_field("sender_user_id"))?,
            ciphertext: ciphertext.ok_or_else(|| de::Error::missing_field("ciphertext"))?,
            signature,
            extensions,
        })
    }
}

impl Ctap2Cbor for Envelope {
    fn to_cbor_value(&self) -> Value {
        let mut map: BTreeMap<u64, Value> = BTreeMap::new();
        map.insert(0, Value::Bytes(self.thread_id.clone()));
        map.insert(1, Value::Integer((self.epoch as u64).into()));
        map.insert(2, Value::Integer(self.seq.into()));
        map.insert(3, Value::Bytes(self.sender_user_id.clone()));
        map.insert(4, Value::Bytes(self.ciphertext.clone()));
        if let Some(signature) = &self.signature {
            map.insert(5, Value::Bytes(signature.clone()));
        }
        for (key, value) in &self.extensions {
            map.insert(*key, value.clone());
        }
        Value::Map(map.into_iter().map(|(k, v)| (key_to_cbor(k), v)).collect())
    }
}

/// Reads a CBOR byte string for an Envelope field.
fn value_as_bytes<E: de::Error>(value: Value, field: &str) -> Result<Vec<u8>, E> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(de::Error::custom(format!(
            "invalid {field} field (expected bytes)"
        ))),
    }
}

/// Reads a CBOR integer for an Envelope field and converts it to u32.
fn value_as_u32<E: de::Error>(value: Value, field: &str) -> Result<u32, E> {
    let raw = value_as_u64(value, field)?;
    u32::try_from(raw).map_err(|_| de::Error::custom(format!("invalid {field} field range")))
}

/// Reads a CBOR integer for an Envelope field and converts it to u64.
fn value_as_u64<E: de::Error>(value: Value, field: &str) -> Result<u64, E> {
    match value {
        Value::Integer(value) => {
            let n = i128::from(value);
            if n >= 0 {
                u64::try_from(n)
                    .map_err(|_| de::Error::custom(format!("invalid {field} field range")))
            } else {
                Err(de::Error::custom(format!(
                    "invalid {field} field (expected unsigned integer)"
                )))
            }
        }
        _ => Err(de::Error::custom(format!(
            "invalid {field} field (expected unsigned integer)"
        ))),
    }
}

/// Convenience for parsing fixed-size hex inputs in tests.
pub fn to_fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], SpexError> {
    if bytes.len() != N {
        return Err(SpexError::InvalidLength("fixed array"));
    }
    let mut arr = [0u8; N];
    arr.copy_from_slice(bytes);
    Ok(arr)
}
