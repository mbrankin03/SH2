//! Dog Ending Room (`R_END_DOG_RM = 0xBD`) 3D control levers enhancement
//! (`Patches/DogRoomEnhanced.cpp`).
//!
//! Ground truth:
//! - `SLUS_202.28` `Fog/fogdata.c` `fogSetCollision` (`0x0016c8a0` line 82 `RoomNameJms() == 0xbd`)
//! - `SLUS_202.28` `StgOverlayGetFileID` (`0x00228300` case `0x34`) / `sh2pc.exe` `0x008c5f48`
//!   (`stage_end_dog`, ev_prog `0x00574430`)
//! - Retail `data/chr/item/inu.mdl` (`0x40..0x90`, `chara_id = 0x10D` = `269`, `anim_info.rs` `inu_pos`)
//! - SH2:EE `Patches/DogRoomEnhanced.cpp` (`sh2e/model/mon.glb`, `kDogModelTable`, `kLeversModelTable`).

/// Room ID for the Dog Ending Control Room (`R_END_DOG_RM = 0xBD`, `SLUS_202.28` `Fog/fogdata.c`
/// `0x0016c8a0`, `Patches.h:345`).
pub const R_END_DOG_RM_ID: u32 = 0xBD;

/// Stage ID for `stage_end_dog` (`52 = 0x34`, `SLUS_202.28` `0x00228300`, `sh2pc.exe` `0x008c5f48`).
pub const STAGE_END_DOG_ID: u16 = 52;

/// Event index at which `mon.glb` is preloaded during room transition (`DogRoomEnhanced.cpp:144`).
pub const DOG_ROOM_PRELOAD_EVENT_INDEX: u32 = 3;

/// Cutscene timer tick rate in Hz used by `GetCutsceneTimer() / 30.0f` (`DogRoomEnhanced.cpp:36, 124`).
pub const DOG_CUTSCENE_TIMER_HZ: f64 = 30.0;

/// Exact 20-`i32` model offset table at `base = 0x40` of retail `data/chr/item/inu.mdl`
/// (`kDogModelTable` in `DogRoomEnhanced.cpp:107`).
pub const DOG_MODEL_OFFSET_TABLE: [i32; 20] = [
    -65533, 4, 176, 31, 2160, 43, 2192, 2288, 3, 5248, 0, 21792, 2, 5040, 2, 5056, 5072, 0, 5104, 0,
];

/// Exact 20-`i32` model offset table for the Dog Room levers (`kLeversModelTable` in
/// `DogRoomEnhanced.cpp:106`).
pub const LEVERS_MODEL_OFFSET_TABLE: [i32; 20] = [
    -65533, 4, 176, 3, 368, 0, 384, 384, 4, 640, 0, 1952, 1, 384, 4, 400, 432, 0, 496, 0,
];

/// Render command returned by [`DogRoomLeversState::evaluate_draw`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DogRoomLeversRenderCmd {
    /// Strict 1:1 vanilla PS2/PC behavior: draw only `inu.mdl`, no 3D `mon.glb` levers overlay.
    VanillaOnly,
    /// Draw `sh2e/model/mon.glb` with identity world transform and alpha blending (`SRCALPHA`/`INVSRCALPHA`),
    /// advancing its animation by `delta_sec` to `anim_timer_sec`, while keeping the vanilla `inu.mdl` draw.
    DrawLeversOverlay {
        /// Current accumulated animation time in seconds.
        anim_timer_sec: f32,
        /// Frame delta in seconds (`cutscene_timer / 30.0 - prev_start_time`).
        delta_sec: f32,
    },
}

/// Runtime state for the Dog Ending Room 3D control levers (`Patches/DogRoomEnhanced.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DogRoomLeversState {
    /// Whether the levers overlay has been initialized for the active Dog Room cutscene.
    pub active: bool,
    /// Last sampled cutscene time in seconds (`GetCutsceneTimer() / 30.0`).
    pub start_time_sec: f64,
    /// Accumulated `mon.glb` animation timer in seconds.
    pub model_anim_timer_sec: f32,
}

impl DogRoomLeversState {
    /// Returns `true` when `sh2e/model/mon.glb` should be preloaded (`DogRoomEnhanced.cpp:142-147`).
    #[must_use]
    pub const fn should_preload(
        room_id: u32,
        event_index: u32,
        mon_glb_available: bool,
        enable_sh2ee_world_fixes: bool,
    ) -> bool {
        enable_sh2ee_world_fixes
            && mon_glb_available
            && room_id == R_END_DOG_RM_ID
            && event_index == DOG_ROOM_PRELOAD_EVENT_INDEX
    }

    /// Checks whether a parsed `.mdl` header's 20-`i32` offset table matches `inu.mdl` (`kDogModelTable`).
    #[must_use]
    pub fn is_dog_model_table(table: &[i32; 20]) -> bool {
        *table == DOG_MODEL_OFFSET_TABLE
    }

    /// Evaluates `ActorDrawTopEpilogue` (`DogRoomEnhanced.cpp:118-139`) for the current draw call.
    ///
    /// Always keeps the vanilla `inu.mdl` draw intact (`return false` in SH2:EE).
    pub fn evaluate_draw(
        &mut self,
        room_id: u32,
        is_dog_model: bool,
        cutscene_timer_ticks: f32,
        mon_glb_available: bool,
        enable_sh2ee_world_fixes: bool,
    ) -> DogRoomLeversRenderCmd {
        if room_id != R_END_DOG_RM_ID {
            self.active = false;
            return DogRoomLeversRenderCmd::VanillaOnly;
        }

        if !enable_sh2ee_world_fixes || !mon_glb_available || !is_dog_model {
            return DogRoomLeversRenderCmd::VanillaOnly;
        }

        let time_now_sec = f64::from(cutscene_timer_ticks) / DOG_CUTSCENE_TIMER_HZ;
        if !self.active {
            self.model_anim_timer_sec = 0.0;
            self.start_time_sec = time_now_sec;
            self.active = true;
        }

        let delta_sec = (time_now_sec - self.start_time_sec).max(0.0) as f32;
        self.start_time_sec = time_now_sec;
        self.model_anim_timer_sec += delta_sec;

        DogRoomLeversRenderCmd::DrawLeversOverlay {
            anim_timer_sec: self.model_anim_timer_sec,
            delta_sec,
        }
    }
}
