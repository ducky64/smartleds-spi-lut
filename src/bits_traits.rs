pub trait NumBits {
    const BITS: u8;
}

impl NumBits for u8 { const BITS: u8 = 8; }
impl NumBits for u16 { const BITS: u8 = 16; }
impl NumBits for u32 { const BITS: u8 = 32; }
impl NumBits for u64 { const BITS: u8 = 64; }

pub trait TruncatedFrom<T> {
    fn truncated_from(value: T) -> Self;
}

impl<T> TruncatedFrom<T> for T {
    fn truncated_from(value: T) -> Self {
        value
    }
}

impl TruncatedFrom<u32> for u8 {
    fn truncated_from(value: u32) -> u8 {
        value as u8
    }
}

impl TruncatedFrom<u16> for u8 {
    fn truncated_from(value: u16) -> u8 {
        value as u8
    }
}

impl TruncatedFrom<u32> for u16 {
    fn truncated_from(value: u32) -> u16 {
        value as u16
    }
}
