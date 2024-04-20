/// Thumb v7 is a 16-bit architecture.
macro_rules! thumb_v7_align {
    ($value:expr) => {
        (($value) & !1)
    };
}
pub(crate) use thumb_v7_align;

/// Vector Table Offset Register.
pub(crate) const VTOR: u64 = 0xE000ED08;

/// Assembly for BKPT #0 on Thumb v7 (16-bit).
pub(crate) const BKPT_ASM: &[u8; 2] = &[0x00, 0xbe];
