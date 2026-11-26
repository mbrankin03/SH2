//! Retail `.mes` event message cache, pagination/dismiss lifecycle, and HUD subtitle rendering
//! for the unified Campaign Game Loop (`SH2-157`).
//!
//! Stage-to-`.mes` path mapping verified 1:1 against:
//! - `SLUS_202.28` `Event/stg_overlay.c` `StgOverlayGetFileID` (`0x00228300`, stages `2..=52`),
//! - `sh2pc.exe` `STRINGS.tsv` (`0x0066dc88..0x0066e1f4` *Born from a Wish* stages `53..=58`,
//!   `0x006724d4` `data/etc/message/common_msg_e.mes`),
//! - `SLUS_202.28` `Font/font.c` `fontPrintStrMain` (`0x001b4a10`) & `fontNextMessage` (`0x001b9cb0`).

use std::path::PathBuf;

use sh2_assets::formats::mes::{parse_mes, MessageTable};
use sh2_assets::AssetPathResolver;

/// Relative path to the retail common message table (`sh2pc.exe` `0x006724d4`).
pub(crate) const COMMON_MES_REL_PATH: &str = "etc/message/common_msg_e.mes";

/// Default subtitle text color matching retail white (`fontSetColor(0)` `0x001b73a0` scaled to 8-bit).
pub(crate) const CAMPAIGN_SUBTITLE_RGBA: [u8; 4] = [240, 240, 240, 255];

/// Default subtitle glyph height in pixels at 720p (`28` px, matching `combat_arena/hud.rs`).
pub(crate) const CAMPAIGN_SUBTITLE_PX_HEIGHT: usize = 28;

/// Origin of an active campaign event message (`EventMainStandard` `0x001a7b50`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CampaignMessageSource {
    /// Active stage's `stage_<name>_msg_e.mes` table (`effects.stage_message`).
    Stage,
    /// Shared `common_msg_e.mes` table (`effects.common_message`).
    Common,
}

/// Active paginated message currently displayed on the campaign HUD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveCampaignMessage {
    /// Which `.mes` table supplied this message.
    pub source: CampaignMessageSource,
    /// Zero-based message ID within the `.mes` table.
    pub msg_id: u16,
    /// Decoded pages from `DialogueMessage::pages`.
    pub pages: Vec<String>,
    /// Zero-based current page index (`0..pages.len()`).
    pub page_idx: usize,
    /// `true` when driven continuously by an active `StageProgramRunner` (`ev_prog`).
    pub driven_by_program: bool,
}

impl ActiveCampaignMessage {
    /// Returns the text of the currently visible page, if any.
    #[must_use]
    pub fn current_page_text(&self) -> Option<&str> {
        self.pages.get(self.page_idx).map(String::as_str)
    }
}

/// Cached `.mes` message tables (`common_msg_e.mes` + active stage `.mes`) and live HUD message
/// state for unified Campaign mode (`SH2-157`).
#[derive(Debug, Default)]
pub(crate) struct CampaignMessageState {
    common_table: Option<(bool, MessageTable)>,
    stage_table: Option<(u16, bool, MessageTable)>,
    active: Option<ActiveCampaignMessage>,
}

impl CampaignMessageState {
    /// Clears any active message without evicting cached `.mes` tables.
    pub fn clear_active(&mut self) {
        self.active = None;
    }

    /// Resets active message and cached tables when leaving or restarting a campaign session.
    pub fn reset(&mut self) {
        self.active = None;
        self.stage_table = None;
    }

    /// Returns the currently displayed message metadata, if any.
    #[cfg(test)]
    #[must_use]
    pub fn active(&self) -> Option<&ActiveCampaignMessage> {
        self.active.as_ref()
    }

    /// Returns the currently visible page text for HUD rendering, if any.
    #[must_use]
    pub fn active_page_text(&self) -> Option<&str> {
        self.active
            .as_ref()
            .and_then(ActiveCampaignMessage::current_page_text)
    }

    /// Ensures `common_msg_e.mes` and the active stage's `stage_*_msg_e.mes` are loaded and cached.
    pub fn ensure_tables_loaded(&mut self, stage_id: u16, prefer_sh2ee: bool) {
        if self
            .common_table
            .as_ref()
            .is_none_or(|(pref, _)| *pref != prefer_sh2ee)
        {
            if let Some(tbl) = load_mes_table(COMMON_MES_REL_PATH, prefer_sh2ee) {
                self.common_table = Some((prefer_sh2ee, tbl));
            }
        }
        if self
            .stage_table
            .as_ref()
            .is_none_or(|(id, pref, _)| *id != stage_id || *pref != prefer_sh2ee)
        {
            if let Some(rel) = stage_mes_rel_path(stage_id) {
                if let Some(tbl) = load_mes_table(rel, prefer_sh2ee) {
                    self.stage_table = Some((stage_id, prefer_sh2ee, tbl));
                }
            }
        }
    }

    /// Injects pre-parsed message tables directly (used by deterministic unit/integration tests).
    #[cfg(test)]
    pub fn set_tables_for_test(
        &mut self,
        stage_id: u16,
        stage_table: MessageTable,
        common_table: MessageTable,
    ) {
        self.stage_table = Some((stage_id, false, stage_table));
        self.common_table = Some((false, common_table));
    }

    /// Applies `CampaignEffects` message directives (`stage_message`, `common_message`,
    /// `clear_message`) emitted by `CampaignOrchestrator::step`.
    pub fn apply_effects(
        &mut self,
        stage_id: u16,
        prefer_sh2ee: bool,
        stage_message: Option<u16>,
        common_message: Option<u16>,
        clear_message: bool,
        prog_running: bool,
    ) {
        if clear_message && stage_message.is_none() && common_message.is_none() {
            self.active = None;
            return;
        }

        let requested = if let Some(id) = stage_message {
            Some((CampaignMessageSource::Stage, id))
        } else {
            common_message.map(|id| (CampaignMessageSource::Common, id))
        };

        let Some((source, msg_id)) = requested else {
            return;
        };

        // Do not reset page index when `StageProgramRunner` re-emits the same message each tick.
        if let Some(ref mut cur) = self.active {
            if cur.source == source && cur.msg_id == msg_id {
                cur.driven_by_program = prog_running;
                return;
            }
        }

        self.ensure_tables_loaded(stage_id, prefer_sh2ee);
        let pages = self.resolve_pages(source, msg_id);
        if pages.is_empty() {
            self.active = None;
            return;
        }

        self.active = Some(ActiveCampaignMessage {
            source,
            msg_id,
            pages,
            page_idx: 0,
            driven_by_program: prog_running,
        });
    }

    /// Handles a confirm/dismiss keypress (`KeyE` / `Enter` / `Space` / `Backspace`) for a
    /// standalone (non-`ev_prog`) message.
    ///
    /// Advances to the next page when multi-page, or clears the message on the final page.
    /// Returns `true` when the keypress was consumed by an active standalone message.
    pub fn handle_dismiss_input(&mut self) -> bool {
        let Some(ref mut cur) = self.active else {
            return false;
        };
        if cur.driven_by_program {
            // `StageProgramRunner` owns its own message lifecycle via `PROG_KEY_A` / `clear_message`.
            return false;
        }
        if cur.page_idx + 1 < cur.pages.len() {
            cur.page_idx += 1;
        } else {
            self.active = None;
        }
        true
    }

    /// Resolves decoded pages for `(source, msg_id)` from the cached tables.
    #[must_use]
    pub fn resolve_pages(&self, source: CampaignMessageSource, msg_id: u16) -> Vec<String> {
        let table = match source {
            CampaignMessageSource::Stage => self.stage_table.as_ref().map(|(_, _, t)| t),
            CampaignMessageSource::Common => self.common_table.as_ref().map(|(_, t)| t),
        };
        let Some(msg) = table.and_then(|t| t.get(usize::from(msg_id))) else {
            return Vec::new();
        };
        if !msg.pages.is_empty() {
            msg.pages.clone()
        } else if !msg.text.is_empty() {
            vec![msg.text.clone()]
        } else {
            Vec::new()
        }
    }
}

/// Returns the canonical relative `.mes` path under `data/` or `sh2e/` for retail `stage_id`
/// (`1..=58`), verified 1:1 against `SLUS_202.28` `StgOverlayGetFileID` (`0x00228300`) and
/// `sh2pc.exe` `STRINGS.tsv` (`0x0066dc88..0x0066e1f4`).
#[must_use]
pub const fn stage_mes_rel_path(stage_id: u16) -> Option<&'static str> {
    match stage_id {
        1 | 3 => Some("etc/message/stage_observation_msg_e.mes"),
        2 => Some("etc/message/stage_toilet_msg_e.mes"),
        4 => Some("etc/message/stage_forest_msg_e.mes"),
        5 => Some("etc/message/stage_town_east_msg_e.mes"),
        6 => Some("etc/message/stage_apart_e1f_msg_e.mes"),
        7 => Some("etc/message/stage_apart_e2f_msg_e.mes"),
        8 => Some("etc/message/stage_apart_e3fw_msg_e.mes"),
        9 => Some("etc/message/stage_apart_e3fe_msg_e.mes"),
        10 => Some("etc/message/stage_apart_w1f_msg_e.mes"),
        11 => Some("etc/message/stage_apart_w2f_msg_e.mes"),
        12 => Some("etc/message/stage_apart_stair_msg_e.mes"),
        13 => Some("etc/message/stage_apart_out_msg_e.mes"),
        14 => Some("etc/message/stage_town_west_msg_e.mes"),
        15 => Some("etc/message/stage_bowling_msg_e.mes"),
        16 => Some("etc/message/stage_to_heaven_msg_e.mes"),
        17 => Some("etc/message/stage_heaven_night_msg_e.mes"),
        18 => Some("etc/message/stage_hospital_1f_f_msg_e.mes"),
        19 => Some("etc/message/stage_hospital_2f_f_msg_e.mes"),
        20 => Some("etc/message/stage_hospital_3f_f_msg_e.mes"),
        21 => Some("etc/message/stage_hospital_rf_f_msg_e.mes"),
        22 => Some("etc/message/stage_hospital_1fw_b_msg_e.mes"),
        23 => Some("etc/message/stage_hospital_1fe_b_msg_e.mes"),
        24 => Some("etc/message/stage_hospital_2f_b_msg_e.mes"),
        25 => Some("etc/message/stage_hospital_3f_b_msg_e.mes"),
        26 => Some("etc/message/stage_hospital_bf_b_msg_e.mes"),
        27 => Some("etc/message/stage_hospital_pass_msg_e.mes"),
        28 => Some("etc/message/stage_society_msg_e.mes"),
        29 => Some("etc/message/stage_delusion_2_msg_e.mes"),
        30 => Some("etc/message/stage_delusion_3_msg_e.mes"),
        31 => Some("etc/message/stage_prison_n_msg_e.mes"),
        32 => Some("etc/message/stage_prison_s_msg_e.mes"),
        33 => Some("etc/message/stage_prison_bf_msg_e.mes"),
        34 => Some("etc/message/stage_labyrinth_w_msg_e.mes"),
        35 => Some("etc/message/stage_labyrinth_e_msg_e.mes"),
        36 => Some("etc/message/stage_labyrinth_n_msg_e.mes"),
        37 => Some("etc/message/stage_eddie_boss_msg_e.mes"),
        38 => Some("etc/message/stage_lake_msg_e.mes"),
        39 => Some("etc/message/stage_hotel_bf_f_msg_e.mes"),
        40 => Some("etc/message/stage_hotel_1f_f_msg_e.mes"),
        41 => Some("etc/message/stage_hotel_2f_f_msg_e.mes"),
        42 => Some("etc/message/stage_hotel_3f_f_msg_e.mes"),
        43 => Some("etc/message/stage_hotel_bf_b_msg_e.mes"),
        44 => Some("etc/message/stage_hotel_1f_b_msg_e.mes"),
        45 => Some("etc/message/stage_hotel_2f_b_msg_e.mes"),
        46 => Some("etc/message/stage_hotel_3f_b_msg_e.mes"),
        47 => Some("etc/message/stage_hotel_fire_msg_e.mes"),
        48 => Some("etc/message/stage_end_recovery_msg_e.mes"),
        49 => Some("etc/message/stage_end_maria_msg_e.mes"),
        50 => Some("etc/message/stage_end_suicide_msg_e.mes"),
        51 => Some("etc/message/stage_end_rebirth_msg_e.mes"),
        52 => Some("etc/message/stage_end_dog_msg_e.mes"),
        53 => Some("etc/message/stage_x_heaven_msg_e.mes"),
        54 => Some("etc/message/stage_x_town_msg_e.mes"),
        55 => Some("etc/message/stage_x_mansion_1f_msg_e.mes"),
        56 => Some("etc/message/stage_x_mansion_2f_msg_e.mes"),
        57 => Some("etc/message/stage_x_mansion_bf_msg_e.mes"),
        58 => Some("etc/message/stage_x_apart_msg_e.mes"),
        _ => None,
    }
}

/// Loads and parses a retail `.mes` file from `sh2e/` or `data/` depending on `prefer_sh2ee`.
pub(crate) fn load_mes_table(rel_path: &str, prefer_sh2ee: bool) -> Option<MessageTable> {
    let resolver = AssetPathResolver::resolve();
    let roots = [
        resolver.game_root,
        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
    ];
    let subdirs: [&str; 2] = if prefer_sh2ee {
        ["sh2e", "data"]
    } else {
        ["data", "sh2e"]
    };
    for root in &roots {
        for sub in &subdirs {
            let path = root.join(sub).join(rel_path);
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(table) = parse_mes(&bytes) {
                    return Some(table);
                }
            }
        }
    }
    None
}

/// Renders an active campaign event message at the authentic bottom-centered subtitle band
/// (`fontPrintStrMain` `0x001b4a10` default subtitle position).
pub(crate) fn draw_campaign_message(rast: &mut sh2_ui::TextRasterizer, text: &str) {
    let clean = text.replace('\x0C', "\n");
    if clean.trim().is_empty() {
        return;
    }
    let cx = rast.width / 2;
    let bottom_y = rast.height.saturating_sub(56);
    sh2_ui::draw_subtitle(
        rast,
        None,
        &clean,
        cx,
        bottom_y,
        CAMPAIGN_SUBTITLE_PX_HEIGHT,
        CAMPAIGN_SUBTITLE_RGBA,
    );
}
