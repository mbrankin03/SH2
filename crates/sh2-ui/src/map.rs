//! Dynamic map companion system supporting James's maps (1-35) and Maria's Baldwin Mansion map (36).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapDescriptor {
    pub map_id: u8,
    pub floor: i8,
}

impl Default for MapDescriptor {
    fn default() -> Self {
        Self {
            map_id: 1, // Town East
            floor: 1,
        }
    }
}
