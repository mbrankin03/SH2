//! Player and environment collision detection and response.
//!
//! Ground truth and 1:1 PS2/PC parity implementation:
//! - PS2 beta `cl_main.c` / `clCheckBg2Chara` & `col_push`: Character cylinder against wall segments
//!   with iterative projection and sliding along surface normals.
//! - PS2 beta `cl_main.c` / `clCollectCharaHeightNormal` & `col_floor_climb`: Barycentric floor height
//!   sampling with stair step-up and step-down thresholds.

use glam::Vec3;
use sh2_assets::formats::cld::{CldFile, FloorTriangle, WallSegment};

/// Configuration constants for character collision and kinematics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionConfig {
    /// Character cylinder collision radius in world units (mm).
    /// James/Maria collision radius is ~250.0 - 300.0 mm.
    pub radius: f32,
    /// Character cylinder height from feet to head in world units.
    /// Default is ~1800.0 mm.
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

/// Active collision environment combining static map `.cld` geometry and dynamic extra walls.
#[derive(Debug, Clone, Default)]
pub struct CollisionWorld {
    /// Floor triangles for height sampling and stepping.
    pub floors: Vec<FloorTriangle>,
    /// Wall segments for lateral cylinder containment and sliding.
    pub walls: Vec<WallSegment>,
    /// Extra transient or stage-added wall segments.
    pub extra_walls: Vec<WallSegment>,
}

impl CollisionWorld {
    /// Create an empty collision world.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load collision geometry from a parsed `.cld` file.
    pub fn from_cld(cld: &CldFile) -> Self {
        Self {
            floors: cld.floors.clone(),
            walls: cld.walls.clone(),
            extra_walls: Vec::new(),
        }
    }

    /// Construct a procedural room collision environment matching the fallback boiler room.
    pub fn create_synthetic_room(w: f32, l: f32, h: f32, pw: f32, pl: f32) -> Self {
        let cld = CldFile::create_synthetic_room(w, l, h, pw, pl);
        Self::from_cld(&cld)
    }

    /// Clear extra dynamic wall segments.
    pub fn clear_extra_walls(&mut self) {
        self.extra_walls.clear();
    }

    /// Add an extra transient wall segment.
    pub fn add_extra_wall(&mut self, wall: WallSegment) {
        self.extra_walls.push(wall);
    }

    /// Iterate over all active walls (static + extra).
    pub fn all_walls(&self) -> impl Iterator<Item = &WallSegment> {
        self.walls.iter().chain(self.extra_walls.iter())
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
        let mut best_y: Option<f32> = None;

        for tri in &self.floors {
            if let Some(y) = tri.sample_height(x, z) {
                if y >= y_ref - up && y <= y_ref + down {
                    match best_y {
                        None => best_y = Some(y),
                        Some(prev_y) => {
                            // Y down: smaller numeric value is higher in space
                            if y < prev_y {
                                best_y = Some(y);
                            }
                        }
                    }
                }
            }
        }

        best_y
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

                let mut ox = pos.x - px;
                let mut oz = pos.z - pz;
                let d_sq = ox * ox + oz * oz;

                if d_sq >= r_sq {
                    continue;
                }

                let mut d = d_sq.sqrt();
                if d < 1e-3 {
                    // Position is exactly on the segment line: push perpendicular to segment
                    let l = len_sq.sqrt();
                    ox = -dz / l;
                    oz = dx / l;
                    d = 0.0;
                } else {
                    ox /= d;
                    oz /= d;
                }

                let push_dist = radius - d;
                pos.x += ox * push_dist;
                pos.z += oz * push_dist;
                moved = true;
                pushed = true;
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
}
