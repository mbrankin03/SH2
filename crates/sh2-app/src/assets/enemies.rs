//! On-demand enemy model/animation asset cache with stage-transition eviction (`SH2-181`).
//!
//! Avoids synchronously loading all 12 enemy `.mdl`/`.anm` files at startup before the title
//! screen, lazy-loads only the species spawned in the active room/arena, and evicts unused
//! species on stage transitions.

use sh2_core::ai::creatures::EnemySpecies;
use sh2_core::config::WorldTextureQuality;
use sh2_renderer::RenderContext;

use super::{load_character_asset, CharacterAsset};

/// Dense slot identifier for the 12 retail enemy/boss character rigs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub(crate) enum EnemyAssetSlot {
    /// Lying Figure (`scu.mdl` / `scu.anm`, Bestiary ID 1).
    Scu = 0,
    /// Mannequin (`mkn.mdl` / `mkn.anm`, Bestiary ID 2).
    Mkn = 1,
    /// Bubble Head Nurse (`nse.mdl` / `nse.anm`, Bestiary ID 3).
    Nse = 2,
    /// Dark / Variant Bubble Head Nurse (`xoo.mdl` / `xoo.anm`).
    Xoo = 3,
    /// Pyramid Head (`red.mdl` / `red.anm`, Bestiary ID 4).
    Red = 4,
    /// Pyramid Head Spear / Twin (`oni.mdl` / `oni.anm`).
    Oni = 5,
    /// Abstract Daddy (`pap.mdl` / `pap.anm`, Bestiary ID 5).
    Pap = 6,
    /// Flesh Lips (`ike.mdl` / `ike.anm`, Bestiary ID 6).
    Ike = 7,
    /// Mandarin (`arm.mdl` / `arm.anm`, Bestiary ID 7).
    Arm = 8,
    /// Creeper (`tyu.mdl` / `tyu.anm`, Bestiary ID 8).
    Tyu = 9,
    /// Mary Final Boss (`bos.mdl` / `bos.anm`, Bestiary ID 9).
    Bos = 10,
    /// Eddie Boss (`lll_edi.mdl` / `lll_edi.anm`).
    Edi = 11,
}

impl EnemyAssetSlot {
    /// Total number of distinct enemy asset slots.
    pub const COUNT: usize = 12;

    /// Returns the dense array index (`0..Self::COUNT`).
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Maps a Bestiary creature ID (`1..=9`) to its corresponding [`EnemyAssetSlot`].
    #[inline]
    pub const fn from_creature_id(creature_id: u32) -> Option<Self> {
        match creature_id {
            1 => Some(Self::Scu),
            2 => Some(Self::Mkn),
            3 => Some(Self::Nse),
            4 => Some(Self::Red),
            5 => Some(Self::Pap),
            6 => Some(Self::Ike),
            7 => Some(Self::Arm),
            8 => Some(Self::Tyu),
            9 => Some(Self::Bos),
            _ => None,
        }
    }

    /// Maps an [`EnemySpecies`] (`0x200..=0x20b`) and dark-nurse flag to its [`EnemyAssetSlot`].
    #[inline]
    pub const fn from_species(species: EnemySpecies, is_dark_nurse: bool) -> Option<Self> {
        match species {
            EnemySpecies::LyingFigure => Some(Self::Scu),
            EnemySpecies::Mannequin => Some(Self::Mkn),
            EnemySpecies::Nurse if is_dark_nurse => Some(Self::Xoo),
            EnemySpecies::Nurse => Some(Self::Nse),
            EnemySpecies::PyramidHead => Some(Self::Red),
            EnemySpecies::TwinPyramidHead => Some(Self::Oni),
            EnemySpecies::AbstractDaddy => Some(Self::Pap),
            EnemySpecies::FleshLips => Some(Self::Ike),
            EnemySpecies::Mandarin => Some(Self::Arm),
            EnemySpecies::Creeper => Some(Self::Tyu),
            EnemySpecies::MaryBoss => Some(Self::Bos),
            EnemySpecies::Eddie => Some(Self::Edi),
            EnemySpecies::RoomInsect => None,
        }
    }

    /// Returns `(display_name, mdl_rel_paths, anm_rel_paths)` for this enemy slot.
    pub const fn asset_spec(
        self,
    ) -> (
        &'static str,
        &'static [&'static str],
        &'static [&'static str],
    ) {
        match self {
            Self::Scu => (
                "Lying Figure (scu)",
                &["data/chr/scu/scu.mdl"],
                &["data/chr/scu/scu.anm"],
            ),
            Self::Mkn => (
                "Mannequin (mkn)",
                &["data/chr/mkn/mkn.mdl"],
                &["data/chr/mkn/mkn.anm"],
            ),
            Self::Nse => (
                "Bubble Head Nurse (nse)",
                &["data/chr/nse/nse.mdl"],
                &["data/chr/nse/nse.anm"],
            ),
            Self::Xoo => (
                "Bubble Head Nurse Variant (xoo)",
                &["data/chr/xoo/xoo.mdl"],
                &["data/chr/xoo/xoo.anm"],
            ),
            Self::Red => (
                "Pyramid Head (red)",
                &["data/chr/red/red.mdl"],
                &["data/chr/red/red.anm"],
            ),
            Self::Oni => (
                "Pyramid Head Spear (oni)",
                &["data/chr/oni/oni.mdl"],
                &["data/chr/oni/oni.anm"],
            ),
            Self::Pap => (
                "Abstract Daddy (pap)",
                &["data/chr/pap/pap.mdl"],
                &["data/chr/pap/pap.anm"],
            ),
            Self::Ike => (
                "Flesh Lips (ike)",
                &["data/chr/ike/ike.mdl"],
                &["data/chr/ike/ike.anm"],
            ),
            Self::Arm => (
                "Mandarin (arm)",
                &["data/chr/arm/arm.mdl"],
                &["data/chr/arm/arm.anm"],
            ),
            Self::Tyu => (
                "Creeper (tyu)",
                &["data/chr/tyu/tyu.mdl"],
                &["data/chr/tyu/tyu.anm"],
            ),
            Self::Bos => (
                "Mary Final Boss (bos)",
                &["data/chr/bos/bos.mdl"],
                &["data/chr/bos/bos.anm"],
            ),
            Self::Edi => (
                "Eddie Boss (lll_edi)",
                &["data/chr/edi/lll_edi.mdl"],
                &["data/chr/edi/lll_edi.anm"],
            ),
        }
    }
}

/// Fixed-slot cache of lazy-loaded enemy [`CharacterAsset`]s (`SH2-181`).
pub(crate) struct EnemyAssetCache {
    slots: [Option<CharacterAsset>; EnemyAssetSlot::COUNT],
    attempted: [bool; EnemyAssetSlot::COUNT],
}

impl Default for EnemyAssetCache {
    fn default() -> Self {
        Self::new()
    }
}

impl EnemyAssetCache {
    /// Creates an empty cache with zero models loaded at startup.
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
            attempted: [false; EnemyAssetSlot::COUNT],
        }
    }

    /// Returns an immutable reference to `slot` if already loaded.
    #[inline]
    pub fn get(&self, slot: EnemyAssetSlot) -> Option<&CharacterAsset> {
        self.slots[slot.index()].as_ref()
    }

    /// Returns a mutable reference to `slot` if already loaded.
    #[inline]
    #[allow(dead_code)]
    pub fn get_mut(&mut self, slot: EnemyAssetSlot) -> Option<&mut CharacterAsset> {
        self.slots[slot.index()].as_mut()
    }

    /// Ensures `slot` is loaded at `quality`, returning a mutable reference if available on disk.
    pub fn ensure_loaded(
        &mut self,
        ctx: &RenderContext,
        slot: EnemyAssetSlot,
        quality: WorldTextureQuality,
    ) -> Option<&mut CharacterAsset> {
        let idx = slot.index();
        if self.slots[idx].is_none() && !self.attempted[idx] {
            self.attempted[idx] = true;
            let (name, mdls, anms) = slot.asset_spec();
            self.slots[idx] = load_character_asset(ctx, name, mdls, anms, quality);
        }
        self.slots[idx].as_mut()
    }

    /// Evicts any currently loaded enemy species not present in `needed`, then lazy-loads any
    /// missing species in `needed` at `quality`.
    pub fn preload_for_slots(
        &mut self,
        ctx: &RenderContext,
        needed: &[EnemyAssetSlot],
        quality: WorldTextureQuality,
    ) {
        let mut keep = [false; EnemyAssetSlot::COUNT];
        for &slot in needed {
            keep[slot.index()] = true;
        }

        let mut evicted = 0usize;
        for idx in 0..EnemyAssetSlot::COUNT {
            if !keep[idx] {
                if self.slots[idx].take().is_some() {
                    evicted += 1;
                }
                self.attempted[idx] = false;
            }
        }
        if evicted > 0 {
            tracing::info!(
                "[EnemyAssetCache] Evicted {} unused enemy species on stage transition.",
                evicted
            );
        }

        for &slot in needed {
            let _ = self.ensure_loaded(ctx, slot, quality);
        }
    }

    /// Drops all currently resident enemy models/textures (e.g. returning to Title Menu or
    /// entering a non-combat room).
    pub fn evict_all(&mut self) {
        let mut evicted = 0usize;
        for idx in 0..EnemyAssetSlot::COUNT {
            if self.slots[idx].take().is_some() {
                evicted += 1;
            }
            self.attempted[idx] = false;
        }
        if evicted > 0 {
            tracing::info!(
                "[EnemyAssetCache] Evicted {} resident enemy species.",
                evicted
            );
        }
    }

    /// Reloads GPU textures for all currently resident enemy slots when `WorldTextureQuality`
    /// changes in Options.
    pub fn reload_textures(&mut self, ctx: &RenderContext, quality: WorldTextureQuality) {
        for slot in &mut self.slots {
            if let Some(asset) = slot.as_mut() {
                asset.reload_textures_for_quality(ctx, quality);
            }
        }
    }
}
