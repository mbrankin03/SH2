//! Transient dynamic stage obstacles (`clAddDynamicWall` / `clAddDynamicFloor`).
//!
//! Stages in Silent Hill 2 register transient dynamic collision polygons:
//! - Moving doors that swing or slide.
//! - Elevators and rising/lowering platforms.
//! - Falling cages and puzzle blocks.
//!
//! These are registered per-stage or per-frame and checked during character movement,
//! weapon hit scans, and camera raycasting.

use sh2_assets::formats::cld::{FloorTriangle, WallSegment};

/// Dynamic stage wall obstacle added by gameplay triggers or puzzle scripts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicWall {
    pub segment: WallSegment,
    pub active: bool,
}

impl DynamicWall {
    /// Construct a dynamic wall segment between two 2D endpoints in world $XZ$.
    pub const fn new(
        x0: f32,
        z0: f32,
        x1: f32,
        z1: f32,
        y_top: f32,
        y_bottom: f32,
        material: u32,
    ) -> Self {
        Self {
            segment: WallSegment::new([x0, z0], [x1, z1], y_top, y_bottom, 1, material),
            active: true,
        }
    }
}

/// Dynamic stage floor obstacle (e.g. elevator platform, trap door).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicFloor {
    pub triangle: FloorTriangle,
    pub active: bool,
}

impl DynamicFloor {
    /// Construct a dynamic floor triangle.
    pub const fn new(v: [[f32; 3]; 3], material: u32) -> Self {
        Self {
            triangle: FloorTriangle::new(v, material),
            active: true,
        }
    }

    /// Construct a horizontal rectangular dynamic floor quad at height `y`.
    pub fn new_quad(
        x_min: f32,
        x_max: f32,
        z_min: f32,
        z_max: f32,
        y: f32,
        material: u32,
    ) -> (Self, Self) {
        let t1 = FloorTriangle::new(
            [[x_min, y, z_min], [x_max, y, z_min], [x_max, y, z_max]],
            material,
        );
        let t2 = FloorTriangle::new(
            [[x_min, y, z_min], [x_max, y, z_max], [x_min, y, z_max]],
            material,
        );
        (
            Self {
                triangle: t1,
                active: true,
            },
            Self {
                triangle: t2,
                active: true,
            },
        )
    }
}
