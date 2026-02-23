/// Converts a u16 value to its big-endian byte representation.
pub fn u16be(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

/// Converts a u32 value to its big-endian byte representation.
pub fn u32be(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

/// Converts a u64 value to its big-endian byte representation.
pub fn u64be(v: u64) -> [u8; 8] {
    v.to_be_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u16be() {
        assert_eq!(u16be(0x1234), [0x12, 0x34]);
    }

    #[test]
    fn test_u32be() {
        assert_eq!(u32be(0x12345678), [0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_u64be() {
        assert_eq!(
            u64be(0x1234567890ABCDEF),
            [0x12, 0x34, 0x56, 0x78, 0x90, 0xAB, 0xCD, 0xEF]
        );
    }
}
