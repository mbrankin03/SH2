use sh2_core::AudioPackSource;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutsceneBgmAction {
    StartNew,
    KeepActive,
    Silence,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CutsceneAudioMapping {
    pub id: &'static str,
    pub fmv_filename: &'static str,
    pub default_bik_track: usize,
    pub bgm_file: Option<&'static str>,
    pub stage_id: u32,
    pub bgm_page: u32,
    pub start_id: u32,
    pub retail_master_volume: f32,
    pub fade_in: Duration,
    pub fade_out: Duration,
    pub bgm_action: CutsceneBgmAction,
    pub description: &'static str,
}

pub const fn master_volume_for_start_id(start_id: u32) -> f32 {
    match start_id {
        50115 | 50101 | 50106 => 0.50,
        50021 | 50003 | 50017 => 0.25,
        50013 => 0.30,
        50102 | 50105 => 0.45,
        50125 | 50124 | 50016 => 0.55,
        50004 => 0.90,
        50001 => 0.70,
        50104 => 0.35,
        _ => 0.40,
    }
}

pub const CUTSCENE_AUDIO_MAP: &[CutsceneAudioMapping] = &[
    CutsceneAudioMapping {
        id: "toilet",
        fmv_filename: "toilet.bik",
        default_bik_track: 0,
        bgm_file: Some("forest/bgm_114_a.adx"),
        stage_id: 2,
        bgm_page: 43,
        start_id: 50129,
        retail_master_volume: 0.40,
        fade_in: Duration::from_millis(500),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::StartNew,
        description: "Restroom mirror monologue / Observation Deck - Theme of Laura (Reprise)",
    },
    CutsceneAudioMapping {
        id: "open",
        fmv_filename: "open.bik",
        default_bik_track: 1,
        bgm_file: None,
        stage_id: 0,
        bgm_page: 0,
        start_id: 0,
        retail_master_volume: 1.0,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_secs(2),
        bgm_action: CutsceneBgmAction::Silence,
        description: "Opening Attract Movie - Theme of Laura (Track 1 Voiced / Track 0 Instrumental)",
    },
    CutsceneAudioMapping {
        id: "hakaba",
        fmv_filename: "hakaba.bik",
        default_bik_track: 0,
        bgm_file: Some("forest/bgm_022.adx"),
        stage_id: 4,
        bgm_page: 0,
        start_id: 50022,
        retail_master_volume: 0.40,
        fade_in: Duration::from_millis(400),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::StartNew,
        description: "Toluca Graveyard: First meeting with Angela Orosco - Forest / Cemetery Theme",
    },
    CutsceneAudioMapping {
        id: "gero",
        fmv_filename: "gero.bik",
        default_bik_track: 0,
        bgm_file: Some("apart/bgm_001.adx"),
        stage_id: 6,
        bgm_page: 4,
        start_id: 50000,
        retail_master_volume: 0.40,
        fade_in: Duration::from_millis(300),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::StartNew,
        description: "Wood Side Apartments Rm 101: Meeting Eddie Dombrowski - First Meeting Eddie",
    },
    CutsceneAudioMapping {
        id: "hei",
        fmv_filename: "hei.bik",
        default_bik_track: 0,
        bgm_file: Some("apart/bgm_118.aix"),
        stage_id: 8,
        bgm_page: 2,
        start_id: 50118,
        retail_master_volume: 0.40,
        fade_in: Duration::from_millis(250),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::KeepActive,
        description: "Blue Creek Courtyard: Laura steps on James hand - Apartment ambient BGM continues",
    },
    CutsceneAudioMapping {
        id: "knife",
        fmv_filename: "knife.bik",
        default_bik_track: 0,
        bgm_file: Some("apart/bgm_014.adx"),
        stage_id: 7,
        bgm_page: 5,
        start_id: 50013,
        retail_master_volume: 0.30,
        fade_in: Duration::from_millis(300),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::StartNew,
        description: "Blue Creek Rm 109: Angela with suicidal thoughts & knife - Pianissimo Epilogue",
    },
    CutsceneAudioMapping {
        id: "deai",
        fmv_filename: "deai.bik",
        default_bik_track: 0,
        bgm_file: Some("town/bgm_004.adx"),
        stage_id: 11,
        bgm_page: 10,
        start_id: 50004,
        retail_master_volume: 0.90,
        fade_in: Duration::from_millis(300),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::StartNew,
        description: "Rosewater Park: First meeting with Maria - Promise (Reprise)",
    },
    CutsceneAudioMapping {
        id: "flash",
        fmv_filename: "flash.bik",
        default_bik_track: 0,
        bgm_file: Some("town/bgm_004.adx"),
        stage_id: 11,
        bgm_page: 10,
        start_id: 50004,
        retail_master_volume: 0.90,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_millis(500),
        bgm_action: CutsceneBgmAction::KeepActive,
        description: "Flashback memory of Mary in hospital bed - Interleaved inside Maria meeting",
    },
    CutsceneAudioMapping {
        id: "saikai",
        fmv_filename: "saikai.bik",
        default_bik_track: 0,
        bgm_file: Some("hospital/bgm_100.aix"),
        stage_id: 16,
        bgm_page: 18,
        start_id: 50100,
        retail_master_volume: 0.40,
        fade_in: Duration::from_millis(200),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::KeepActive,
        description: "Brookhaven Hospital Basement: Reunited with Maria behind gate",
    },
    CutsceneAudioMapping {
        id: "rouya",
        fmv_filename: "rouya.bik",
        default_bik_track: 0,
        bgm_file: Some("prison/bgm_116.aix"),
        stage_id: 24,
        bgm_page: 24,
        start_id: 50116,
        retail_master_volume: 0.50,
        fade_in: Duration::from_millis(200),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::KeepActive,
        description: "Labyrinth cell: Maria behind bars - Labyrinth Cell Ambient",
    },
    CutsceneAudioMapping {
        id: "korosu_a",
        fmv_filename: "korosu_a.bik",
        default_bik_track: 0,
        bgm_file: Some("prison/bgm_015.adx"),
        stage_id: 23,
        bgm_page: 25,
        start_id: 50014,
        retail_master_volume: 0.40,
        fade_in: Duration::from_millis(200),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::StartNew,
        description: "Prison meat locker: Eddie standing over human corpse - Eddie Killer Theme A",
    },
    CutsceneAudioMapping {
        id: "korosu_b",
        fmv_filename: "korosu_b.bik",
        default_bik_track: 0,
        bgm_file: Some("prison/bgm_015.adx"),
        stage_id: 23,
        bgm_page: 25,
        start_id: 50014,
        retail_master_volume: 0.40,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::KeepActive,
        description: "Prison meat locker: Eddie brandishing revolver - Eddie Killer Theme B",
    },
    CutsceneAudioMapping {
        id: "murder",
        fmv_filename: "murder.bik",
        default_bik_track: 1,
        bgm_file: Some("hotel/bgm_009.adx"),
        stage_id: 34,
        bgm_page: 34,
        start_id: 50008,
        retail_master_volume: 0.40,
        fade_in: Duration::from_millis(300),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::StartNew,
        description: "Lakeview Hotel Rm 312: James watches the videotape - The Truth / Day of Night",
    },
    CutsceneAudioMapping {
        id: "water",
        fmv_filename: "water.bik",
        default_bik_track: 0,
        bgm_file: Some("end/bgm_002.adx"),
        stage_id: 50,
        bgm_page: 39,
        start_id: 50001,
        retail_master_volume: 0.70,
        fade_in: Duration::from_millis(200),
        fade_out: Duration::from_secs(4),
        bgm_action: CutsceneBgmAction::StartNew,
        description: "In Water Ending: James drives car into Toluca Lake - Angel Thanatos",
    },
    CutsceneAudioMapping {
        id: "end",
        fmv_filename: "end.bik",
        default_bik_track: 0,
        bgm_file: None,
        stage_id: 48,
        bgm_page: 0,
        start_id: 0,
        retail_master_volume: 1.0,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::Silence,
        description: "Ending Staff Roll: Track 0 Maria (Promise), Track 1 Rebirth, Track 2 Water, Track 3 Leave",
    },
    CutsceneAudioMapping {
        id: "end_dog",
        fmv_filename: "end_dog.bik",
        default_bik_track: 0,
        bgm_file: None,
        stage_id: 52,
        bgm_page: 0,
        start_id: 0,
        retail_master_volume: 1.0,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_secs(2),
        bgm_action: CutsceneBgmAction::Silence,
        description: "Dog Ending Credits: Mira the Shiba Inu control room - Comedic polka music",
    },
    CutsceneAudioMapping {
        id: "end_wish",
        fmv_filename: "end_wish.bik",
        default_bik_track: 0,
        bgm_file: None,
        stage_id: 58,
        bgm_page: 0,
        start_id: 0,
        retail_master_volume: 1.0,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::Silence,
        description: "Born From A Wish Ending: Maria resolves to meet James at Rosewater Park",
    },
    CutsceneAudioMapping {
        id: "credits",
        fmv_filename: "credits.bik",
        default_bik_track: 0,
        bgm_file: None,
        stage_id: 48,
        bgm_page: 0,
        start_id: 0,
        retail_master_volume: 1.0,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_secs(3),
        bgm_action: CutsceneBgmAction::Silence,
        description: "Final Credits Roll: Akira Yamaoka post-game staff crawl",
    },
    CutsceneAudioMapping {
        id: "sh2e3",
        fmv_filename: "sh2e3.bik",
        default_bik_track: 0,
        bgm_file: None,
        stage_id: 0,
        bgm_page: 0,
        start_id: 0,
        retail_master_volume: 1.0,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_secs(2),
        bgm_action: CutsceneBgmAction::Silence,
        description: "Silent Hill 2 Tokyo Game Show & E3 2001 Reveal Exhibition Trailer",
    },
    CutsceneAudioMapping {
        id: "sh3e3",
        fmv_filename: "sh3e3.bik",
        default_bik_track: 0,
        bgm_file: None,
        stage_id: 0,
        bgm_page: 0,
        start_id: 0,
        retail_master_volume: 1.0,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_secs(2),
        bgm_action: CutsceneBgmAction::Silence,
        description: "Silent Hill 3 E3 2002 Announcement Trailer - Youre Not Here",
    },
    CutsceneAudioMapping {
        id: "sh3tgs",
        fmv_filename: "sh3tgs.bik",
        default_bik_track: 0,
        bgm_file: None,
        stage_id: 0,
        bgm_page: 0,
        start_id: 0,
        retail_master_volume: 1.0,
        fade_in: Duration::from_millis(100),
        fade_out: Duration::from_secs(2),
        bgm_action: CutsceneBgmAction::Silence,
        description: "Silent Hill 3 Tokyo Game Show 2002 Teaser Trailer",
    },
];

pub fn find_cutscene_audio_mapping(id_or_filename: &str) -> Option<&'static CutsceneAudioMapping> {
    let clean = id_or_filename
        .trim()
        .trim_end_matches(".bik")
        .trim_end_matches(".sfd");
    CUTSCENE_AUDIO_MAP.iter().find(|m| {
        m.id.eq_ignore_ascii_case(clean) || m.fmv_filename.eq_ignore_ascii_case(id_or_filename)
    })
}

fn resolve_case_insensitive_in_dir(base_dir: &Path, rel_path: &str) -> Option<PathBuf> {
    let clean = rel_path.trim_start_matches(['/', '\\']).replace('\\', "/");
    let direct = base_dir.join(&clean);
    if direct.exists() {
        return Some(direct);
    }

    // Fallback: case-insensitive directory and filename traversal for Linux filesystems
    let mut current = base_dir.to_path_buf();
    for part in clean.split('/').filter(|s| !s.is_empty()) {
        let next = current.join(part);
        if next.exists() {
            current = next;
            continue;
        }
        let entries = std::fs::read_dir(&current).ok()?;
        let mut matched = None;
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            if file_name.to_string_lossy().eq_ignore_ascii_case(part) {
                matched = Some(entry.path());
                break;
            }
        }
        current = matched?;
    }

    if current.exists() {
        Some(current)
    } else {
        None
    }
}

/// Resolves a BGM `.adx` or `.aix` stream path according to the requested [`AudioPackSource`]
/// (`Auto` / `Sh2eeLossless` prefers `sh2e/sound/adx` at 48 kHz; `Ps2Original` prefers `data/sound/adx`).
pub fn resolve_bgm_stream_path_for_source(
    game_root: impl AsRef<Path>,
    rel_bgm_path: &str,
    source: AudioPackSource,
) -> Option<PathBuf> {
    let root = game_root.as_ref();
    let ee_dir = root.join("sh2e").join("sound").join("adx");
    let data_dir = root.join("data").join("sound").join("adx");
    let direct_dir = root.join("sound").join("adx");

    let search_dirs = match source {
        AudioPackSource::Auto | AudioPackSource::Sh2eeLossless => [ee_dir, data_dir, direct_dir],
        AudioPackSource::Ps2Original => [data_dir, ee_dir, direct_dir],
    };

    for dir in &search_dirs {
        if let Some(found) = resolve_case_insensitive_in_dir(dir, rel_bgm_path) {
            return Some(found);
        }
    }
    None
}

pub fn resolve_bgm_stream_path(game_root: impl AsRef<Path>, rel_bgm_path: &str) -> Option<PathBuf> {
    resolve_bgm_stream_path_for_source(game_root, rel_bgm_path, AudioPackSource::EnhancedLossless)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cutscene_mapping_entries_and_lookup() {
        assert_eq!(CUTSCENE_AUDIO_MAP.len(), 21);

        let toilet = find_cutscene_audio_mapping("toilet").expect("toilet not found");
        assert_eq!(toilet.fmv_filename, "toilet.bik");
        assert_eq!(toilet.bgm_file, Some("forest/bgm_114_a.adx"));
        assert_eq!(toilet.bgm_action, CutsceneBgmAction::StartNew);

        let open = find_cutscene_audio_mapping("open.bik").expect("open not found");
        assert_eq!(open.default_bik_track, 1);
        assert_eq!(open.bgm_action, CutsceneBgmAction::Silence);

        let murder = find_cutscene_audio_mapping("murder").expect("murder not found");
        assert_eq!(murder.default_bik_track, 1);
        assert_eq!(murder.bgm_file, Some("hotel/bgm_009.adx"));

        let deai = find_cutscene_audio_mapping("deai").expect("deai not found");
        assert_eq!(deai.bgm_file, Some("town/bgm_004.adx"));
        assert_eq!(deai.retail_master_volume, 0.90);
    }
}
