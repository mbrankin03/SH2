//! Authentic Silent Hill 2 Sound Effect (`sddata.bin` & `voice.afs`) lookup tables,
//! 3D distance attenuation (`SeCallPosDistanceF` / `SeCallPosDistance`), stereo pan
//! (`SeCallPosDirection`), and surface material footstep selection (`SH2-61`, `SH2-137`).
//!
//! # Ground Truth & Binary References (`SLUS_202.28`, `sh2pc.exe`, & SH2:EE)
//! - **`0x8A1ED8` (`0x29A` = 666 pairs)**: Maps game `se_id` (`u32`, e.g., `10000` / `0x2710` = `CursorMove`,
//!   `11048` / `0x2B28` = `HandgunShot`) to internal driver ID `5000..=5698`, which indexes `0x8A67E0` (`SFXAddrMap`)
//!   to resolve one of the **417** RIFF WAVE entries in `sddata.bin` (`0..=416`).
//! - **`0x8A1E58` (32 pairs)**: Maps streamed SFX `se_id` (`u16`, e.g., `10051` / `0x2743` = `SavePoint`,
//!   `15002` / `0x3A9A` = `GameStartTransition`) to `voice.afs` entry indices (`raw_id - 0xEA60`, `13..=44`).
//! - **`SeCallPosDistanceF` (`SLUS_202.28` `0x00192bc0`) & `SeCallPosDistance` (`0x00192a70` / `sh2pc.exe` `0x005155e0`)**:
//!   - Inner full-volume radius: $d_{\min} = 1500.0$ mm (`0x00192bc0` / `sh2pc` `0x006B4038`).
//!   - Reciprocal attenuation span: $0.0001$ (`0x00192bc0` / `sh2pc` `0x006C5D70`), giving outer cutoff $d_{\max} = 11500.0$ mm.
//!   - Dual-distance linear product combining camera-to-James distance $d_{\text{cam}}$ (`vcWork.cam_pos` `0x010e5b40`
//!     vs `sh2jms + 0x20`) and source-to-James distance $d_{\text{src}}$:
//!     $$\text{atten} = (1 - \text{clamp}((d_{\text{cam}} - 1500)\cdot 0.0001, 0, 1)) \cdot (1 - \text{clamp}((d_{\text{src}} - 1500)\cdot 0.0001, 0, 1))$$
//!   - Byte attenuation passed to `shSdCall`: `(int)(255.0 - base_volume * atten * 255.0)` (`0` = loudest, `255` = silent).
//! - **`SeCallPosDirection` (`SLUS_202.28` `0x00192990` / `sh2pc.exe` `0x00515e60`)**:
//!   - Transforms `[pos.x, pos.y, pos.z, 1.0]` by `VbWvsMatrix` (`0x010e59d0` / `sh2pc` `0x01f80e00`).
//!   - Offsets camera-space X by `-750.0` mm (`0x00192990` / `sh2pc` `0x006a8478`), zeroes Y, normalizes XZ,
//!     and outputs `(int)(sign(-x) * (70.0 - z * 70.0))` (`±70°` at left/right, `0°` ahead, `±140°` behind).
//!   - **Prototype vs. Retail PC**: Both `SLUS_202.28` and `sh2pc.exe` use identical equations and constants for
//!     `SeCallPosDistance`, `SeCallPosDistanceF`, and `SeCallPosDirection`.
//! - **SH2:EE Patches**:
//!   - `Patches/PlayWavSound.cpp`: Standalone `.wav` cues in `sh2e/sound/extra/` (`g_start.wav`, `save_sound.wav`,
//!     `flashlight_on.wav`, `flashlight_off.wav`).
//!   - `Patches/ChainsawSoundFix.cpp`: Chainsaw idle loop (`0x1642` -> `sddata[352]`) skips initial rev transient on loop.

mod lookup;
mod sddata_table_core;
mod sddata_table_stage;
mod spatial;

pub use lookup::{
    footstep_sddata_indices, resolve_se_id_to_sddata_index, resolve_se_id_to_voice_afs_index,
    SE_ID_TO_VOICE_AFS_INDEX,
};
pub use spatial::{
    build_ps2_wvs_matrix, compute_se_call_pos_distance_f, compute_se_call_pos_spatial,
    compute_sfx_distance_param_v, se_call_pos_direction, se_call_pos_distance,
    se_call_pos_distance_f, SfxSpatialEval, SE_DIR_CAMERA_X_OFFSET_MM, SE_DIR_PAN_SCALE_DEG,
    SFX_DISTANCE_INV_SPAN, SFX_DISTANCE_SPAN, SFX_MAX_DISTANCE, SFX_MIN_DISTANCE,
};

/// Total number of RIFF WAVE entries inside retail `data/sound/sddata.bin` and SH2:EE `sh2e/sound/sddata.bin`.
pub const SDDATA_ENTRY_COUNT: usize = 417;

/// Concatenates the two halves of the 666-entry `sh2pc.exe` `0x8A1ED8` -> `0x8A67E0` lookup table at compile time.
const fn concat_sddata_table<const A: usize, const B: usize, const N: usize>(
    a: [(u16, u16); A],
    b: [(u16, u16); B],
) -> [(u16, u16); N] {
    let mut out = [(0u16, 0u16); N];
    let mut i = 0;
    while i < A {
        out[i] = a[i];
        i += 1;
    }
    let mut j = 0;
    while j < B {
        out[A + j] = b[j];
        j += 1;
    }
    out
}

/// Authentic 666-entry `(se_id, sddata_index)` lookup table extracted 1:1 from `sh2pc.exe` (`0x8A1ED8` -> `0x8A67E0`).
pub const SE_ID_TO_SDDATA_INDEX: [(u16, u16); 666] = concat_sddata_table(
    sddata_table_core::SE_ID_TO_SDDATA_INDEX_CORE,
    sddata_table_stage::SE_ID_TO_SDDATA_INDEX_STAGE,
);
