//! Event Sub-Coroutines (`SLUS_202.28` `Event/event_sub.c` `0x001abf50..0x001ad85b`
//! and `sh2pc.exe` `0x004a22d0..0x004a30aa`).
//!
//! Organized into four focused submodules:
//! - [`self`]: Message, question, item-use, item-pickup-with-crouch-anim, and file-load coroutines.
//! - [`picture_draw`]: `PictureDraw` / `PictureLoadImage` GS command builders (`Event/picture.c`).
//! - [`picture`]: `EvSubPicture*` / `EvSubMapGet` / `EvSubPictureCursor` cutaway coroutines.
//! - [`model_movie`]: `EvDispControlModelEntry`/`Exec` and `EvSubMovieReady`/`Start`/`End`.

pub mod model_movie;
pub mod picture;
pub mod picture_draw;

pub use model_movie::{
    ev_disp_control_model_entry, ev_disp_control_model_exec, ev_sub_movie_end, ev_sub_movie_ready,
    ev_sub_movie_start, DispControlModelEntry, DispControlModelTable, DispOnOffCommand,
    MovieCommand, MovieReadyInput, MovieStartInput, MovieState, MovieTimingProfile,
    DEMO_STATUS_MOVIE_READY_BIT, MOVIE_MAIN_MODE_PC, MOVIE_MAIN_MODE_PS2,
    MOVIE_READY_LEAD_FRAMES_PC, MOVIE_READY_LEAD_FRAMES_PS2, SYS_FLAG_DEMO_ACTIVE_BIT,
};
pub use picture::{
    decode_cursor_axis, ev_sub_map_get, ev_sub_picture_cursor, ev_sub_picture_display,
    ev_sub_picture_display_and_fade_in, ev_sub_picture_display_and_fade_out,
    ev_sub_picture_display_only, ev_sub_picture_end, ev_sub_picture_filter, ev_sub_picture_init,
    ev_sub_picture_layer, ev_sub_picture_start, CursorBounds, CursorInput, CursorTintMode,
    EvSubPictureDisplayInput, EvSubPictureState, CURSOR_DEADZONE_HIGH, CURSOR_DEADZONE_LOW,
    CURSOR_SPEED_X_PX_PER_SEC, CURSOR_SPEED_Y_PX_PER_SEC, EV_FILTER_ALPHA_SCALE,
    EV_FILTER_RATE_PER_SEC,
};
pub use picture_draw::{
    picture_draw, picture_load_image, PictureAlphaBlend, PictureDrawCommand, PictureDrawParams,
    PictureFlags, PictureLoadImageCommand, PicturePrimitiveKind, PictureTestConfig, PictureUv,
    PICTURE_DEFAULT_CBP,
};

use crate::events::engine::{
    EventEngineEffects, EventPositionalSound, EventStepState, PlayerAnimSignal,
};
use crate::inventory::InventoryManager;

/// Crouch pickup vertical threshold in PS2 world millimetres (`100.0` in `EvSubItemGetAndAnim`
/// `SLUS_202.28` `0x001ac470` and `sh2pc.exe` `0x004a2440`).
pub const ITEM_CROUCH_PICKUP_THRESHOLD_MM: f32 = 100.0;

/// Player event animation ID for crouched floor item pickup (`0x4e21` in `0x001ac470` / `0x004a2440`).
pub const ANIM_ID_PICKUP_CROUCH: u16 = 0x4e21;

/// Player event animation ID for standing item pickup (`0x65` in `0x001ac470` / `0x004a2440`).
pub const ANIM_ID_PICKUP_STAND: u16 = 0x0065;

/// Sound effect ID played on item acquisition (`0x2b21` in `0x001ac470` / `0x004a2440`).
pub const SE_ID_ITEM_GET: u32 = 0x2b21;

/// File buffer alignment in bytes for layered picture loading (`0x800` = 2048 bytes in
/// `EvSubFileLoadAndFadeOut` `SLUS_202.28` `0x001ac780` and `sh2pc.exe` `0x004a2670`).
pub const FILE_LAYER_ALIGN_BYTES: usize = 0x800;

/// Executes one frame of `EvSubMessage` (`SLUS_202.28` `0x001abf50`).
///
/// Displays stage message `msg_id`, waits until `message_finished` (`fontGetStatus() != -1`) or
/// `state.ev_cancel`, clears `ev_prog_flag_set` if cancelled (PS2 `0x001abf50`), calls
/// `fontClear()`, and returns `true` on step `2`.
///
/// For retail PC `sh2pc.exe` (`0x004a22d0`) semantics where `ev_cancel` does **not** clear
/// `ev_prog_flag_set`, use [`ev_sub_message_retail_pc`].
pub fn ev_sub_message(
    msg_id: u16,
    message_finished: bool,
    state: &mut EventStepState,
    effects: &mut EventEngineEffects,
) -> bool {
    ev_sub_message_inner(msg_id, message_finished, true, state, effects)
}

/// Executes one frame of retail PC `EvSubMessage` (`sh2pc.exe` `0x004a22d0`).
///
/// Prototype-vs-retail drift: unlike PS2 `SLUS_202.28` (`0x001abf50`), retail PC omits clearing
/// `ev_prog_flag_set` when `state.ev_cancel` dismisses step `1`.
pub fn ev_sub_message_retail_pc(
    msg_id: u16,
    message_finished: bool,
    state: &mut EventStepState,
    effects: &mut EventEngineEffects,
) -> bool {
    ev_sub_message_inner(msg_id, message_finished, false, state, effects)
}

fn ev_sub_message_inner(
    msg_id: u16,
    message_finished: bool,
    clear_prog_flag_on_cancel: bool,
    state: &mut EventStepState,
    effects: &mut EventEngineEffects,
) -> bool {
    match state.ev_s_step {
        2 => true,
        1 => {
            if message_finished || state.ev_cancel {
                if clear_prog_flag_on_cancel && state.ev_cancel {
                    state.ev_prog_flag_set = false;
                }
                effects.clear_message = true;
                state.ev_s_step += 1;
            }
            false
        }
        0 => {
            effects.stage_message = Some(msg_id);
            state.ev_s_step += 1;
            false
        }
        _ => false,
    }
}

/// Executes one frame of `EvSubQuestion` (`SLUS_202.28` `0x001ac040`, `sh2pc.exe` `0x004a2330`).
///
/// Identical to [`ev_sub_message`] except `state.ev_cancel` does not abort step `1`.
pub fn ev_sub_question(
    msg_id: u16,
    message_finished: bool,
    state: &mut EventStepState,
    effects: &mut EventEngineEffects,
) -> bool {
    match state.ev_s_step {
        2 => true,
        1 => {
            if message_finished {
                effects.clear_message = true;
                state.ev_s_step += 1;
            }
            false
        }
        0 => {
            effects.stage_message = Some(msg_id);
            state.ev_s_step += 1;
            false
        }
        _ => false,
    }
}

/// Determines whether an item's use SFX plays before (`after_message == false`) or after
/// (`after_message == true`) the stage message dismisses (`ItemUseSeTiming` in `SLUS_202.28`
/// `0x001ac310`).
///
/// In `SLUS_202.28`, prototype item IDs `0x38, 0x39, 0x3a` (the three Prison Tablets) play
/// their SFX after the message dismisses; in retail PC (`sh2pc.exe`), Born From a Wish item
/// insertions shifted those three Prison Tablets by `+3` to `0x3b, 0x3c, 0x3d` (`59, 60, 61`).
/// Passing `retail_ids = true` checks the retail IDs (`59..=61`); `false` checks the PS2
/// prototype IDs (`56..=58`).
pub const fn item_use_se_timing(item_id: u8, after_message: bool, retail_ids: bool) -> bool {
    let is_delayed_tablet = if retail_ids {
        matches!(item_id, 0x3b..=0x3d)
    } else {
        matches!(item_id, 0x38..=0x3a)
    };
    if is_delayed_tablet {
        after_message
    } else {
        !after_message
    }
}

/// Parameters for [`ev_sub_item_use0`] (`EvSubItemUse0` `SLUS_202.28` `0x001ac100`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvSubItemUseParams {
    /// Item ID passed to `ItemUse` (`param_1`).
    pub item_id: u8,
    /// Stage `.mes` message ID (`param_2`).
    pub msg_id: u16,
    /// Optional sound effect ID (`param_3`; `0` = none).
    pub se_id: i16,
    /// Optional 3D sound position (`param_5`; `None` -> 2D `SeCall`, `Some` -> 3D `SeCallPos`).
    pub se_pos: Option<[f32; 3]>,
    /// Whether to lock/unlock player control (`param_6 != 0`).
    pub lock_player: bool,
    /// Use retail PC (`59..=61`) vs PS2 prototype (`56..=58`) Prison Tablet IDs for `ItemUseSeTiming`.
    pub retail_ids: bool,
}

/// Executes one frame of `EvSubItemUse0` (`SLUS_202.28` `0x001ac100`, `sh2pc.exe` `0x004a33b0`).
///
/// Prototype-vs-retail drift: when `params.retail_ids` is `true` (matching retail PC `0x004a33b0`),
/// step `1` additionally permits `state.ev_cancel` to dismiss the message when `params.lock_player`
/// (`param_6 != 0`) is set; PS2 `0x001ac100` checks `message_dismissed` (`fontGetStatus() == -2`)
/// only.
pub fn ev_sub_item_use0(
    params: &EvSubItemUseParams,
    message_dismissed: bool,
    state: &mut EventStepState,
    inventory: &mut InventoryManager,
    effects: &mut EventEngineEffects,
) -> bool {
    let emit_se = |effects: &mut EventEngineEffects| {
        if params.se_id != 0 {
            if let Some(pos) = params.se_pos {
                effects.positional_sounds.push(EventPositionalSound {
                    se_id: params.se_id,
                    volume: 1.0,
                    pos,
                    channel: 0,
                });
            } else {
                effects.sound_effects.push(params.se_id as u32);
            }
        }
    };

    match state.ev_s_step {
        2 => {
            if params.lock_player {
                effects.player_lock_change = Some(false);
            }
            true
        }
        1 => {
            let retail_cancel = params.retail_ids && params.lock_player && state.ev_cancel;
            if message_dismissed || retail_cancel {
                effects.clear_message = true;
                if params.se_id != 0 && item_use_se_timing(params.item_id, true, params.retail_ids)
                {
                    emit_se(effects);
                }
                state.ev_s_step += 1;
            }
            false
        }
        0 => {
            if params.lock_player {
                effects.player_lock_change = Some(true);
            }
            inventory.item_lose(params.item_id);
            effects.stage_message = Some(params.msg_id);
            if params.se_id != 0 && item_use_se_timing(params.item_id, false, params.retail_ids) {
                emit_se(effects);
            }
            state.ev_s_step += 1;
            false
        }
        _ => false,
    }
}

/// Executes one frame of `EvSubItemGet` (`SLUS_202.28` `0x001ac370`, `sh2pc.exe` `0x004a23a0`).
///
/// Displays stage message `msg_id` (and plays `SE_ID_ITEM_GET` `0x2b21` on step 0 matching retail
/// `sh2pc.exe` `0x004a23a0`), then on `message_dismissed` grants `item_id`, deletes the associated
/// world sub-character `item_chara_id` if non-zero, and clears the message.
pub fn ev_sub_item_get(
    item_id: u8,
    item_chara_id: i32,
    msg_id: u16,
    message_dismissed: bool,
    state: &mut EventStepState,
    inventory: &mut InventoryManager,
    effects: &mut EventEngineEffects,
) -> bool {
    match state.ev_s_step {
        2 => true,
        1 => {
            if message_dismissed {
                inventory.item_get_now(item_id);
                if item_chara_id != 0 {
                    effects.deleted_item_model = Some((item_chara_id, 0));
                }
                effects.clear_message = true;
                state.ev_s_step += 1;
            }
            false
        }
        0 => {
            effects.stage_message = Some(msg_id);
            effects.sound_effects.push(SE_ID_ITEM_GET);
            state.ev_s_step += 1;
            false
        }
        _ => false,
    }
}

/// Inputs for one frame of [`ev_sub_item_get_and_anim`] (`EvSubItemGetAndAnim` `0x001ac470`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvSubItemGetAndAnimInput {
    /// Item ID granted via `ItemGet`.
    pub item_id: u8,
    /// Stage `.mes` message ID displayed upon pickup.
    pub msg_id: u16,
    /// Sub-character ID from `item_to_chara[item_id]` (`0` if none).
    pub item_chara_id: i32,
    /// Player Y position in PS2 world mm (`_sh2jms + 0x24` PS2 / `+ 0x20` PC).
    pub player_y: f32,
    /// World item sub-character Y position in PS2 world mm (`None` if no sub-character exists).
    pub item_y: Option<f32>,
    /// `true` when `PlayerEventAnimeSuccessFrame() != 0`.
    pub anim_success_frame: bool,
    /// `true` when `shCharacterAnimeIsEnd(_sh2jms) != 0`.
    pub anim_ended: bool,
    /// `true` when `fontGetStatus() == -2`.
    pub message_dismissed: bool,
}

/// Executes one frame of `EvSubItemGetAndAnim` (`SLUS_202.28` `0x001ac470`, `sh2pc.exe` `0x004a2440`).
///
/// Exact step machine (`ev_s_step`):
/// - `0`: Locks player; if `item_y` exists and `player_y - item_y < 100.0`, plays crouch pickup
///   (`0x4e21`) -> step `9`, else standing pickup (`0x65`) -> step `10`.
/// - `9`: Waits for `anim_success_frame`, emits [`PlayerAnimSignal::Pause`] -> step `10` (or on
///   `ev_cancel`, clears `ev_prog_flag_set` -> step `11`).
/// - `10`: Displays stage message `msg_id` and plays `SE_ID_ITEM_GET` (`0x2b21`) -> step `3`.
/// - `3`: On `message_dismissed || ev_cancel`, grants `item_id`, clears message, deletes world
///   sub-character if present; if crouched, emits [`PlayerAnimSignal::Restart`] -> step `8`, else
///   step `11`.
/// - `8`: Waits for `anim_ended || ev_cancel` -> step `11`.
/// - `11` (`0xb`): Unlocks player and returns `true`.
pub fn ev_sub_item_get_and_anim(
    input: &EvSubItemGetAndAnimInput,
    state: &mut EventStepState,
    inventory: &mut InventoryManager,
    effects: &mut EventEngineEffects,
) -> bool {
    let is_crouch = input
        .item_y
        .is_some_and(|iy| (input.player_y - iy) < ITEM_CROUCH_PICKUP_THRESHOLD_MM);

    match state.ev_s_step {
        0xb => {
            effects.player_lock_change = Some(false);
            true
        }
        8 => {
            if input.anim_ended || state.ev_cancel {
                state.ev_s_step = 0xb;
            }
            false
        }
        3 => {
            if input.message_dismissed || state.ev_cancel {
                inventory.item_get_now(input.item_id);
                effects.clear_message = true;
                if is_crouch {
                    effects.player_anim_signal = Some(PlayerAnimSignal::Restart);
                    state.ev_s_step = 8;
                } else {
                    state.ev_s_step = 0xb;
                }
                if input.item_y.is_some() && input.item_chara_id != 0 {
                    effects.deleted_item_model = Some((input.item_chara_id, 0));
                }
            }
            false
        }
        10 => {
            effects.stage_message = Some(input.msg_id);
            effects.sound_effects.push(SE_ID_ITEM_GET);
            state.ev_s_step = 3;
            false
        }
        9 => {
            if state.ev_cancel {
                state.ev_prog_flag_set = false;
                state.ev_s_step = 0xb;
            } else if input.anim_success_frame {
                effects.player_anim_signal = Some(PlayerAnimSignal::Pause);
                state.ev_s_step = 10;
            }
            false
        }
        0 => {
            effects.player_lock_change = Some(true);
            if is_crouch {
                effects.player_pickup_anim = Some(ANIM_ID_PICKUP_CROUCH);
                state.ev_s_step = 9;
            } else {
                effects.player_pickup_anim = Some(ANIM_ID_PICKUP_STAND);
                state.ev_s_step = 10;
            }
            false
        }
        _ => false,
    }
}

/// Request produced by [`ev_sub_file_load_and_fade_out`] (`EvSubFileLoadAndFadeOut` `0x001ac780`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileLoadPlan {
    /// Primary picture file ID loaded at offset `0` of the GP data buffer, if any.
    pub main_file_id: Option<u32>,
    /// Secondary layer file ID loaded at `layer_offset_bytes`, if any.
    pub layer_file_id: Option<u32>,
    /// 2048-byte (`0x800`) aligned byte offset for the secondary layer file (`_layer_adr`).
    pub layer_offset_bytes: usize,
}

/// Error returned when [`ev_sub_file_load_and_fade_out`] is called with a layer file but no main
/// file (which triggers an infinite-loop assert in `SLUS_202.28` `0x001ac780` and `sh2pc.exe`
/// `0x004a2670`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("EvSubFileLoadAndFadeOut: layer file specified without a main picture file")]
pub struct InvalidFileLoadArgs;

/// Executes one frame of `EvSubFileLoadAndFadeOut` (`SLUS_202.28` `0x001ac780`,
/// `sh2pc.exe` `0x004a2670`).
///
/// On `ev_s_step == 0`, computes the `0x800`-aligned layer buffer offset, emits a fade-out
/// (`fade_command = Some(1)`), advances `ev_s_step = 3`, and returns the [`FileLoadPlan`].
/// Returns `Ok((done, plan))` where `done` is `true` once `fs_sync_ready && fade_complete`.
pub fn ev_sub_file_load_and_fade_out(
    main_file_id: Option<u32>,
    main_file_size: usize,
    layer_file_id: Option<u32>,
    fs_sync_ready: bool,
    fade_complete: bool,
    state: &mut EventStepState,
    effects: &mut EventEngineEffects,
) -> Result<(bool, Option<FileLoadPlan>), InvalidFileLoadArgs> {
    if main_file_id.is_none() && layer_file_id.is_some() {
        return Err(InvalidFileLoadArgs);
    }

    let mut plan = None;
    if state.ev_s_step == 0 {
        let layer_offset_bytes = if main_file_id.is_some() {
            (main_file_size + (FILE_LAYER_ALIGN_BYTES - 1)) & !(FILE_LAYER_ALIGN_BYTES - 1)
        } else {
            0
        };
        plan = Some(FileLoadPlan {
            main_file_id,
            layer_file_id,
            layer_offset_bytes,
        });
        effects.fade_command = Some(1);
        state.ev_s_step = 3;
    }

    let done = fs_sync_ready && fade_complete;
    Ok((done, plan))
}
