//! Re-exports for sleep culling (`enCheckSleepIn` `0x00142950`, `enCheckSleepOut` `0x001429f0`)
//! and the 8-slot tribe alert broadcast bus (`enSetCommunication` `0x00146d80`,
//! `enCommunicateTribe` `0x00146e40`), deduplicated to [`crate::ai::creatures::common`].

pub use crate::ai::creatures::common::{
    en_check_sleep_in, en_check_sleep_out, TribeAlert, TribeCommunicationBus, EN_SLEEP_IN_DIST_MM,
    EN_SLEEP_OUT_DIST_MM, MAX_TRIBE_COMM_SLOTS,
};

/// Alias for [`EN_SLEEP_IN_DIST_MM`] (`15000.0` mm, `enCheckSleepIn` `0x00142950`).
pub const ENEMY_SLEEP_IN_DIST_MM: f32 = EN_SLEEP_IN_DIST_MM;

/// Alias for [`EN_SLEEP_OUT_DIST_MM`] (`7500.0` mm, `enCheckSleepOut` `0x001429f0`).
pub const ENEMY_SLEEP_OUT_DIST_MM: f32 = EN_SLEEP_OUT_DIST_MM;

/// Backwards-compatible alias for [`TribeCommunicationBus`].
pub type TribeCommPool = TribeCommunicationBus;

/// Backwards-compatible alias for [`TribeAlert`].
pub type TribeCommSlot = TribeAlert;
