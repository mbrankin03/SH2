//! Unit and integration tests for `SH2-15` modular cutaway still and puzzle overlay `.tex` loading:
//! - PC format `8` (8-bit paletted `.tex` with palette header at `0x50 + len + 0x20`)
//! - PS2 `0xa7a7a7a7` `.tex` (both 32-bit `bpp == 0x18` and GS-swizzled `PSMT8` + CSM1 CLUT)
//! - Seamless switching between SH2:EE (`sh2e/pic`) and original PS2/PC (`data/pic`) packs
//! - Original texel UV normalization and 4:3 / widescreen viewport aspect correction.

use sh2_assets::{
    decode_texture_bytes, probe_tex_dimensions, AssetPathResolver, CutawayTextureAsset,
    DecodedTexture, TextureLoader, TEX_MAGIC_BYTES,
};
use std::path::Path;

#[test]
fn test_normalize_cutaway_rel_path() {
    assert_eq!(
        TextureLoader::normalize_cutaway_rel_path("out_p_incar"),
        Path::new("pic/out/p_incar.tex")
    );
    assert_eq!(
        TextureLoader::normalize_cutaway_rel_path("apt_clock_close"),
        Path::new("pic/apt/clock_close.tex")
    );
    assert_eq!(
        TextureLoader::normalize_cutaway_rel_path("out/p_incar_map"),
        Path::new("pic/out/p_incar_map.tex")
    );
    assert_eq!(
        TextureLoader::normalize_cutaway_rel_path("data/pic/item/x_mary_p.tex"),
        Path::new("pic/item/x_mary_p.tex")
    );
}

#[test]
fn test_decode_synthetic_pc_paletted_format_8_tex() {
    let w: u16 = 4;
    let h: u16 = 4;
    let pixel_count = (w as usize) * (h as usize);
    let total_len = 0x50 + pixel_count + 0x20 + 1024;
    let mut buf = vec![0u8; total_len];

    // Magic 0x19990901
    buf[0..4].copy_from_slice(&TEX_MAGIC_BYTES);
    // Texture header at 0x10: tag=1, w=4, h=4, nspr=1
    buf[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
    buf[0x14..0x16].copy_from_slice(&w.to_le_bytes());
    buf[0x16..0x18].copy_from_slice(&h.to_le_bytes());
    buf[0x1c..0x1e].copy_from_slice(&1u16.to_le_bytes());

    // Sprite header at 0x30: w, h at 0x38, fmt=8 at 0x3c, comp=0 at 0x3d, ds=16 at 0x40, marker=0x9900 at 0x4e
    buf[0x38..0x3a].copy_from_slice(&w.to_le_bytes());
    buf[0x3a..0x3c].copy_from_slice(&h.to_le_bytes());
    buf[0x3c] = 8;
    buf[0x3d] = 0;
    buf[0x40..0x44].copy_from_slice(&(pixel_count as u32).to_le_bytes());
    buf[0x4e..0x50].copy_from_slice(&0x9900u16.to_le_bytes());

    // Indices at 0x50..0x60: set pixel 0 to palette index 1, pixel 1 to palette index 2
    buf[0x50] = 1;
    buf[0x51] = 2;

    // Palette at 0x50 + 16 + 0x20
    let pal_ofs = 0x50 + pixel_count + 0x20;
    buf[pal_ofs + 4..pal_ofs + 8].copy_from_slice(&[10, 20, 30, 255]); // index 1
    buf[pal_ofs + 8..pal_ofs + 12].copy_from_slice(&[100, 150, 200, 255]); // index 2

    assert_eq!(probe_tex_dimensions(&buf), Some((4, 4)));
    let decoded = decode_texture_bytes(&buf, Path::new("synthetic_pal8.tex")).unwrap();
    assert_eq!(decoded.width, 4);
    assert_eq!(decoded.height, 4);
    assert_eq!(&decoded.rgba[0..4], &[10, 20, 30, 255]);
    assert_eq!(&decoded.rgba[4..8], &[100, 150, 200, 255]);
}

#[test]
fn test_decode_synthetic_ps2_tex_24bpp_and_8bpp_clut() {
    // 1. PS2 24bpp (bpp = 0x18): RGB is already full 0..=255, only Alpha is PS2 0..=0x80 -> 0..=255
    let w: u16 = 2;
    let h: u16 = 2;
    let mut ps2_24 = vec![0u8; 0xc0 + 2 * 2 * 4];
    ps2_24[0x0c..0x10].copy_from_slice(b"\xa7\xa7\xa7\xa7");
    ps2_24[0x48..0x4a].copy_from_slice(&w.to_le_bytes());
    ps2_24[0x4a..0x4c].copy_from_slice(&h.to_le_bytes());
    ps2_24[0x4c] = 0x18;
    // Pixel 0: RGB = [64, 180, 200] preserved verbatim, Alpha = 0x80 -> 255
    ps2_24[0xc0..0xc4].copy_from_slice(&[64, 180, 200, 0x80]);

    assert_eq!(probe_tex_dimensions(&ps2_24), Some((2, 2)));
    let dec24 = decode_texture_bytes(&ps2_24, Path::new("synthetic_ps2_24.tex")).unwrap();
    assert_eq!(&dec24.rgba[0..4], &[64, 180, 200, 255]);

    // 2. PS2 8bpp GS-swizzled + CSM1 CLUT: RGB preserved verbatim, Alpha doubled
    let w8: u16 = 16;
    let h8: u16 = 16;
    let pix_len = (w8 as usize) * (h8 as usize);
    let mut ps2_8 = vec![0u8; 0xc0 + pix_len + 0x30 + 0x4000];
    ps2_8[0x0c..0x10].copy_from_slice(b"\xa7\xa7\xa7\xa7");
    ps2_8[0x48..0x4a].copy_from_slice(&w8.to_le_bytes());
    ps2_8[0x4a..0x4c].copy_from_slice(&h8.to_le_bytes());
    ps2_8[0x4c] = 0x08;
    // Swizzled index 0 at (0,0) maps to byte 0 -> set index = 0
    ps2_8[0xc0] = 0;
    let clut_ofs = 0xc0 + pix_len + 0x30;
    ps2_8[clut_ofs..clut_ofs + 4].copy_from_slice(&[50, 150, 200, 64]);
    let dec8 = decode_texture_bytes(&ps2_8, Path::new("synthetic_ps2_8.tex")).unwrap();
    assert_eq!(dec8.width, 16);
    assert_eq!(dec8.height, 16);
    assert_eq!(&dec8.rgba[0..4], &[50, 150, 200, 128]);
}

#[test]
fn test_parse_texture_container_uses_per_sprite_dims_and_ds_advance() {
    // Build a 0x19990901 container with 1 texture block (block dims 8x8, nspr = 2):
    // - Sprite 0: per-sprite dims 4x4, BGRA8 (fmt=0x20, comp=0), exp=64 bytes, declared ds=96 bytes (32B padding/mip)
    // - Sprite 1: per-sprite dims 2x2, BGRA8 (fmt=0x20, comp=0), ds=16 bytes
    let mut buf = Vec::new();
    // Container header (16B)
    buf.extend_from_slice(&TEX_MAGIC_BYTES);
    buf.extend_from_slice(&[0u8; 12]);
    // Texture block header (32B): tex_id=77, block_w=8, block_h=8, nspr=2
    let mut blk = [0u8; 32];
    blk[0..4].copy_from_slice(&77u32.to_le_bytes());
    blk[4..6].copy_from_slice(&8u16.to_le_bytes());
    blk[6..8].copy_from_slice(&8u16.to_le_bytes());
    blk[12..14].copy_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&blk);

    // Sprite 0 header (32B): sid=100, sw=4, sh=4, fmt=0x20, comp=0, ds=96, marker=0x9900
    let mut spr0 = [0u8; 32];
    spr0[0..4].copy_from_slice(&100u32.to_le_bytes());
    spr0[8..10].copy_from_slice(&4u16.to_le_bytes());
    spr0[10..12].copy_from_slice(&4u16.to_le_bytes());
    spr0[12] = 0x20;
    spr0[13] = 0;
    spr0[16..20].copy_from_slice(&96u32.to_le_bytes());
    spr0[30..32].copy_from_slice(&0x9900u16.to_le_bytes());
    buf.extend_from_slice(&spr0);
    // Sprite 0 payload: 64 bytes BGRA + 32 bytes trailing padding/mip
    let mut pay0 = vec![0u8; 96];
    pay0[0..4].copy_from_slice(&[10, 20, 30, 255]); // B=10, G=20, R=30, A=255
    buf.extend_from_slice(&pay0);

    // Sprite 1 header (32B): sid=101, sw=2, sh=2, fmt=0x20, comp=0, ds=16, marker=0x9900
    let mut spr1 = [0u8; 32];
    spr1[0..4].copy_from_slice(&101u32.to_le_bytes());
    spr1[8..10].copy_from_slice(&2u16.to_le_bytes());
    spr1[10..12].copy_from_slice(&2u16.to_le_bytes());
    spr1[12] = 0x20;
    spr1[13] = 0;
    spr1[16..20].copy_from_slice(&16u32.to_le_bytes());
    spr1[30..32].copy_from_slice(&0x9900u16.to_le_bytes());
    buf.extend_from_slice(&spr1);
    let mut pay1 = vec![0u8; 16];
    pay1[0..4].copy_from_slice(&[40, 50, 60, 255]); // B=40, G=50, R=60, A=255
    buf.extend_from_slice(&pay1);

    let parsed = sh2_assets::parse_texture_container(&buf, 0).expect("container must parse");
    assert_eq!(parsed.len(), 2, "must not desync past padded sprite 0");
    assert_eq!(parsed[0].sprite_id, 100);
    assert_eq!((parsed[0].width, parsed[0].height), (4, 4));
    assert_eq!(parsed[0].data.len(), 96);
    assert_eq!(&parsed[0].to_rgba().unwrap()[0..4], &[30, 20, 10, 255]);

    assert_eq!(parsed[1].sprite_id, 101);
    assert_eq!((parsed[1].width, parsed[1].height), (2, 2));
    assert_eq!(parsed[1].data.len(), 16);
    assert_eq!(&parsed[1].to_rgba().unwrap()[0..4], &[60, 50, 40, 255]);
}

#[test]
fn test_cutaway_uv_normalization_and_viewport_aspect_transform() {
    // Simulate a 4x upscaled SH2:EE sprite sheet (1024x1024) whose original PS2/PC sheet was 256x256
    let asset = CutawayTextureAsset {
        texture: DecodedTexture {
            width: 1024,
            height: 1024,
            rgba: vec![0; 1024 * 1024 * 4],
        },
        original_width: 256,
        original_height: 256,
        is_sh2ee_hd: true,
    };

    // Original texel (128, 64) normalizes against original 256x256 -> (0.5, 0.25)
    let (u, v) = asset.normalize_uv(128.0, 64.0);
    assert!((u - 0.5).abs() < 1e-6);
    assert!((v - 0.25).abs() < 1e-6);

    // Square texture (512x512 or 2048x2048) displayed in a 16:9 (1920x1080) viewport pillars to 4:3 (1440x1080)
    let vp = asset.compute_viewport_transform(1920.0, 1080.0);
    assert!((vp.draw_width - 1440.0).abs() < 1e-3);
    assert!((vp.draw_height - 1080.0).abs() < 1e-3);
    assert!((vp.offset_x - 240.0).abs() < 1e-3);
    assert!((vp.offset_y - 0.0).abs() < 1e-3);
}

#[test]
fn test_retail_cutaway_and_special_format_textures_when_present() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.has_ps2_pic_pack() {
        return;
    }
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_ref());

    // Verify original PS2/PC special format files (paletted fmt 8 and PS2 0xa7a7a7a7) decode cleanly
    for name in [
        "htl_p_hanging",
        "hsp_p_box_key",
        "out_p_statue_box",
        "item_x_coin_snake",
        "out_p_incar",
        "out/p_incar_map",
        "apt_clock_close",
        "apt/clock_hari",
    ] {
        let orig = loader
            .load_cutaway_texture(name, false)
            .unwrap_or_else(|e| panic!("Failed to decode original cutaway {name}: {e}"));
        assert!(orig.texture.width > 0 && orig.texture.height > 0);
        assert!(!orig.is_sh2ee_hd);

        if resolver.has_sh2ee_pic_pack() {
            let hd = loader
                .load_cutaway_texture(name, true)
                .unwrap_or_else(|e| panic!("Failed to decode SH2:EE cutaway {name}: {e}"));
            assert!(hd.texture.width >= orig.texture.width);
            assert_eq!(hd.original_width, orig.original_width);
            assert_eq!(hd.original_height, orig.original_height);
        }
    }
}
