pub trait NumBits {
    const BITS: u8;
}

impl NumBits for u8 { const BITS: u8 = 8; }
impl NumBits for u16 { const BITS: u8 = 16; }
impl NumBits for u32 { const BITS: u8 = 32; }
impl NumBits for u64 { const BITS: u8 = 64; }

pub trait WordOps {
    fn to_u32(self) -> u32;
    fn truncate_from_u32(value: u32) -> Self;
}

impl WordOps for u8 {
    fn to_u32(self) -> u32 {
        self as u32
    }

    fn truncate_from_u32(value: u32) -> Self {
        value as u8
    }
}

impl WordOps for u16 {
    fn to_u32(self) -> u32 {
        self as u32
    }

    fn truncate_from_u32(value: u32) -> Self {
        value as u16
    }
}

impl WordOps for u32 {
    fn to_u32(self) -> u32 {
        self as u32
    }

    fn truncate_from_u32(value: u32) -> Self {
        value
    }
}
