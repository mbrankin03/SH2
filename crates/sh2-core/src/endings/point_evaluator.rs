//! Live Ending Point Evaluator (`EvProgEndingJudge` `0x00576490`, Low-HP / Overheal / Maria / Hallway Monologue).
//!
//! Verified 1:1 against `sh2pc.exe` (`0x00576490..0x0057672a`, `0x0048eccb..0x0048ed8c`,
//! `0x0049ecf5..0x0049ed32`, `0x00471ca6..0x0047219b`, `0x005767e0`).

use std::f32::consts::PI;

use crate::events::EventFlags;

use super::playing_info::{
    ClearEndKind, MainEndingKind, PlayingInfo, FLAG_ANGELAS_KNIFE_EXAMINED,
    FLAG_HOSPITAL_ROOF_DIARY, FLAG_HOSPITAL_S3_MARIA_SECOND_CHECK, FLAG_HOTEL_READING_ROOM_TAPE,
    FLAG_LABYRINTH_MARIA_REVISIT, FLAG_MARIA_REMINDED_ABOUT_MARY, FLAG_MARIA_WRONG_DIRECTION,
    FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE, FLAG_NEELYS_BAR_SECOND_MESSAGE, IN_WATER_START_POINTS,
    ITEM_ANGELAS_KNIFE, ITEM_LETTER_FROM_MARY, ITEM_PHOTO_OF_MARY, LEAVE_START_POINTS,
    LOW_HP_RATIO_THRESHOLD, LOW_HP_WEIGHTED_SECS_TIER_1, LOW_HP_WEIGHTED_SECS_TIER_2,
    MARIA_BUMP_COUNTER_CAP, MARIA_BUMP_HIGH_THRESHOLD, MARIA_BUMP_LOW_THRESHOLD,
    MARIA_CLOSE_DISTANCE_MM, MARIA_CLOSE_TIME_THRESHOLD_SECS, MARIA_ENEMY_DAMAGE_RATIO_LIMIT,
    MARIA_FAR_DISTANCE_MM, MARIA_FAR_TIME_THRESHOLD_SECS, MARIA_HP_SCALE_EASY,
    MARIA_HP_SCALE_NORMAL_HARD, MARIA_START_POINTS, MARY_CORRIDOR_COMPLETE_SUBTITLE_FRAMES,
    MARY_CORRIDOR_START_DELAY_SECS, OVERHEAL_RATIO_THRESHOLD, ROOM_HOTEL_CORRIDOR_MONOLOGUE,
};

/// Stage event flags consulted by `EvProgEndingJudge` (`0x005764c9..0x00576530`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EndingEvalFlags {
    /// Flag `655`: Walked wrong way away from Rosewater Park / Hotel (`-1` Leave).
    pub maria_wrong_direction: bool,
    /// Flag `656`: Reminded Maria about Mary (`-1` Maria).
    pub maria_reminded_about_mary: bool,
    /// Flag `204`: Read Brookhaven Hospital Roof suicidal diary (`+1` In Water).
    pub hospital_roof_diary_read: bool,
    /// Flag `371`: Revisited Maria's Labyrinth room after her death (`+1` Maria).
    pub labyrinth_maria_room_revisited: bool,
    /// Flag `500`: Listened to Hotel 2F Reading Room cassette headphones (`+1` In Water).
    pub hotel_reading_room_tape_heard: bool,
    /// Flag `615`: Read Neely's Bar 2nd visit message in Dark South Vale (`+1` In Water).
    pub neelys_bar_second_message_read: bool,
    /// Flag `630`: Examined Angela's Knife in inventory (`+1` In Water).
    pub angelas_knife_examined: bool,
    /// Flag `629`: Checked on Maria resting in Brookhaven Room S3 a 2nd time (`+1` Maria).
    pub hospital_s3_maria_second_check: bool,
    /// Flag `624`: Listened to Mary's entire Hotel 1F B hallway monologue (`+1` Leave, `+1` In Water).
    pub mary_corridor_monologue_complete: bool,
    /// Count (`0..=4`) of Rebirth ritual items owned (`74, 75, 76, 77`).
    pub rebirth_ritual_items_count: u8,
}

impl EndingEvalFlags {
    /// Extracts the ending evaluation flags directly from [`EventFlags`] and Rebirth item count.
    pub fn from_event_flags(flags: &EventFlags, rebirth_ritual_items_count: u8) -> Self {
        Self {
            maria_wrong_direction: flags.get(FLAG_MARIA_WRONG_DIRECTION),
            maria_reminded_about_mary: flags.get(FLAG_MARIA_REMINDED_ABOUT_MARY),
            hospital_roof_diary_read: flags.get(FLAG_HOSPITAL_ROOF_DIARY),
            labyrinth_maria_room_revisited: flags.get(FLAG_LABYRINTH_MARIA_REVISIT),
            hotel_reading_room_tape_heard: flags.get(FLAG_HOTEL_READING_ROOM_TAPE),
            neelys_bar_second_message_read: flags.get(FLAG_NEELYS_BAR_SECOND_MESSAGE),
            angelas_knife_examined: flags.get(FLAG_ANGELAS_KNIFE_EXAMINED),
            hospital_s3_maria_second_check: flags.get(FLAG_HOSPITAL_S3_MARIA_SECOND_CHECK),
            mary_corridor_monologue_complete: flags.get(FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE),
            rebirth_ritual_items_count,
        }
    }
}

/// State machine for Mary's long corridor monologue (`EvAllTimeFunc` `0x005767e0`, Room `0xba`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum MaryCorridorMonologueState {
    /// Not yet entered Room `0xba`.
    #[default]
    NotEntered,
    /// Waiting out the initial `3.5` s (`MARY_CORRIDOR_START_DELAY_SECS`) delay before `0xead8` starts.
    Delaying { elapsed_secs: f32 },
    /// Playing `0xead8` (`voice.afs[120]`), tracking subtitle frame progress toward `2400.0`.
    Playing { subtitle_frames: f32 },
    /// Exited Room `0xba` before reaching `2400.0` frames (monologue skipped; Flag 624 not awarded).
    SkippedEarly,
    /// Stayed through `2400.0` frames (`80.0` s at 30 Hz); Flag 624 awarded (`+1` Leave, `+1` In Water).
    Completed,
}

/// Detailed score breakdown produced by [`EndingPointTracker::evaluate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndingEvaluationReport {
    /// Final *Leave* point total (`%edi`).
    pub leave_points: i32,
    /// Final *Maria* point total (`%ecx`).
    pub maria_points: i32,
    /// Final *In Water* point total (`%esi`).
    pub in_water_points: i32,
    /// Selected main ending (`Leave`, `Maria`, `InWater`, or `Rebirth`).
    pub selected_ending: MainEndingKind,
}

/// Live accumulator for ending-point telemetry (`0x01f7adcc..0x01f7ae7b` in `sh2pc.exe`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EndingPointTracker {
    /// Cumulative time (`s`) Maria stayed within `< 1000.0` mm (`0x01f7adcc`).
    pub maria_close_secs: f32,
    /// Cumulative time (`s`) Maria was left `> 5000.0` mm behind (`0x01f7add0`).
    pub maria_far_secs: f32,
    /// Cumulative overheal ratio `(hp + heal - hp_max) / hp_max` (`0x01f7add4`).
    pub overheal_ratio_sum: f32,
    /// Cumulative low-HP weighted time `dt * (1 - sin((hp / hp_max) * PI))` (`0x01f7add8`).
    pub low_hp_weighted_secs: f32,
    /// Number of times `Photo of Mary` (`20`) or `Letter from Mary` (`21`) was examined (`0..=2`, `0x01f7ae7a`).
    pub mary_item_examine_count: u8,
    /// Number of times James bumped/shoved Maria (`0..=100`, `0x01f7ae7b`).
    pub maria_bump_count: u8,
    /// Whether `Angela's knife` (`24`) was examined (`Flag 630`).
    pub angelas_knife_examined: bool,
    /// Mary's long hallway monologue progression (`Room 0xba`, `0x005767e0`).
    pub corridor_monologue: MaryCorridorMonologueState,
}

impl EndingPointTracker {
    /// Tick low-HP urgency accumulator (`0x0048eccb..0x0048ed1a` in `sh2pc.exe`).
    pub fn tick_low_hp(&mut self, hp: f32, hp_max: f32, dt_secs: f32) {
        if hp_max <= 0.0 || dt_secs <= 0.0 {
            return;
        }
        let ratio = (hp / hp_max).clamp(0.0, 1.0);
        if ratio < LOW_HP_RATIO_THRESHOLD {
            let weight = 1.0 - (ratio * PI).sin();
            self.low_hp_weighted_secs += dt_secs * weight;
        }
    }

    /// Records healing item use and accumulates excess overheal ratio (`ItemMedicineUse` `0x0049ecf5`).
    pub fn record_heal(&mut self, hp_before: f32, heal_amount: f32, hp_max: f32) -> f32 {
        if hp_max <= 0.0 {
            return hp_before;
        }
        let unclamped = hp_before + heal_amount.max(0.0);
        if unclamped > hp_max {
            self.overheal_ratio_sum += (unclamped - hp_max) / hp_max;
            hp_max
        } else {
            unclamped
        }
    }

    /// Records examining an inventory item (`otn_itemmain` `0x00471ca6..0x0047219b`).
    pub fn record_item_examined(&mut self, item_id: u8) {
        match item_id {
            ITEM_PHOTO_OF_MARY | ITEM_LETTER_FROM_MARY => {
                if self.mary_item_examine_count < 2 {
                    self.mary_item_examine_count += 1;
                }
            }
            ITEM_ANGELAS_KNIFE => {
                self.angelas_knife_examined = true;
            }
            _ => {}
        }
    }

    /// Ticks Maria escort proximity timers and bump counter (`0x0048ed1e..0x0048ed8c`).
    pub fn tick_maria_escort(&mut self, distance_mm: f32, bumped_this_tick: bool, dt_secs: f32) {
        if dt_secs > 0.0 {
            if distance_mm < MARIA_CLOSE_DISTANCE_MM {
                self.maria_close_secs += dt_secs;
            } else if distance_mm > MARIA_FAR_DISTANCE_MM {
                self.maria_far_secs += dt_secs;
            }
        }
        if bumped_this_tick && self.maria_bump_count < MARIA_BUMP_COUNTER_CAP {
            self.maria_bump_count += 1;
        }
    }

    /// Updates Mary's Hotel 1F B hallway monologue (`EvAllTimeFunc` `0x005767e0`, Room `0xba`).
    ///
    /// Returns `true` when `FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE` (`Flag 624`) is newly earned.
    pub fn tick_mary_corridor_monologue(&mut self, current_room: u16, dt_secs: f32) -> bool {
        let in_corridor = current_room == ROOM_HOTEL_CORRIDOR_MONOLOGUE;
        let mut dt = dt_secs.max(0.0);

        if matches!(
            self.corridor_monologue,
            MaryCorridorMonologueState::NotEntered
        ) {
            if !in_corridor {
                return false;
            }
            self.corridor_monologue = MaryCorridorMonologueState::Delaying { elapsed_secs: 0.0 };
        }

        if let MaryCorridorMonologueState::Delaying { mut elapsed_secs } = self.corridor_monologue {
            if !in_corridor {
                self.corridor_monologue = MaryCorridorMonologueState::SkippedEarly;
                return false;
            }
            let needed = (MARY_CORRIDOR_START_DELAY_SECS - elapsed_secs).max(0.0);
            if dt >= needed {
                dt -= needed;
                self.corridor_monologue = MaryCorridorMonologueState::Playing {
                    subtitle_frames: 0.0,
                };
            } else {
                elapsed_secs += dt;
                self.corridor_monologue = MaryCorridorMonologueState::Delaying { elapsed_secs };
                return false;
            }
        }

        if let MaryCorridorMonologueState::Playing {
            mut subtitle_frames,
        } = self.corridor_monologue
        {
            if !in_corridor {
                self.corridor_monologue = MaryCorridorMonologueState::SkippedEarly;
                return false;
            }
            subtitle_frames += dt * 30.0;
            if subtitle_frames >= MARY_CORRIDOR_COMPLETE_SUBTITLE_FRAMES {
                self.corridor_monologue = MaryCorridorMonologueState::Completed;
                return true;
            }
            self.corridor_monologue = MaryCorridorMonologueState::Playing { subtitle_frames };
        }

        false
    }

    /// Evaluates the exact main-scenario ending (`EvProgEndingJudge` at `0x00576490..0x0057672a`).
    pub fn evaluate(
        &self,
        playing: &PlayingInfo,
        mut flags: EndingEvalFlags,
    ) -> EndingEvaluationReport {
        if self.angelas_knife_examined {
            flags.angelas_knife_examined = true;
        }
        if self.corridor_monologue == MaryCorridorMonologueState::Completed {
            flags.mary_corridor_monologue_complete = true;
        }

        let mut leave = LEAVE_START_POINTS;
        let mut maria = MARIA_START_POINTS;
        let mut in_water = IN_WATER_START_POINTS;

        if flags.maria_wrong_direction {
            leave -= 1;
        }
        if flags.maria_reminded_about_mary {
            maria -= 1;
        }
        if flags.hospital_roof_diary_read {
            in_water += 1;
        }
        if flags.labyrinth_maria_room_revisited {
            maria += 1;
        }
        if flags.hotel_reading_room_tape_heard {
            in_water += 1;
        }
        if flags.neelys_bar_second_message_read {
            in_water += 1;
        }
        if flags.angelas_knife_examined {
            in_water += 1;
        }
        if flags.hospital_s3_maria_second_check {
            maria += 1;
        }
        if flags.mary_corridor_monologue_complete {
            leave += 1;
            in_water += 1;
        }
        if self.mary_item_examine_count >= 2 {
            maria -= 1;
        }

        if self.low_hp_weighted_secs > LOW_HP_WEIGHTED_SECS_TIER_1 {
            in_water += 1;
        }
        if self.low_hp_weighted_secs > LOW_HP_WEIGHTED_SECS_TIER_2 {
            in_water += 1;
        }
        if self.overheal_ratio_sum > OVERHEAL_RATIO_THRESHOLD {
            leave += 1;
        }
        if self.maria_close_secs > MARIA_CLOSE_TIME_THRESHOLD_SECS {
            maria += 1;
        }
        if self.maria_far_secs > MARIA_FAR_TIME_THRESHOLD_SECS {
            maria -= 1;
        }
        if self.maria_bump_count > MARIA_BUMP_HIGH_THRESHOLD {
            maria -= 1;
        }
        if self.maria_bump_count < MARIA_BUMP_LOW_THRESHOLD {
            maria += 1;
        }

        let seen = playing.clear_end_kind;
        let seen_leave = seen.contains(ClearEndKind::LEAVE);
        let seen_maria = seen.contains(ClearEndKind::MARIA);
        let seen_water = seen.contains(ClearEndKind::IN_WATER);

        if seen_leave {
            if !seen_maria {
                maria += 1;
            }
            if !seen_water {
                in_water += 1;
            }
        }
        if seen_maria {
            if !seen_leave {
                leave += 1;
            }
            if !seen_water {
                in_water += 1;
            }
        }
        if seen_water {
            if !seen_leave {
                leave += 1;
            }
            if !seen_maria {
                maria += 1;
            }
        }

        let maria_hp_scale = if playing.battle_level <= 1 {
            MARIA_HP_SCALE_EASY
        } else {
            MARIA_HP_SCALE_NORMAL_HARD
        };
        if playing.mar_damage_by_jms > 0.0 {
            maria -= 1;
        }
        if playing.mar_damage_by_jms > LOW_HP_RATIO_THRESHOLD * maria_hp_scale {
            maria -= 1;
        }
        if playing.jms_damage_total <= 1.0
            || (playing.mar_damage_by_enemy / playing.jms_damage_total)
                < MARIA_ENEMY_DAMAGE_RATIO_LIMIT
        {
            maria += 1;
        }

        if flags.rebirth_ritual_items_count >= 4 {
            return EndingEvaluationReport {
                leave_points: leave,
                maria_points: maria,
                in_water_points: in_water,
                selected_ending: MainEndingKind::Rebirth,
            };
        }

        let selected_ending =
            resolve_ending_winner(leave, maria, in_water, seen_leave, seen_maria, seen_water);

        EndingEvaluationReport {
            leave_points: leave,
            maria_points: maria,
            in_water_points: in_water,
            selected_ending,
        }
    }
}

/// Exact tie-breaker logic from `0x00576677..0x0057672a` in `sh2pc.exe`.
fn resolve_ending_winner(
    leave: i32,
    maria: i32,
    in_water: i32,
    seen_leave: bool,
    seen_maria: bool,
    seen_water: bool,
) -> MainEndingKind {
    if !seen_leave && leave >= maria && leave >= in_water {
        return MainEndingKind::Leave;
    }
    if !seen_maria && maria >= leave && maria >= in_water {
        return MainEndingKind::Maria;
    }
    if !seen_water && in_water >= leave && in_water >= maria {
        return MainEndingKind::InWater;
    }

    if leave > maria && leave > in_water {
        return MainEndingKind::Leave;
    }
    if maria > leave && maria > in_water {
        return MainEndingKind::Maria;
    }
    if in_water > leave && in_water > maria {
        return MainEndingKind::InWater;
    }

    if leave == maria && leave > in_water {
        return MainEndingKind::Leave;
    }
    if leave == in_water && leave > maria {
        return MainEndingKind::Leave;
    }
    if maria == in_water && maria > leave {
        return MainEndingKind::Maria;
    }
    MainEndingKind::Leave
}
