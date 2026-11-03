//! Model format (.mdls) parser stub.

/// Model binary header format matching original PS2/PC 3D model files.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModelHeader {
    pub mesh_count: u32,
}
