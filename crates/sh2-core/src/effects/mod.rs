//! Core 1:1 PS2 effect simulations (`SH2-100`):
//! - [`fire_smoke`]: Angela's staircase fire (`stg_hotel_fire.c`, `hh_class_fire_phenomenon_00.c`,
//!   `hh_class_flame_00..03.c`), rising smoke (`ef_smoke.c`), and toggleable native fire embers/light.
//! - [`water_rain`]: Flooded corridor ripples (`hh_class_water_pool_00.c`, `hh_class_water_00.c`,
//!   `hh_class_water_common.c`), splashes (`hh_class_water_splash_phenomenon_00.c`), and rain (`ef_rain.c`).
//! - [`combat_fx`]: Brass shell casings (`ef_cartridge.c`), metallic impact sparks (`ef_sparks.c`),
//!   and shattering glass shards (`ef_broken_glass.c`, `hh_class_glass_break_phenomenon_00.c`,
//!   `hh_class_glass_piece_00.c`).

pub mod combat_fx;
pub mod fire_smoke;
pub mod water_rain;

pub use combat_fx::{
    GlassBreakPhenomenon, GlassBreakSystem, GlassShard, ImpactSpark, ImpactSparkSystem,
    ShellCasing, ShellCasingSystem, ShellWeaponKind, SparkFlashLight, GLASS_GRAVITY_MM_S2,
    GLASS_MAX_SHARDS_PER_BURST, GLASS_SPECULAR_COS_MIN, GLASS_UV_STEP,
};
pub use fire_smoke::{
    FireEmber, FirePhenomenon, FlameEmitter, FlameInitParams, FlameParticle, SmokeParticle,
    SmokeSystem, FLAME_00_PARM_TABLE, FLAME_02_PARM_TABLE, FLAME_GRAVITY_ACCEL_MM_S2,
    FLAME_UV_QUADRANTS, HOTEL_FIRE_POST_INFO_DEMO_HEAD, HOTEL_FIRE_POST_INFO_NORMAL_HEAD,
};
pub use water_rain::{
    RainDrop, RainDropSystem, WaterGrid2D, WaterPoolRipple, WaterPoolSystem, WaterSplashDroplet,
    WaterSplashSystem, WaveElement, RAIN_FALL_SPEED_MM_S, RAIN_STREAK_LEN_MM,
    WATER_POOL_MOTION_INFO, WATER_POOL_RGBA_END, WATER_POOL_RGBA_START, WATER_POOL_SCALE_END,
    WATER_POOL_UV_GRID,
};

/// Deterministic LCG PRNG matching Team Silent's `shRandI` / `shRandF` usage for particle spawning.
#[derive(Debug, Clone, Copy)]
pub struct FxRng {
    state: u32,
}

impl FxRng {
    /// Creates a new deterministic PRNG from `seed`.
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x1234_5678 } else { seed },
        }
    }

    /// Advances the LCG and returns a 31-bit non-negative integer.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.state >> 1) & 0x7fff_ffff
    }

    /// Returns a uniform float in `[0.0, 1.0)`.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (0x8000_0000u32 as f32)
    }

    /// Returns a uniform float in `[min_val, max_val]` (`shSway1f`).
    #[inline]
    pub fn sway(&mut self, min_val: f32, max_val: f32) -> f32 {
        min_val + self.next_f32() * (max_val - min_val)
    }

    /// Returns a centered integer offset `(rand % span) - (span >> 1)` as `f32`
    /// (`hh_class_flame_00.c` `Particle_Initialize` `0x00281110`).
    #[inline]
    pub fn centered_i32(&mut self, span: i32) -> f32 {
        if span <= 1 {
            return 0.0;
        }
        let r = (self.next_u32() as i32) % span;
        (r - (span >> 1)) as f32
    }
}
