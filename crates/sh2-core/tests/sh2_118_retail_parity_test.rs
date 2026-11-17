//! Byte-for-byte retail PC Director's Cut (`sh2pc.exe`, `letter*_msg_e.mes`, `voice.afs`, `data/movie/*.bik`)
//! verification tests for `SH2-118`.

use std::fs;
use std::path::{Path, PathBuf};

use sh2_core::endings::*;

fn find_retail_game_root() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
        PathBuf::from("../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
    ];
    candidates.into_iter().find(|p| p.join("data").is_dir())
}

#[test]
fn letter_mes_scroll_bik_headers_voice_afs_and_sh2pc_match_retail_rip() {
    assert_eq!(leave_ending_cemetery_draw_distance(false), 30_000.0);
    assert_eq!(leave_ending_cemetery_draw_distance(true), 40_000.0);
    assert_eq!(LETTER_MSG_E_ENTRIES.len(), 35);
    assert_eq!(LETTER_W1_MSG_E_ENTRIES.len(), 33);
    assert_eq!(LETTER_W2_MSG_E_ENTRIES.len(), 2);
    assert_eq!(LETTER_W0_MSG_E_ENTRIES.len(), 35);
    assert!(
        (letter_scroll_total_duration_secs(&LETTER_MSG_E_ENTRIES) - 5265.0 / 16.5).abs() < 1e-4
    );

    assert_eq!(
        credits_descriptor_for_ending(GameEnding::Leave).letter_voice_stream_id,
        Some(SE_LEAVE_LETTER_VOICEOVER)
    );
    assert_eq!(
        credits_descriptor_for_ending(GameEnding::Maria).letter_voice_stream_id,
        Some(SE_MARIA_LETTER_VOICEOVER)
    );
    assert_eq!(
        credits_descriptor_for_ending(GameEnding::InWater).letter_voice_stream_id,
        Some(SE_IN_WATER_DROWNING_STREAM)
    );

    let Some(root) = find_retail_game_root() else {
        return;
    };

    // 1. Verify all 4 retail letter*_msg_e.mes files byte-for-byte against static tables
    let mes_cases: [(&str, &[MaryLetterScrollEntry]); 4] = [
        ("data/etc/message/letter_msg_e.mes", &LETTER_MSG_E_ENTRIES),
        (
            "data/etc/message/letter_w1_msg_e.mes",
            LETTER_W1_MSG_E_ENTRIES,
        ),
        (
            "data/etc/message/letter_w2_msg_e.mes",
            &LETTER_W2_MSG_E_ENTRIES,
        ),
        (
            "data/etc/message/letter_w0_msg_e.mes",
            &LETTER_W0_MSG_E_ENTRIES,
        ),
    ];
    for (rel, expected) in mes_cases {
        let bytes = fs::read(root.join(rel)).expect("read retail letter .mes");
        let parsed = parse_letter_mes_entries(&bytes).expect("parse retail letter .mes");
        assert_eq!(
            parsed.len(),
            expected.len(),
            "entry count mismatch in {rel}"
        );
        for (i, (got, exp)) in parsed.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                got.f8_spacing_px, exp.f8_spacing_px,
                "{rel}[{i}] f8 spacing mismatch"
            );
            assert_eq!(got.text, exp.text, "{rel}[{i}] text mismatch");
        }
    }

    // 2. Verify all 5 retail .bik headers match exact frame/FPS duration constants
    let bik_cases = [
        ("data/movie/credits.bik", CREDITS_BIK_DURATION_SECS),
        ("data/movie/end.bik", END_BIK_DURATION_SECS),
        ("data/movie/end_dog.bik", END_DOG_BIK_DURATION_SECS),
        ("data/movie/end_wish.bik", END_WISH_BIK_DURATION_SECS),
        ("data/movie/water.bik", WATER_BIK_DURATION_SECS),
    ];
    for (rel, expected_secs) in bik_cases {
        let hdr = fs::read(root.join(rel)).expect("read retail .bik");
        let frames = u32::from_le_bytes(hdr[8..12].try_into().expect("frames")) as f32;
        let fps_num = u32::from_le_bytes(hdr[28..32].try_into().expect("fps_num")) as f32;
        let fps_den = u32::from_le_bytes(hdr[32..36].try_into().expect("fps_den")) as f32;
        let actual_secs = frames * fps_den / fps_num;
        assert!(
            (actual_secs - expected_secs).abs() < 1e-4,
            "{rel} duration mismatch: got {actual_secs}, expected {expected_secs}"
        );
    }

    // 3. Verify voice.afs slots 129 (Leave), 4 (Maria), and 82 (In Water) ADX sample durations
    let afs = fs::read(root.join("data/sound/adx/voice/voice.afs")).expect("read voice.afs");
    for (slot, exp_dur) in [
        (
            usize::from(LEAVE_LETTER_VOICE_AFS_INDEX),
            LEAVE_LETTER_VOICE_DURATION_SECS,
        ),
        (
            usize::from(MARIA_LETTER_VOICE_AFS_INDEX),
            MARIA_LETTER_VOICE_DURATION_SECS,
        ),
        (
            usize::from(IN_WATER_DROWNING_VOICE_AFS_INDEX),
            IN_WATER_DROWNING_VOICE_DURATION_SECS,
        ),
    ] {
        let idx_off = 8 + slot * 8;
        let adx_off =
            u32::from_le_bytes(afs[idx_off..idx_off + 4].try_into().expect("off")) as usize;
        let sr = u32::from_be_bytes(afs[adx_off + 8..adx_off + 12].try_into().expect("sr")) as f32;
        let samples =
            u32::from_be_bytes(afs[adx_off + 12..adx_off + 16].try_into().expect("samples")) as f32;
        assert!(
            (samples / sr - exp_dur).abs() < 1e-4,
            "voice.afs[{slot}] duration mismatch"
        );
    }

    // 4. Verify UFO textures & sh2pc.exe sh2_attack_list[8, 9, 19..=23] @ 0x00629000
    for tex_rel in UFO_TEXTURE_PATHS
        .iter()
        .chain(BlueGemLocation::HospitalGarden.vignette_textures())
        .chain(BlueGemLocation::TolucaLakeDock.vignette_textures())
    {
        assert!(
            Path::new(&root.join(tex_rel)).is_file(),
            "missing {tex_rel}"
        );
    }

    let exe = fs::read(root.join("sh2pc.exe")).expect("read sh2pc.exe");
    let pe_off = u32::from_le_bytes(exe[0x3C..0x40].try_into().expect("pe")) as usize;
    let num_sec = u16::from_le_bytes(exe[pe_off + 6..pe_off + 8].try_into().expect("sec")) as usize;
    let opt_sz =
        u16::from_le_bytes(exe[pe_off + 20..pe_off + 22].try_into().expect("opt")) as usize;
    let img_base =
        u32::from_le_bytes(exe[pe_off + 52..pe_off + 56].try_into().expect("base")) as usize;
    let rva = 0x0062_9000usize - img_base;
    let mut table_off = 0usize;
    for i in 0..num_sec {
        let s = pe_off + 24 + opt_sz + i * 40;
        let vsz = u32::from_le_bytes(exe[s + 8..s + 12].try_into().expect("vsz")) as usize;
        let va_s = u32::from_le_bytes(exe[s + 12..s + 16].try_into().expect("va")) as usize;
        let rawoff = u32::from_le_bytes(exe[s + 20..s + 24].try_into().expect("raw")) as usize;
        if rva >= va_s && rva < va_s + vsz {
            table_off = rawoff + (rva - va_s);
            break;
        }
    }
    for spec in [
        ATK_HYPER_SPRAY_STAND,
        ATK_HYPER_SPRAY_WALK,
        ATK_CHAINSAW_SWING_1,
        ATK_CHAINSAW_SWING_2,
        ATK_CHAINSAW_OVERHEAD_1,
        ATK_CHAINSAW_OVERHEAD_2,
        ATK_CHAINSAW_THRUST,
    ] {
        let e = table_off + (spec.id as usize) * 36;
        let id = u32::from(u16::from_le_bytes(exe[e..e + 2].try_into().expect("id")));
        let kind = u16::from_le_bytes(exe[e + 2..e + 4].try_into().expect("kind")) as u8;
        let ap = f32::from_le_bytes(exe[e + 4..e + 8].try_into().expect("ap"));
        let sp = f32::from_le_bytes(exe[e + 8..e + 12].try_into().expect("sp"));
        let min_r = f32::from_le_bytes(exe[e + 12..e + 16].try_into().expect("min"));
        let max_r = f32::from_le_bytes(exe[e + 16..e + 20].try_into().expect("max"));
        assert_eq!(
            (id, kind, ap, sp, min_r, max_r),
            (
                spec.id,
                spec.kind,
                spec.ap,
                spec.sp,
                spec.min_range,
                spec.max_range
            )
        );
    }
    println!(
        "Verified retail letter*.mes, .bik headers, voice.afs, and sh2pc.exe @ {}",
        root.display()
    );
}
