//! Dynamic map companion system supporting James's maps (1-35) and Maria's Baldwin Mansion map (36).
//!
//! # Map Architecture
//! - Top screen: Primary 3D horror gameplay.
//! - Bottom screen: Real-time map displaying current floor, room bounds, player marker,
//!   and circle/cross annotation decals extracted from `data/pic/map/*.tex`.

/// Identifying descriptor for active level map and floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapDescriptor {
    /// Numerical map identifier (1 = Town East, 2 = Apartments, etc., 36 = Baldwin Mansion).
    pub map_id: u8,
    /// Current building floor (-1 = Basement, 1 = 1F, 2 = 2F, 3 = 3F, 4 = Roof).
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
