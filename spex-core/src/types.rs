use crate::{cbor, error::SpexError};
use serde_cbor::Value;
use std::collections::BTreeMap;

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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
        let mut map = BTreeMap::new();
        map.insert(Value::Integer(0), Value::Bytes(self.user_id.clone()));
        map.insert(Value::Integer(1), Value::Bytes(self.verifying_key.clone()));
        map.insert(Value::Integer(2), Value::Bytes(self.device_id.clone()));
        map.insert(Value::Integer(3), Value::Bytes(self.device_nonce.clone()));
        map.insert(Value::Integer(4), Value::Integer(self.issued_at as i128));
        if let Some(invite) = &self.invite {
            map.insert(Value::Integer(5), invite.to_cbor_value());
        }
        if let Some(signature) = &self.signature {
            map.insert(Value::Integer(6), Value::Bytes(signature.clone()));
        }
        for (key, value) in &self.extensions {
            map.insert(Value::Integer(*key as i128), value.clone());
        }
        Value::Map(map.into_iter().collect())
    }
}

/// Represents an invite token (often embedded in cards or bridge messages).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InviteToken {
    pub major: u16,
    pub minor: u16,
    pub requires_puzzle: bool,
    pub extensions: BTreeMap<u64, Value>,
}

impl Ctap2Cbor for InviteToken {
    fn to_cbor_value(&self) -> Value {
        let mut map = BTreeMap::new();
        map.insert(Value::Integer(0), Value::Integer(self.major as i128));
        map.insert(Value::Integer(1), Value::Integer(self.minor as i128));
        map.insert(Value::Integer(2), Value::Bool(self.requires_puzzle));
        for (key, value) in &self.extensions {
            map.insert(Value::Integer(*key as i128), value.clone());
        }
        Value::Map(map.into_iter().collect())
    }
}

/// Represents a grant token (membership/permission entry).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GrantToken {
    pub user_id: Vec<u8>,
    pub role: u64,
    pub flags: Option<u64>,
    pub expires_at: Option<u64>,
    pub extensions: BTreeMap<u64, Value>,
}

impl Ctap2Cbor for GrantToken {
    fn to_cbor_value(&self) -> Value {
        let mut map = BTreeMap::new();
        map.insert(Value::Integer(0), Value::Bytes(self.user_id.clone()));
        map.insert(Value::Integer(1), Value::Integer(self.role as i128));
        if let Some(flags) = self.flags {
            map.insert(Value::Integer(2), Value::Integer(flags as i128));
        }
        if let Some(expires_at) = self.expires_at {
            map.insert(Value::Integer(3), Value::Integer(expires_at as i128));
        }
        for (key, value) in &self.extensions {
            map.insert(Value::Integer(*key as i128), value.clone());
        }
        Value::Map(map.into_iter().collect())
    }
}

/// Represents the per-thread configuration blob.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
        let mut map = BTreeMap::new();
        map.insert(Value::Integer(0), Value::Integer(self.proto_major as i128));
        map.insert(Value::Integer(1), Value::Integer(self.proto_minor as i128));
        map.insert(Value::Integer(2), Value::Integer(self.ciphersuite_id as i128));
        map.insert(Value::Integer(3), Value::Integer(self.flags as i128));
        map.insert(Value::Integer(4), Value::Bytes(self.thread_id.clone()));
        let grants = self
            .grants
            .iter()
            .map(|grant| grant.to_cbor_value())
            .collect();
        map.insert(Value::Integer(5), Value::Array(grants));
        for (key, value) in &self.extensions {
            map.insert(Value::Integer(*key as i128), value.clone());
        }
        Value::Map(map.into_iter().collect())
    }
}

/// Represents a sealed message envelope.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Envelope {
    pub thread_id: Vec<u8>,
    pub epoch: u32,
    pub seq: u64,
    pub sender_user_id: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub signature: Option<Vec<u8>>,
    pub extensions: BTreeMap<u64, Value>,
}

impl Ctap2Cbor for Envelope {
    fn to_cbor_value(&self) -> Value {
        let mut map = BTreeMap::new();
        map.insert(Value::Integer(0), Value::Bytes(self.thread_id.clone()));
        map.insert(Value::Integer(1), Value::Integer(self.epoch as i128));
        map.insert(Value::Integer(2), Value::Integer(self.seq as i128));
        map.insert(Value::Integer(3), Value::Bytes(self.sender_user_id.clone()));
        map.insert(Value::Integer(4), Value::Bytes(self.ciphertext.clone()));
        if let Some(signature) = &self.signature {
            map.insert(Value::Integer(5), Value::Bytes(signature.clone()));
        }
        for (key, value) in &self.extensions {
            map.insert(Value::Integer(*key as i128), value.clone());
        }
        Value::Map(map.into_iter().collect())
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
