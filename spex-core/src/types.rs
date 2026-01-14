use crate::error::SpexError;

#[derive(Clone, Copy, Debug)]
pub struct ProtoSuite {
    pub major: u16,
    pub minor: u16,
    pub ciphersuite_id: u16,
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
