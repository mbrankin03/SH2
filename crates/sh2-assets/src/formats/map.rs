//! Map format (.map) parser stub.

/// Map binary header format matching original PS2/PC map files.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MapHeader {
    pub chunk_count: u32,
}
