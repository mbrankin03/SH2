//! Centralized CollisionController and CollisionWorld subsystem (`cl_main.c` / `cl_calc.c`).
//!
//! Provides the primary simulation interface for:
//! - Static `.cld` background geometry and spatial broadphase caching (`SpatialGrid`).
//! - Dynamic extra walls/floors (`clAddDynamicWall`, `clAddDynamicFloor`).
//! - Character cylinder management and separation (`clCheckColumn2ColumnHit`).
//! - Wall pushback relaxation and surface sliding (`clCheckBg2Chara`).
//! - Floor height sampling and stair climbing (`clCollectCharaHeightNormal`).
//! - Camera ray clamping against room barriers (`clCheckHitEyes`).
//! - Safe spawn snapping (`col_spawn`).
//! - Room / block collision merging (`col_merge`).

use glam::Vec3;
use sh2_assets::formats::cld::{CldFile, FloorTriangle, WallSegment};

use super::character::{CharacterCylinder, CharacterId};
use super::dynamic::{DynamicFloor, DynamicWall};
use super::grid::SpatialGrid;
use super::raycast::{clamp_camera_ray, RaycastFilter};

/// Surface material category extracted from `.cld` / `.sfm` floor collision triangles (`FloorTriangle::material`),
/// driving authentic material-dependent footstep sound selection (`0x5147E0` / `sddata.bin`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SurfaceMaterial {
    /// Asphalt, concrete, stone, and tile surfaces (`sddata[146, 151, 280]`).
    #[default]
    Concrete = 0,
    /// Wooden planks, floorboards, and stairs (`sddata[148, 150, 145]`).
    Wood = 1,
    /// Metal grating, iron stairs, and boiler plates (`sddata[147, 141, 143]`).
    Metal = 2,
    /// Carpet, rugs, and padded hospital/hotel hallways (`sddata[142, 130]`).
    Carpet = 3,
    /// Shallow water puddles, flooded basement, and sewers (`sddata[149, 83, 84, 371]`).
    Water = 4,
    /// Forest dirt, grass, and foliage trails (`sddata[144, 131]`).
    Grass = 5,
    /// Gravel, loose rubble, and cemetery paths (`sddata[132, 144]`).
    Gravel = 6,
    /// Viscous blood / otherworld organic surfaces (`sddata[95, 367]`).
    Blood = 7,
}

impl SurfaceMaterial {
    /// Total number of distinct surface material categories.
    pub const COUNT: usize = 8;

    /// Ordered list of all surface material categories.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Concrete,
        Self::Wood,
        Self::Metal,
        Self::Carpet,
        Self::Water,
        Self::Grass,
        Self::Gravel,
        Self::Blood,
    ];

    /// Maps a raw `.cld` `FloorTriangle::material` attribute (`u32`) to a [`SurfaceMaterial`].
    ///
    /// In `.cld` floor records (`FloorTriangle`), the low bits encode the surface acoustic attribute.
    #[inline]
    pub const fn from_cld_material(raw: u32) -> Self {
        match raw & 0x07 {
            0 => Self::Concrete,
            1 => Self::Wood,
            2 => Self::Metal,
            3 => Self::Carpet,
            4 => Self::Water,
            5 => Self::Grass,
            6 => Self::Gravel,
            _ => Self::Blood,
        }
    }

    /// Cycles to the next surface material (useful for interactive audio debug testing).
    #[inline]
    pub const fn next(self) -> Self {
        Self::from_cld_material((self as u32) + 1)
    }

    /// Human-readable label for debug HUD display.
    #[inline]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Concrete => "Concrete / Stone",
            Self::Wood => "Wood / Planks",
            Self::Metal => "Metal Grating",
            Self::Carpet => "Carpet / Rug",
            Self::Water => "Water / Puddle",
            Self::Grass => "Grass / Forest Dirt",
            Self::Gravel => "Gravel / Rubble",
            Self::Blood => "Blood / Organic",
        }
    }
}

/// Result of sampling the walkable floor beneath a character at $(X, Z)$, including both height and surface material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloorSample {
    /// Walkable floor height $Y$ in world units ($+Y$ points downward).
    pub height_y: f32,
    /// Raw `.cld` floor triangle material attribute (`FloorTriangle::material`).
    pub raw_material: u32,
    /// Classified acoustic surface material for footstep SFX dispatch.
    pub surface: SurfaceMaterial,
}

/// Configuration constants for character collision and kinematics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionConfig {
    /// Character cylinder collision radius in world units (mm).
    /// James/Maria collision radius is ~250.0 - 260.0 mm.
    pub radius: f32,
    /// Character cylinder height from feet to head in world units (~1800.0 mm).
    pub height: f32,
    /// Maximum vertical climb/step-up threshold (e.g. stair riser limit, ~250 mm).
    pub max_step_up: f32,
    /// Maximum vertical drop/step-down threshold (~350 mm).
    pub max_step_down: f32,
    /// Number of iterative wall pushback relaxation iterations.
    pub max_iterations: usize,
}

impl Default for CollisionConfig {
    fn default() -> Self {
        Self {
            radius: 260.0,
            height: 1800.0,
            max_step_up: 250.0,
            max_step_down: 350.0,
            max_iterations: 3,
        }
    }
}

/// Static and dynamic collision state representing an active map room or combined streaming sectors.
#[derive(Debug, Clone, Default)]
pub struct CollisionWorld {
    /// Room origin coordinates $(X, Z)$.
    pub origin: [f32; 2],
    /// Floor triangles for height sampling and stepping.
    pub floors: Vec<FloorTriangle>,
    /// Wall segments for lateral cylinder containment and sliding.
    pub walls: Vec<WallSegment>,
    /// Extra transient or stage-added wall segments (`s_extra`).
    pub extra_walls: Vec<WallSegment>,
    /// Dynamic stage wall obstacles.
    pub dynamic_walls: Vec<DynamicWall>,
    /// Dynamic stage floor obstacles.
    pub dynamic_floors: Vec<DynamicFloor>,
    /// 4x4 spatial acceleration broadphase grid.
    pub grid: SpatialGrid,
}

impl CollisionWorld {
    /// Create an empty collision world.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load collision geometry from a parsed `.cld` file.
    pub fn from_cld(cld: &CldFile) -> Self {
        let grid = SpatialGrid::from_cld(cld);
        Self {
            origin: cld.origin,
            floors: cld.floors.clone(),
            walls: cld.walls.clone(),
            extra_walls: Vec::new(),
            dynamic_walls: Vec::new(),
            dynamic_floors: Vec::new(),
            grid,
        }
    }

    /// Construct a procedural room collision environment matching the fallback boiler room.
    pub fn create_synthetic_room(w: f32, l: f32, h: f32, pw: f32, pl: f32) -> Self {
        let cld = CldFile::create_synthetic_room(w, l, h, pw, pl);
        Self::from_cld(&cld)
    }

    /// Clear extra dynamic wall segments (`col_extra_clear`).
    pub fn clear_extra_walls(&mut self) {
        self.extra_walls.clear();
        self.dynamic_walls.clear();
        self.dynamic_floors.clear();
    }

    /// Add an extra transient wall segment (`col_extra_wall`).
    pub fn add_extra_wall(&mut self, wall: WallSegment) {
        self.extra_walls.push(wall);
    }

    /// Add a dynamic stage wall obstacle (`clAddDynamicWall`).
    pub fn add_dynamic_wall(&mut self, wall: DynamicWall) {
        self.dynamic_walls.push(wall);
    }

    /// Add a dynamic stage floor obstacle (`clAddDynamicFloor`).
    pub fn add_dynamic_floor(&mut self, floor: DynamicFloor) {
        self.dynamic_floors.push(floor);
    }

    /// Iterate over all active walls (static + extra + dynamic).
    pub fn all_walls(&self) -> impl Iterator<Item = &WallSegment> {
        self.walls.iter().chain(self.extra_walls.iter()).chain(
            self.dynamic_walls
                .iter()
                .filter(|dw| dw.active)
                .map(|dw| &dw.segment),
        )
    }

    /// Merge multiple collision worlds into a single combined environment (`col_merge`).
    pub fn merge(parts: &[&CollisionWorld]) -> Self {
        let mut merged = Self::new();
        for p in parts {
            merged.floors.extend_from_slice(&p.floors);
            merged.walls.extend_from_slice(&p.walls);
            merged.extra_walls.extend_from_slice(&p.extra_walls);
            merged.dynamic_walls.extend_from_slice(&p.dynamic_walls);
            merged.dynamic_floors.extend_from_slice(&p.dynamic_floors);
        }

        // Rebuild spatial grid for merged geometry
        let cld = CldFile {
            origin: [0.0, 0.0],
            floors: merged.floors.clone(),
            walls: merged.walls.clone(),
            b0_cells: Default::default(),
            b1_cells: Default::default(),
        };
        merged.grid = SpatialGrid::from_cld(&cld);
        merged
    }

    /// Sample the walkable floor height and surface material at $(x, z)$ closest to `y_ref` within step boundaries.
    ///
    /// Authentic PS2 `clCollectCharaHeightNormal`:
    /// Because world $+Y$ points downward:
    /// - `up` is above feet: $y \ge y_{\text{ref}} - \text{up}$
    /// - `down` is below feet: $y \le y_{\text{ref}} + \text{down}$
    ///
    /// Returns the highest floor (minimum numeric $Y$) within range along with its `FloorTriangle::material`.
    pub fn sample_floor_surface(
        &self,
        x: f32,
        z: f32,
        y_ref: f32,
        up: f32,
        down: f32,
    ) -> Option<FloorSample> {
        let mut best: Option<FloorSample> = None;

        // 1. Broadphase: query floor triangles in the current grid cell
        let cell = self.grid.point_to_cell(x, z);
        let cell_tri_indices = &self.grid.floor_cells[cell];

        if !cell_tri_indices.is_empty() {
            for &idx in cell_tri_indices {
                if let Some(tri) = self.floors.get(idx) {
                    if let Some(y) = tri.sample_height(x, z) {
                        if y >= y_ref - up && y <= y_ref + down {
                            match best {
                                None => {
                                    best = Some(FloorSample {
                                        height_y: y,
                                        raw_material: tri.material,
                                        surface: SurfaceMaterial::from_cld_material(tri.material),
                                    });
                                }
                                Some(prev) if y < prev.height_y => {
                                    best = Some(FloorSample {
                                        height_y: y,
                                        raw_material: tri.material,
                                        surface: SurfaceMaterial::from_cld_material(tri.material),
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        } else {
            // Broadphase cell empty or unindexed: fallback to all static floors
            for tri in &self.floors {
                if let Some(y) = tri.sample_height(x, z) {
                    if y >= y_ref - up && y <= y_ref + down {
                        match best {
                            None => {
                                best = Some(FloorSample {
                                    height_y: y,
                                    raw_material: tri.material,
                                    surface: SurfaceMaterial::from_cld_material(tri.material),
                                });
                            }
                            Some(prev) if y < prev.height_y => {
                                best = Some(FloorSample {
                                    height_y: y,
                                    raw_material: tri.material,
                                    surface: SurfaceMaterial::from_cld_material(tri.material),
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // 2. Check dynamic floor obstacles
        for df in &self.dynamic_floors {
            if !df.active {
                continue;
            }
            if let Some(y) = df.triangle.sample_height(x, z) {
                if y >= y_ref - up && y <= y_ref + down {
                    match best {
                        None => {
                            best = Some(FloorSample {
                                height_y: y,
                                raw_material: df.triangle.material,
                                surface: SurfaceMaterial::from_cld_material(df.triangle.material),
                            });
                        }
                        Some(prev) if y < prev.height_y => {
                            best = Some(FloorSample {
                                height_y: y,
                                raw_material: df.triangle.material,
                                surface: SurfaceMaterial::from_cld_material(df.triangle.material),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        best
    }

    /// Sample the walkable floor height at $(x, z)$ closest to `y_ref` within step boundaries.
    ///
    /// Authentic PS2 `clCollectCharaHeightNormal`:
    /// Because world $+Y$ points downward:
    /// - `up` is above feet: $y \ge y_{\text{ref}} - \text{up}$
    /// - `down` is below feet: $y \le y_{\text{ref}} + \text{down}$
    ///
    /// Returns the highest floor (minimum numeric $Y$) within range.
    pub fn sample_floor_height(
        &self,
        x: f32,
        z: f32,
        y_ref: f32,
        up: f32,
        down: f32,
    ) -> Option<f32> {
        self.sample_floor_surface(x, z, y_ref, up, down)
            .map(|s| s.height_y)
    }

    /// Resolve lateral wall collisions for a vertical cylinder with radius `config.radius`.
    ///
    /// Authentic 1:1 PS2 `clCheckBg2Chara` / `col_push`:
    /// Iteratively projects the cylinder center onto wall segments in $(X, Z)$,
    /// calculating the closest point $t \in [0, 1]$ and sliding/pushing along the normal
    /// if the distance is less than the radius.
    pub fn resolve_wall_pushback(&self, pos: &mut Vec3, config: &CollisionConfig) -> bool {
        let mut pushed = false;
        let y = pos.y;
        let h = config.height;
        let radius = config.radius;
        let r_sq = radius * radius;

        for _iter in 0..config.max_iterations {
            let mut moved = false;

            for w in self.all_walls() {
                // Group 2 is ceiling / hanging slab geometry; does not push characters
                if w.group == 2 || !w.overlaps_vertical(y, h) {
                    continue;
                }

                let dx = w.end[0] - w.start[0];
                let dz = w.end[1] - w.start[1];
                let len_sq = dx * dx + dz * dz;
                if len_sq < 1e-3 {
                    continue;
                }

                // Project cylinder center (pos.x, pos.z) onto segment start..end
                let t = (((pos.x - w.start[0]) * dx + (pos.z - w.start[1]) * dz) / len_sq)
                    .clamp(0.0, 1.0);

                let px = w.start[0] + dx * t;
                let pz = w.start[1] + dz * t;

                let ox = pos.x - px;
                let oz = pos.z - pz;
                let dist_sq = ox * ox + oz * oz;

                if dist_sq < r_sq {
                    let d = dist_sq.sqrt();
                    let (push_nx, push_nz) = if d < 1e-3 {
                        // Exactly on segment: push along segment normal (-dz, dx)
                        let l = len_sq.sqrt();
                        (-dz / l, dx / l)
                    } else {
                        (ox / d, oz / d)
                    };

                    let pen = radius - d;
                    pos.x += push_nx * pen;
                    pos.z += push_nz * pen;
                    moved = true;
                    pushed = true;
                }
            }

            if !moved {
                break;
            }
        }

        pushed
    }

    /// Full kinematic collision update for character position:
    /// 1. Raycast continuous collision prevention against tunneling through walls.
    /// 2. Lateral wall pushback and sliding along surface normals.
    /// 3. Vertical floor height snapping and step traversal.
    pub fn resolve_character_movement(
        &self,
        current_pos: Vec3,
        desired_pos: Vec3,
        config: &CollisionConfig,
    ) -> Vec3 {
        let mut pos = desired_pos;

        // 1. Raycast line segment intersection to prevent tunneling through thin/corner walls
        let y = current_pos.y;
        let h = config.height;
        let move_dx = desired_pos.x - current_pos.x;
        let move_dz = desired_pos.z - current_pos.z;
        let move_dist_sq = move_dx * move_dx + move_dz * move_dz;

        if move_dist_sq > 1e-4 {
            let mut earliest_hit: Option<(f32, f32, f32)> = None;

            for w in self.all_walls() {
                if w.group == 2 || !w.overlaps_vertical(y, h) {
                    continue;
                }

                let rx = desired_pos.x - current_pos.x;
                let rz = desired_pos.z - current_pos.z;
                let sx = w.end[0] - w.start[0];
                let sz = w.end[1] - w.start[1];

                let den = rx * sz - rz * sx;
                if den.abs() < 1e-6 {
                    continue;
                }

                let t =
                    ((w.start[0] - current_pos.x) * sz - (w.start[1] - current_pos.z) * sx) / den;
                let u =
                    ((w.start[0] - current_pos.x) * rz - (w.start[1] - current_pos.z) * rx) / den;

                if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                    let hit_x = current_pos.x + rx * t;
                    let hit_z = current_pos.z + rz * t;
                    match earliest_hit {
                        None => earliest_hit = Some((t, hit_x, hit_z)),
                        Some((prev_t, _, _)) if t < prev_t => {
                            earliest_hit = Some((t, hit_x, hit_z));
                        }
                        _ => {}
                    }
                }
            }

            if let Some((_hit_t, hit_x, hit_z)) = earliest_hit {
                // Stop short of the wall along movement direction by character radius
                let move_len = move_dist_sq.sqrt();
                let dir_x = move_dx / move_len;
                let dir_z = move_dz / move_len;

                pos.x = hit_x - dir_x * config.radius;
                pos.z = hit_z - dir_z * config.radius;
            }
        }

        // 2. Resolve lateral wall pushback and continuous sliding
        self.resolve_wall_pushback(&mut pos, config);

        // 3. Sample floor height
        if let Some(floor_y) = self.sample_floor_height(
            pos.x,
            pos.z,
            current_pos.y,
            config.max_step_up,
            config.max_step_down,
        ) {
            pos.y = floor_y;
        }

        pos
    }

    /// Clamp camera position against room walls (`clCheckHitEyes` / `col_clamp_ray`).
    pub fn clamp_camera(&self, target: Vec3, camera_pos: Vec3, filter: &RaycastFilter) -> Vec3 {
        clamp_camera_ray(target, camera_pos, self.all_walls(), filter)
    }

    /// Find safe walkable spawn point closest to $(x, z)$ snapped to nearest floor centroid (`col_spawn`).
    pub fn find_safe_spawn_point(&self, x: f32, z: f32) -> Option<Vec3> {
        let mut best_dist_sq = f32::INFINITY;
        let mut best_spawn: Option<Vec3> = None;

        for f in &self.floors {
            let cx = (f.v[0][0] + f.v[1][0] + f.v[2][0]) / 3.0;
            let cy = (f.v[0][1] + f.v[1][1] + f.v[2][1]) / 3.0;
            let cz = (f.v[0][2] + f.v[1][2] + f.v[2][2]) / 3.0;

            let dx = cx - x;
            let dz = cz - z;
            let dist_sq = dx * dx + dz * dz;

            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best_spawn = Some(Vec3::new(cx, cy, cz));
            }
        }

        best_spawn
    }
}

/// Unified CollisionController subsystem managing frame lifecycle and multi-actor separation (`cl_main.c`).
#[derive(Debug, Clone, Default)]
pub struct CollisionController {
    /// Active collision environment.
    pub world: CollisionWorld,
    /// Registered character collision cylinders (James, Maria, enemies).
    pub characters: Vec<CharacterCylinder>,
    /// Global collision config.
    pub config: CollisionConfig,
}

impl CollisionController {
    /// Initialize collision controller (`clAllInitCollisionData`).
    pub fn new(world: CollisionWorld, config: CollisionConfig) -> Self {
        Self {
            world,
            characters: Vec::new(),
            config,
        }
    }

    /// Per-frame lifecycle initialization (`clFrameInitCollisionData`).
    pub fn frame_init(&mut self) {
        // Clear transient extra walls added for single-frame events
        self.world.extra_walls.clear();
    }

    /// Register or update a character cylinder (`clSetCharaHitColumn`).
    pub fn register_character(&mut self, chara: CharacterCylinder) {
        if let Some(existing) = self.characters.iter_mut().find(|c| c.id == chara.id) {
            *existing = chara;
        } else {
            self.characters.push(chara);
        }
    }

    /// Remove a character cylinder.
    pub fn unregister_character(&mut self, id: CharacterId) {
        self.characters.retain(|c| c.id != id);
    }

    /// Resolve character-to-character separation pushes across all active cylinders (`clCheckColumn2ColumnHit`).
    pub fn resolve_character_separations(&mut self) {
        let len = self.characters.len();
        if len < 2 {
            return;
        }

        for i in 0..len {
            for j in (i + 1)..len {
                let char_a = self.characters[i];
                let char_b = self.characters[j];

                let (delta_a, delta_b) = char_a.resolve_separation(&char_b);
                if delta_a != Vec3::ZERO {
                    self.characters[i].position += delta_a;
                }
                if delta_b != Vec3::ZERO {
                    self.characters[j].position += delta_b;
                }
            }
        }
    }

    /// Update a character's position through full environment collision and separation.
    pub fn update_character_kinematics(&mut self, id: CharacterId, desired_pos: Vec3) -> Vec3 {
        let (current_pos, config) = if let Some(c) = self.characters.iter().find(|c| c.id == id) {
            let mut cfg = self.config;
            cfg.radius = c.radius;
            cfg.height = c.height;
            (c.position, cfg)
        } else {
            (desired_pos, self.config)
        };

        // 1. Resolve background world geometry collision
        let resolved_pos = self
            .world
            .resolve_character_movement(current_pos, desired_pos, &config);

        // Update stored cylinder position
        if let Some(c) = self.characters.iter_mut().find(|c| c.id == id) {
            c.position = resolved_pos;
        }

        // 2. Resolve actor-to-actor separations
        self.resolve_character_separations();

        // Return updated position
        self.characters
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.position)
            .unwrap_or(resolved_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wall_pushback_and_sliding() {
        // Enclosed square room 2000x2000 (X: -1000..1000, Z: -1000..1000)
        let world = CollisionWorld::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
        let config = CollisionConfig::default(); // radius = 260.0

        // Move player towards east wall (X = 1000.0) beyond the boundary
        let current_pos = Vec3::new(900.0, 0.0, 0.0);
        let desired_pos = Vec3::new(980.0, 0.0, 100.0); // should be pushed back to X <= 1000 - 260 = 740

        let final_pos = world.resolve_character_movement(current_pos, desired_pos, &config);

        assert!(
            final_pos.x <= 740.01,
            "Player X ({}) must be clamped by wall radius boundary 740.0",
            final_pos.x
        );
        // Z motion should slide freely
        assert_eq!(final_pos.z, 100.0);
        assert_eq!(final_pos.y, 0.0);
    }

    #[test]
    fn test_center_pillar_collision() {
        // 4000x4000 room with center pillar 600x600 (-300..300, -300..300)
        let world = CollisionWorld::create_synthetic_room(4000.0, 4000.0, 3000.0, 600.0, 600.0);
        let config = CollisionConfig::default(); // radius = 260.0

        // Try walking into the pillar from the west
        let current_pos = Vec3::new(-500.0, 0.0, 0.0);
        let desired_pos = Vec3::new(-200.0, 0.0, 0.0); // Inside the pillar's west wall (-300)

        let final_pos = world.resolve_character_movement(current_pos, desired_pos, &config);

        // West wall of pillar is at X = -300, so cylinder center should be pushed to <= -300 - 260 = -560
        assert!(
            final_pos.x <= -559.9,
            "Player should be pushed outside pillar boundary: pos.x={}",
            final_pos.x
        );
    }

    #[test]
    fn test_stair_stepping_and_gravity() {
        let mut world = CollisionWorld::new();
        // Lower floor at Y = 0.0, step at Y = -150.0 (higher in Y-down space)
        world.floors.push(FloorTriangle::new(
            [[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [1000.0, 0.0, 1000.0]],
            0,
        ));
        world.floors.push(FloorTriangle::new(
            [[0.0, 0.0, 0.0], [1000.0, 0.0, 1000.0], [0.0, 0.0, 1000.0]],
            0,
        ));
        // Raised platform
        world.floors.push(FloorTriangle::new(
            [
                [1000.0, -150.0, 0.0],
                [2000.0, -150.0, 0.0],
                [2000.0, -150.0, 1000.0],
            ],
            0,
        ));
        world.floors.push(FloorTriangle::new(
            [
                [1000.0, -150.0, 0.0],
                [2000.0, -150.0, 1000.0],
                [1000.0, -150.0, 1000.0],
            ],
            0,
        ));

        let config = CollisionConfig::default(); // max_step_up = 250.0

        // Step from lower floor onto raised platform
        let current_pos = Vec3::new(900.0, 0.0, 500.0);
        let desired_pos = Vec3::new(1100.0, 0.0, 500.0);

        let final_pos = world.resolve_character_movement(current_pos, desired_pos, &config);
        assert_eq!(
            final_pos.y, -150.0,
            "Player must step up onto platform at -150.0"
        );
    }

    #[test]
    fn test_camera_wall_clamping_cl_check_hit_eyes() {
        let world = CollisionWorld::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
        let filter = RaycastFilter::default();

        // Player is at (0, 0, 0). Camera tries to be outside north wall (Z = -1200, wall is at Z = -1000)
        let player_eye = Vec3::new(0.0, -1200.0, 0.0);
        let desired_camera = Vec3::new(0.0, -1200.0, -1500.0);

        let clamped = world.clamp_camera(player_eye, desired_camera, &filter);

        // Clamped camera Z should stop safely before the wall at -1000.0
        assert!(
            clamped.z > -1000.0,
            "Camera ray must stop before wall: clamped.z={}",
            clamped.z
        );
        assert!(clamped.z < -800.0);
    }

    #[test]
    fn test_character_to_character_separation_cl_check_column2_column_hit() {
        let world = CollisionWorld::new();
        let config = CollisionConfig::default();
        let mut controller = CollisionController::new(world, config);

        // James at (0, 0, 0) with radius 260
        let james =
            CharacterCylinder::new(CharacterId(1), Vec3::new(0.0, 0.0, 0.0), 260.0, 1800.0, 1.0);
        // Nurse at (300, 0, 0) with radius 240. Combined radius = 500 > 300 (overlapping!)
        let nurse = CharacterCylinder::new(
            CharacterId(2),
            Vec3::new(300.0, 0.0, 0.0),
            240.0,
            1800.0,
            1.0,
        );

        controller.register_character(james);
        controller.register_character(nurse);

        controller.resolve_character_separations();

        let new_james = controller
            .characters
            .iter()
            .find(|c| c.id == CharacterId(1))
            .unwrap();
        let new_nurse = controller
            .characters
            .iter()
            .find(|c| c.id == CharacterId(2))
            .unwrap();

        let dist = (new_nurse.position.x - new_james.position.x).abs();
        assert!(
            dist >= 499.9,
            "Cylinders must be pushed apart to at least combined radius 500.0; got {}",
            dist
        );
    }

    #[test]
    fn test_col_spawn_centroid_snapping() {
        let world = CollisionWorld::create_synthetic_room(4000.0, 4000.0, 3000.0, 0.0, 0.0);
        let spawn = world
            .find_safe_spawn_point(500.0, 500.0)
            .expect("Must find floor centroid");

        // Synthetic floor quad (-2000..2000, -2000..2000) split into 2 triangles
        assert_eq!(spawn.y, 0.0);
        assert!(spawn.x.abs() <= 2000.0);
        assert!(spawn.z.abs() <= 2000.0);
    }

    #[test]
    fn test_dynamic_wall_pushback() {
        let mut world = CollisionWorld::new();
        let config = CollisionConfig::default();

        // Add a closed dynamic door wall at X = 500 across Z: -500..500
        world.add_dynamic_wall(DynamicWall::new(
            500.0, -500.0, 500.0, 500.0, -3000.0, 100.0, 0,
        ));

        let current_pos = Vec3::new(400.0, 0.0, 0.0);
        let desired_pos = Vec3::new(480.0, 0.0, 0.0);

        let resolved = world.resolve_character_movement(current_pos, desired_pos, &config);
        assert!(
            resolved.x <= 500.0 - 260.0 + 0.1,
            "Dynamic wall must push character back: pos.x={}",
            resolved.x
        );
    }

    #[test]
    fn test_col_merge_rooms() {
        let room_a = CollisionWorld::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
        let room_b = CollisionWorld::create_synthetic_room(4000.0, 4000.0, 3000.0, 0.0, 0.0);

        let merged = CollisionWorld::merge(&[&room_a, &room_b]);
        assert_eq!(
            merged.floors.len(),
            room_a.floors.len() + room_b.floors.len()
        );
        assert_eq!(merged.walls.len(), room_a.walls.len() + room_b.walls.len());
    }
}
