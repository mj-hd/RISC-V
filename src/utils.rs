pub trait ApplyByte {
    fn apply_byte(src: Self, val: u8, offset: usize) -> Self;
}

impl ApplyByte for u32 {
    fn apply_byte(src: u32, val: u8, offset: usize) -> u32 {
        (src & (!(((!0u8) as u32) << offset * 8))) | ((val as u32) << offset * 8)
    }
}

impl ApplyByte for u64 {
    fn apply_byte(src: u64, val: u8, offset: usize) -> u64 {
        (src & (!(((!0u8) as u64) << offset * 8))) | ((val as u64) << offset * 8)
    }
}
