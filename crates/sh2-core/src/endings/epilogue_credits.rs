//! Epilogue Mary Letter Readings, Cemetery Draw Distance (`LeaveEndingDrawDistance.cpp`), and Per-Ending Staff Credits Roll.
//!
//! Verified 1:1 against:
//! - PS2 Prototype ELF (`SLUS_202.28`):
//!   - `stg_end_recovery.c` (`EvProgLastScene` `0x01f02750` / `0x01f02900`), `stg_end_maria.c`,
//!     `stg_end_suicide.c`, `stg_end_rebirth.c`, `stg_end_dog.c`.
//! - Retail PC Binary (`sh2pc.exe` v1.0 Director's Cut):
//!   - Leave (`0x00440070` -> Stage 4 `0x0059f75b..0x0059fa30`, `letter_msg_e.mes` + `0xEAE1` (`voice.afs[129]`, `300.0` s),
//!     `ending.sfd` -> `end.bik` track `3` + `credits.sfd` -> `credits.bik`, cemetery draw distance `30000.0` at `0x008e7878`).
//!   - Maria (`0x00440d80` -> `0x005a0f3e..0x005a12d0`, `letter_w1_msg_e.mes` / `letter_msg_e.mes` + `0xEA64` (`voice.afs[4]`, `351.021` s),
//!     `end_mria.sfd` -> `end.bik` track `0` + `credits.sfd` -> `credits.bik`).
//!   - In Water (`0x0043ed20..0x0043f280`, `36.0` s bedside scene `0x006388a4` -> `water.bik` (`290.290` s) ->
//!     `25.0` s closing scroll `0x0063889c` (`letter_w2_msg_e.mes` / `letter_w0_msg_e.mes`) + `0xEAB2` (`voice.afs[82]`) at `15.0` s `0x006315e0`,
//!     `end_scde.sfd` -> `end.bik` track `2` + `credits.sfd` -> `credits.bik`).
//!   - Rebirth (`0x00574930` -> `0x0057dc00..0x0057dfa0`, `end_rbrt.sfd` -> `end.bik` track `1` + `credits.sfd` -> `credits.bik`).
//!   - Dog (`0x00574430..0x00574600`, `end_dog.sfd` -> `end_dog.bik` + `credits.sfd` -> `credits.bik`).
//!   - UFO (`0x00578c50..0x00578f63`, stream `0xEAFC`, `staff01.tex..staff07.tex`).
//!   - *Born from a Wish* (`0x00570d20` -> `0x00573a80..0x00573bd0`, `end_wish.sfd` -> `end_wish.bik` + `credits.sfd` -> `credits.bik`).
//! - Silent Hill 2: Enhanced Edition:
//!   - `Patches/LeaveEndingDrawDistance.cpp` (`0x008e7878` on `SH2V_10`: `30000.0` vanilla vs `40000.0` SH2:EE).

use super::letter_scroll::{
    MaryLetterScrollEntry, LEAVE_LETTER_VOICE_DELAY_SECS, LETTER_MSG_E_ENTRIES,
    LETTER_W1_MSG_E_ENTRIES, LETTER_W2_MSG_E_ENTRIES, MARIA_LETTER_VOICE_DELAY_SECS,
    SE_IN_WATER_DROWNING_STREAM, SE_LEAVE_LETTER_VOICEOVER, SE_MARIA_LETTER_VOICEOVER,
};
use super::playing_info::GameEnding;

/// Vanilla PS2 / PC Leave ending cemetery camera far clip (`30000.0` mm at `0x008e7878` in `sh2pc.exe` `SH2V_10`).
pub const LEAVE_CEMETERY_VANILLA_DRAW_DISTANCE_MM: f32 = 30_000.0;
/// Extended SH2:EE Leave ending cemetery camera far clip (`40000.0` mm in `LeaveEndingDrawDistance.cpp`)
/// preventing distant trees from popping in behind James and Laura.
pub const LEAVE_CEMETERY_ENHANCED_DRAW_DISTANCE_MM: f32 = 40_000.0;

/// Resolves the Leave ending cemetery far-clip distance (`LeaveEndingDrawDistance.cpp`, `0x008e7878`).
///
/// - `enable_extended_draw_distance == false`: `30000.0` mm (strict 1:1 vanilla PS2/PC).
/// - `enable_extended_draw_distance == true`: `40000.0` mm (SH2:EE pop-in fix).
pub const fn leave_ending_cemetery_draw_distance(enable_extended_draw_distance: bool) -> f32 {
    if enable_extended_draw_distance {
        LEAVE_CEMETERY_ENHANCED_DRAW_DISTANCE_MM
    } else {
        LEAVE_CEMETERY_VANILLA_DRAW_DISTANCE_MM
    }
}

/// Authentic Akira Yamaoka musical theme paired with each ending's staff credits roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsMusicTheme {
    /// *Overdose Delusion* (*Leave* ending, `end.bik` track `3`).
    OverdoseDelusion,
    /// *Promise* (*Maria* ending, `end.bik` track `0`).
    Promise,
    /// *Angel's Thanatos* (*In Water* ending, `end.bik` track `2` / `end/bgm_002.adx`).
    AngelsThanatos,
    /// *The Reverse Will* (*Rebirth* ending, `end.bik` track `1`).
    TheReverseWill,
    /// *Dog Ending Theme* / Mira blooper reel (`end_dog.bik` track `0`).
    DogBlooperPolka,
    /// *Silent Hill* UFO silent-film arrangement (`voice.afs[156]`, `0xEAFC`).
    SilentHillUfoArrangement,
    /// *Love Psalm* (*Born from a Wish* ending, `end_wish.bik` track `0`).
    LovePsalm,
}

impl CreditsMusicTheme {
    /// Human-readable soundtrack title from the Silent Hill 2 Original Soundtrack.
    pub const fn title(self) -> &'static str {
        match self {
            Self::OverdoseDelusion => "Overdose Delusion",
            Self::Promise => "Promise",
            Self::AngelsThanatos => "Angel's Thanatos",
            Self::TheReverseWill => "The Reverse Will",
            Self::DogBlooperPolka => "Dog Ending (Inu no End)",
            Self::SilentHillUfoArrangement => "Silent Hill (UFO Arrangement)",
            Self::LovePsalm => "Love Psalm",
        }
    }
}

/// Presentation style of an ending's epilogue and staff credits sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsPresentationKind {
    /// Full Mary's letter upward scroll (`0x00482f60`) followed by `end.bik` epilogue & `credits.bik` staff roll (*Leave*, *Maria*, *In Water*).
    MaryLetterThenScrollingRoll,
    /// Rebirth ritual boat FMV (`end_rbrt.sfd` -> `end.bik` track `1`) followed by `credits.bik` staff roll (`The Reverse Will`).
    RitualSceneThenScrollingRoll,
    /// Mira the Shiba Inu control-room FMV + comedy blooper reel (`end_dog.bik` + `credits.bik`).
    DogBlooperReel,
    /// Silent-film comic slideshow (`hj01..07.tex`) transitioning into 7 static staff cards (`staff01..07.tex`).
    UfoSilentFilmSlides,
    /// Maria *Born from a Wish* epilogue (`end_wish.bik`) + *Love Psalm* staff roll (`credits.bik`).
    BornFromAWishRoll,
}

/// Complete epilogue + staff credits descriptor for one of the 7 endings (`0x0043ed20..0x005a12d0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndingCreditsDescriptor {
    /// Which ending this sequence belongs to.
    pub ending: GameEnding,
    /// Whether Mary reads her complete letter in full (`true` for *Leave*, *In Water*, and *Maria*).
    pub plays_full_mary_letter: bool,
    /// Retail `.mes` file scrolled during the epilogue letter phase (`0x004457c0` cases `7`, `8`, `0xb`, `0xc`/`0xd`).
    pub letter_mes_path: Option<&'static str>,
    /// Voice stream ID (`0xEA60 + voice.afs` index) triggered during the `.mes` scroll (`0xEAE1`, `0xEA64`, or `0xEAB2`).
    pub letter_voice_stream_id: Option<u32>,
    /// Delay in seconds before `letter_voice_stream_id` fires (`0.8` s Leave, `0.0` s Maria, `15.0` s In Water closing scroll).
    pub letter_voice_delay_secs: f32,
    /// Optional pre-epilogue Bink FMV (`"water.bik"` on *In Water* at `0x0043efa9`).
    pub prelude_fmv: Option<&'static str>,
    /// Original PS2 `.sfd` stream identifier (`"ending.sfd"`, `"end_mria.sfd"`, `"end_scde.sfd"`, `"end_rbrt.sfd"`, `"end_dog.sfd"`, `"end_wish.sfd"`).
    pub ps2_epilogue_sfd: Option<&'static str>,
    /// Retail PC Bink FMV container (`"end.bik"` with 4 muxed audio tracks `0..=3` mapped at `0x0043d754..0x0043d809`, `"end_dog.bik"`, `"end_wish.bik"`).
    pub epilogue_fmv: Option<&'static str>,
    /// Final Konami/Yamaoka staff credits Bink FMV (`"credits.bik"`, or `None` for UFO `.tex` slides).
    pub credits_fmv: Option<&'static str>,
    /// Audio track index in `end.bik` (`0` Maria, `1` Rebirth, `2` In Water, `3` Leave; `0x0043d754..0x0043d809`).
    pub end_bik_audio_track: usize,
    /// Authentic Akira Yamaoka credits music theme.
    pub music_theme: CreditsMusicTheme,
    /// Visual presentation mode for `sh2-ui::credits`.
    pub presentation: CreditsPresentationKind,
}

impl EndingCreditsDescriptor {
    /// Returns the static `.mes` entries scrolled during this ending's letter phase (`0x00482f60`).
    pub const fn letter_scroll_entries(self) -> &'static [MaryLetterScrollEntry] {
        match self.ending {
            GameEnding::Leave => &LETTER_MSG_E_ENTRIES,
            GameEnding::Maria => LETTER_W1_MSG_E_ENTRIES,
            GameEnding::InWater => &LETTER_W2_MSG_E_ENTRIES,
            GameEnding::Rebirth | GameEnding::Dog | GameEnding::Ufo | GameEnding::BornFromAWish => {
                &[]
            }
        }
    }
}

/// Canonical per-ending epilogue & credits table covering all 7 endings (`0x0043ed20..0x005a12d0`).
pub const ENDING_CREDITS_TABLE: [EndingCreditsDescriptor; 7] = [
    EndingCreditsDescriptor {
        ending: GameEnding::Leave,
        plays_full_mary_letter: true,
        letter_mes_path: Some("data/etc/message/letter_msg_e.mes"),
        letter_voice_stream_id: Some(SE_LEAVE_LETTER_VOICEOVER),
        letter_voice_delay_secs: LEAVE_LETTER_VOICE_DELAY_SECS,
        prelude_fmv: None,
        ps2_epilogue_sfd: Some("ending.sfd"),
        epilogue_fmv: Some("end.bik"),
        credits_fmv: Some("credits.bik"),
        end_bik_audio_track: 3,
        music_theme: CreditsMusicTheme::OverdoseDelusion,
        presentation: CreditsPresentationKind::MaryLetterThenScrollingRoll,
    },
    EndingCreditsDescriptor {
        ending: GameEnding::Maria,
        plays_full_mary_letter: true,
        letter_mes_path: Some("data/etc/message/letter_w1_msg_e.mes"),
        letter_voice_stream_id: Some(SE_MARIA_LETTER_VOICEOVER),
        letter_voice_delay_secs: MARIA_LETTER_VOICE_DELAY_SECS,
        prelude_fmv: None,
        ps2_epilogue_sfd: Some("end_mria.sfd"),
        epilogue_fmv: Some("end.bik"),
        credits_fmv: Some("credits.bik"),
        end_bik_audio_track: 0,
        music_theme: CreditsMusicTheme::Promise,
        presentation: CreditsPresentationKind::MaryLetterThenScrollingRoll,
    },
    EndingCreditsDescriptor {
        ending: GameEnding::InWater,
        plays_full_mary_letter: true,
        letter_mes_path: Some("data/etc/message/letter_w0_msg_e.mes"),
        letter_voice_stream_id: Some(SE_IN_WATER_DROWNING_STREAM),
        letter_voice_delay_secs: 15.0,
        prelude_fmv: Some("water.bik"),
        ps2_epilogue_sfd: Some("end_scde.sfd"),
        epilogue_fmv: Some("end.bik"),
        credits_fmv: Some("credits.bik"),
        end_bik_audio_track: 2,
        music_theme: CreditsMusicTheme::AngelsThanatos,
        presentation: CreditsPresentationKind::MaryLetterThenScrollingRoll,
    },
    EndingCreditsDescriptor {
        ending: GameEnding::Rebirth,
        plays_full_mary_letter: false,
        letter_mes_path: None,
        letter_voice_stream_id: None,
        letter_voice_delay_secs: 0.0,
        prelude_fmv: None,
        ps2_epilogue_sfd: Some("end_rbrt.sfd"),
        epilogue_fmv: Some("end.bik"),
        credits_fmv: Some("credits.bik"),
        end_bik_audio_track: 1,
        music_theme: CreditsMusicTheme::TheReverseWill,
        presentation: CreditsPresentationKind::RitualSceneThenScrollingRoll,
    },
    EndingCreditsDescriptor {
        ending: GameEnding::Dog,
        plays_full_mary_letter: false,
        letter_mes_path: None,
        letter_voice_stream_id: None,
        letter_voice_delay_secs: 0.0,
        prelude_fmv: None,
        ps2_epilogue_sfd: Some("end_dog.sfd"),
        epilogue_fmv: Some("end_dog.bik"),
        credits_fmv: Some("credits.bik"),
        end_bik_audio_track: 0,
        music_theme: CreditsMusicTheme::DogBlooperPolka,
        presentation: CreditsPresentationKind::DogBlooperReel,
    },
    EndingCreditsDescriptor {
        ending: GameEnding::Ufo,
        plays_full_mary_letter: false,
        letter_mes_path: None,
        letter_voice_stream_id: None,
        letter_voice_delay_secs: 0.0,
        prelude_fmv: None,
        ps2_epilogue_sfd: None,
        epilogue_fmv: None,
        credits_fmv: None,
        end_bik_audio_track: 0,
        music_theme: CreditsMusicTheme::SilentHillUfoArrangement,
        presentation: CreditsPresentationKind::UfoSilentFilmSlides,
    },
    EndingCreditsDescriptor {
        ending: GameEnding::BornFromAWish,
        plays_full_mary_letter: false,
        letter_mes_path: None,
        letter_voice_stream_id: None,
        letter_voice_delay_secs: 0.0,
        prelude_fmv: None,
        ps2_epilogue_sfd: Some("end_wish.sfd"),
        epilogue_fmv: Some("end_wish.bik"),
        credits_fmv: Some("credits.bik"),
        end_bik_audio_track: 0,
        music_theme: CreditsMusicTheme::LovePsalm,
        presentation: CreditsPresentationKind::BornFromAWishRoll,
    },
];

/// Looks up the canonical [`EndingCreditsDescriptor`] for `ending`.
pub const fn credits_descriptor_for_ending(ending: GameEnding) -> EndingCreditsDescriptor {
    match ending {
        GameEnding::Leave => ENDING_CREDITS_TABLE[0],
        GameEnding::Maria => ENDING_CREDITS_TABLE[1],
        GameEnding::InWater => ENDING_CREDITS_TABLE[2],
        GameEnding::Rebirth => ENDING_CREDITS_TABLE[3],
        GameEnding::Dog => ENDING_CREDITS_TABLE[4],
        GameEnding::Ufo => ENDING_CREDITS_TABLE[5],
        GameEnding::BornFromAWish => ENDING_CREDITS_TABLE[6],
    }
}

/// Canonical Team Silent key staff credits sections for the scrolling staff roll (`credits.bik` / UI fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaffCreditsSection {
    /// Role / department heading.
    pub role: &'static str,
    /// Credited Team Silent staff names.
    pub names: &'static [&'static str],
}

/// Authentic Team Silent core staff sections presented during the Silent Hill 2 staff roll.
pub const TEAM_SILENT_STAFF_SECTIONS: [StaffCreditsSection; 10] = [
    StaffCreditsSection {
        role: "DIRECTOR",
        names: &["Masashi Tsuboyama"],
    },
    StaffCreditsSection {
        role: "SCENARIO & EVENT DIRECTOR",
        names: &["Hiroyuki Owaku"],
    },
    StaffCreditsSection {
        role: "ART DIRECTOR & CREATURE DESIGNER",
        names: &["Masahiro Ito"],
    },
    StaffCreditsSection {
        role: "CHARACTER DESIGNER & CGI DIRECTOR",
        names: &["Takayoshi Sato"],
    },
    StaffCreditsSection {
        role: "SOUND DIRECTOR & MUSIC COMPOSER",
        names: &["Akira Yamaoka"],
    },
    StaffCreditsSection {
        role: "CHIEF PROGRAMMER",
        names: &["Norio Takemoto"],
    },
    StaffCreditsSection {
        role: "DRAMA DIRECTOR",
        names: &["Suguru Murakoshi"],
    },
    StaffCreditsSection {
        role: "VOICE CAST",
        names: &[
            "Guy Cihi (James Sunderland)",
            "Monica Taylor Horgan (Mary / Maria)",
            "Donna Burke (Angela Orosco)",
            "David Schaufele (Eddie Dombrowski)",
            "Jacquelyn Breckenridge (Laura)",
        ],
    },
    StaffCreditsSection {
        role: "PRODUCER",
        names: &["Akihiro Imamura"],
    },
    StaffCreditsSection {
        role: "PRODUCED BY",
        names: &["Konami Computer Entertainment Tokyo"],
    },
];
