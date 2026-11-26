//! SH2-170 1:1 parity tests for `.anm` root track types 4/5/6, 10-byte axis-angle payload,
//! nibble bit 3 (`nibble & 8`), and retail `.anm` decoding (`SLUS_202.28` `shExec` `0x001002c0` /
//! `sh2pc.exe` `FUN_0054fdc0` `0x0054fdc0`).

use sh2_assets::formats::anm::{parse_anm, AnimationTrackType};
use sh2_assets::formats::mdls::parse_mdl;
use std::path::PathBuf;

fn f32_to_f16_bits(val: f32) -> u16 {
    let bits = val.to_bits();
    let sign = ((bits >> 31) & 1) as u16;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let mant = bits & 0x7F_FFFF;
    if exp == 0 {
        return sign << 15;
    }
    let half_exp = (exp - 127 + 15).clamp(1, 30) as u16;
    let half_mant = (mant >> 13) as u16;
    (sign << 15) | (half_exp << 10) | half_mant
}

#[test]
fn test_anm_root_track_4_and_5_inversion_fix_and_axis_angle() {
    // 1 root bone (`parents = [255]`), 2 frames:
    // Frame 0: root track type 5 + nibble bit 3 (`0xD` = `8 | 5`):
    //   - `cur_trans` = 3x f32: [100.0, -250.0, 42.5] (12B)
    //   - `rot` = 3x i16 Q12: [4096, -2048, 1024] -> [1.0, -0.5, 0.25] rad (6B)
    //   - `axis` = 3x i16 Q15: [16384, 0, -16384] -> [0.5, 0.0, -0.5] (6B)
    //   - `angle` = i16 Q12: 2048 -> 0.5 rad (2B) + 2B pad
    // Frame 1: root track type 4 without bit 3 (`0x4`):
    //   - copies Frame 0's `next_trans` (`[100.0, -250.0, 42.5]`) into `cur_trans`
    //   - reads NEW `next_trans` as 3x f16 at 4-byte stride (`GetSF`, `p+0, p+4, p+8`, 12B):
    //     [2.0, -4.0, 8.0]
    //   - `rot` = [0, 0, 0], `axis` = [0, 32767, 0], `angle` = -4096 (-1.0 rad), 2B pad
    let parents = [255u8];
    let mut buf = Vec::new();

    // Frame 0 header: bone 0 = 0xD (bit3=1, track=5)
    buf.extend_from_slice(&0x0000_000Du32.to_le_bytes());
    for v in [100.0f32, -250.0, 42.5] {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    for v in [4096i16, -2048, 1024, 16384, 0, -16384, 2048, 0] {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    // Frame 1 header: bone 0 = 0x4 (bit3=0, track=4)
    buf.extend_from_slice(&0x0000_0004u32.to_le_bytes());
    for v in [2.0f32, -4.0, 8.0] {
        buf.extend_from_slice(&f32_to_f16_bits(v).to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes()); // upper 16 bits of 4-byte slot
    }
    for v in [0i16, 0, 0, 0, 16384, 0, -4096, 0] {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    let anm = parse_anm(&buf, 1, &parents).expect("track 4/5 stream must parse");
    assert_eq!(anm.frames.len(), 2);

    // Frame 0 (track 5): 3x f32 root translation + bit3=true + axis-angle
    let b0 = anm.frames[0].bones[0].as_ref().expect("bone 0 frame 0");
    assert_eq!(
        b0.track_type,
        AnimationTrackType::ExtendedTranslationAndRotation2
    );
    assert!(
        b0.nibble_flag,
        "nibble bit 3 (`uVar4 & 8`) must be preserved"
    );
    assert_eq!(b0.translation, Some([100.0, -250.0, 42.5]));
    assert_eq!(b0.next_translation, Some([100.0, -250.0, 42.5]));
    assert_eq!(b0.rotation, Some([1.0, -0.5, 0.25]));
    assert_eq!(b0.axis_angle_delta, Some([0.5, 0.0, -0.5, 0.5]));

    // Frame 1 (track 4): previous next_trans -> cur_trans, new next_trans decoded as 3x f16 @ 4B stride
    let b1 = anm.frames[1].bones[0].as_ref().expect("bone 0 frame 1");
    assert_eq!(
        b1.track_type,
        AnimationTrackType::ExtendedTranslationAndRotation1
    );
    assert!(!b1.nibble_flag);
    assert_eq!(b1.translation, Some([100.0, -250.0, 42.5]));
    assert_eq!(b1.next_translation, Some([2.0, -4.0, 8.0]));
    assert_eq!(b1.axis_angle_delta, Some([0.0, 0.5, 0.0, -1.0]));
}

#[test]
fn test_anm_track_6_dual_translation_and_track_3_promotion() {
    // 1 root bone, Frame 0 = track 6 (3x f32 cur_trans + 3x f16@4B next_trans + 16B rot/axis-angle)
    // Frame 1 = track 3 (promotes Frame 0's `next_trans` into `cur_trans` + 16B rot/axis-angle)
    let parents = [255u8];
    let mut buf = Vec::new();

    // Frame 0: track 6
    buf.extend_from_slice(&0x0000_0006u32.to_le_bytes());
    for v in [10.0f32, 20.0, 30.0] {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    for v in [1.5f32, -2.5, 3.5] {
        buf.extend_from_slice(&f32_to_f16_bits(v).to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
    }
    for v in [0i16, 0, 0, 8192, 0, 0, 1024, 0] {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    // Frame 1: track 3
    buf.extend_from_slice(&0x0000_0003u32.to_le_bytes());
    for v in [0i16, 0, 0, 0, 0, 8192, 512, 0] {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    let anm = parse_anm(&buf, 1, &parents).expect("track 6+3 stream must parse");
    assert_eq!(anm.frames.len(), 2);

    let b0 = anm.frames[0].bones[0].as_ref().unwrap();
    assert_eq!(b0.translation, Some([10.0, 20.0, 30.0]));
    assert_eq!(b0.next_translation, Some([1.5, -2.5, 3.5]));
    assert_eq!(b0.axis_angle_delta, Some([0.25, 0.0, 0.0, 0.25]));

    let b1 = anm.frames[1].bones[0].as_ref().unwrap();
    assert_eq!(b1.translation, Some([1.5, -2.5, 3.5]));
    assert_eq!(b1.axis_angle_delta, Some([0.0, 0.0, 0.25, 0.125]));
}

#[test]
fn test_retail_jms_basic1_anm_tracks_4_5_6_finite_and_valid() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/chr/jms");
    let mdl_path = base.join("hll_jms.mdl");
    let anm_path = base.join("jms_basic1.anm");
    if !mdl_path.exists() || !anm_path.exists() {
        return;
    }

    let mdl_bytes = std::fs::read(&mdl_path).expect("read hll_jms.mdl");
    let mdl = parse_mdl(&mdl_bytes).expect("parse hll_jms.mdl");
    let anm_bytes = std::fs::read(&anm_path).expect("read jms_basic1.anm");
    let anm = parse_anm(&anm_bytes, mdl.bone_count, &mdl.parents).expect("parse jms_basic1.anm");

    assert!(
        anm.frames.len() >= 700,
        "jms_basic1.anm must have >= 700 frames"
    );
    let mut non_empty_bones = 0usize;
    for frame in &anm.frames {
        for b in frame.bones.iter().flatten() {
            non_empty_bones += 1;
            if let Some(t) = b.translation {
                assert!(
                    t.iter().all(|v| v.is_finite()),
                    "non-finite translation: {t:?}"
                );
            }
            if let Some(nt) = b.next_translation {
                assert!(
                    nt.iter().all(|v| v.is_finite()),
                    "non-finite next_translation: {nt:?}"
                );
            }
            if let Some(aa) = b.axis_angle_delta {
                assert!(
                    aa.iter().all(|v| v.is_finite()),
                    "non-finite axis_angle_delta: {aa:?}"
                );
            }
        }
    }
    assert!(
        non_empty_bones > 10_000,
        "retail jms_basic1.anm must decode > 10,000 bone transforms"
    );
}
