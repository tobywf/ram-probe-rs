use std::fmt;

#[repr(transparent)]
pub struct HexU32(pub u32);

impl fmt::Debug for HexU32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}

struct SegmentDebug<'a, 'data>(&'a (u64, &'data [u8]));

impl fmt::Debug for SegmentDebug<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08x} ({} bytes)", self.0 .0, self.0 .1.len())
    }
}

#[derive(Clone)]
pub struct Segments<'data>(pub Vec<(u64, &'data [u8])>);

impl<'data> Segments<'data> {
    pub fn iter(&self) -> std::slice::Iter<'_, (u64, &[u8])> {
        self.0.iter()
    }
}

impl fmt::Debug for Segments<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries = self.0.iter().map(SegmentDebug);
        f.debug_list().entries(entries).finish()
    }
}

#[derive(Clone)]
pub struct VectorTable {
    /// The address of the vector table.
    pub address: u32,
    /// The initial stack pointer address.
    pub initial_sp: u32,
    /// The reset handler address.
    pub reset: u32,
    /// The hard fault handler address.
    pub hard_fault: u32,
}

impl fmt::Debug for VectorTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VectorTable")
            .field("address", &HexU32(self.address))
            .field("initial_sp", &HexU32(self.initial_sp))
            .field("reset", &HexU32(self.reset))
            .field("hard_fault", &HexU32(self.hard_fault))
            .finish()
    }
}
