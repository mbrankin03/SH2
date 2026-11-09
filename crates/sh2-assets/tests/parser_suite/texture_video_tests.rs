use sh2_assets::*;
use std::fs;
use std::path::Path;

use crate::ROOT_ASSETS;

#[test]
fn test_texture_container_parsing_and_tbn2() {
    let tbn2_path = format!("{ROOT_ASSETS}/data/pic/effect/fog.tbn2");
    if !Path::new(&tbn2_path).exists() {
        println!("Skipping test_texture_container_parsing_and_tbn2: asset file not found");
        return;
    }

    let data = fs::read(&tbn2_path).expect("Failed to read fog.tbn2");
    let textures = parse_texture_container(&data, 0).expect("Failed to parse fog.tbn2 container");
    assert_eq!(textures.len(), 1);
    let tex = &textures[0];
    assert_eq!(tex.id, 11042);
    assert_eq!(tex.width, 256);
    assert_eq!(tex.height, 256);
    let rgba = tex.to_rgba().expect("Failed to decode fog.tbn2 DXT5");
    assert_eq!(rgba.len(), 256 * 256 * 4);
}

#[test]
fn test_texture_loader_and_ee_fallback() {
    let data_dir = format!("{ROOT_ASSETS}/data");
    let sh2e_dir = format!("{ROOT_ASSETS}/sh2e");

    if Path::new(&data_dir).exists() || Path::new(&sh2e_dir).exists() {
        let loader = TextureLoader::new(&data_dir, Some(&sh2e_dir));
        let tex = loader
            .load_texture("pic/etc/start00.tex")
            .expect("Failed to load start00.tex via TextureLoader");
        assert!(tex.width > 0 && tex.height > 0);
        assert_eq!(tex.rgba.len(), tex.width * tex.height * 4);
        println!("Loaded start00.tex: {}x{}", tex.width, tex.height);
    }

    if !Path::new(&data_dir).exists() && !Path::new(&sh2e_dir).exists() {
        println!("Skipping real texture loading: asset directories not present");
        // Test in-memory synthetic DDS decoding to ensure test coverage in headless CI
        let mut fake_dds = vec![0u8; 128 + 8];
        fake_dds[0..4].copy_from_slice(b"DDS ");
        fake_dds[12..16].copy_from_slice(&4u32.to_le_bytes()); // height
        fake_dds[16..20].copy_from_slice(&4u32.to_le_bytes()); // width
        fake_dds[84..88].copy_from_slice(b"DXT1"); // fourcc
        let res = decode_dds_file(&fake_dds);
        assert!(res.is_ok());
        let (w, h, rgba) = res.unwrap();
        assert_eq!(w, 4);
        assert_eq!(h, 4);
        assert_eq!(rgba.len(), 4 * 4 * 4);
        return;
    }

    let loader = TextureLoader::new(&data_dir, Some(&sh2e_dir));

    // Test resolving and decoding a PNG texture from sh2e pack if it exists
    if let Ok(tex) = loader.load_texture("pic/etc/RedCrossIcon.png") {
        assert!(tex.width > 0 && tex.height > 0);
        assert_eq!(tex.rgba.len(), tex.width * tex.height * 4);
    }

    // Test resolving vanilla DDS if it exists
    if let Ok(dds_tex) = loader.load_texture("demo/reizouko_a/reizouko_jms.dds") {
        assert!(dds_tex.width > 0 && dds_tex.height > 0);
        assert_eq!(dds_tex.rgba.len(), dds_tex.width * dds_tex.height * 4);
    }
}

#[test]
fn test_sh2_10_bink_movies_hd_map_overrides_and_mipmaps() {
    // 1. Always verify mipmap chain generation in headless CI
    let base_rgba = vec![200u8; 64 * 32 * 4];
    let mips = generate_mipmap_chain_rgba(&base_rgba, 64, 32);
    assert_eq!(mips.len(), 7);
    assert_eq!((mips[0].0, mips[0].1), (64, 32));
    assert_eq!((mips[6].0, mips[6].1), (1, 1));

    let data_dir = format!("{ROOT_ASSETS}/data");
    let sh2e_dir = format!("{ROOT_ASSETS}/sh2e");
    if !Path::new(&data_dir).exists() || !Path::new(&sh2e_dir).exists() {
        println!("Skipping retail Bink & HD map override checks: retail assets not present");
        return;
    }

    // 2. Verify all 21 Bink (.bik) headers in both vanilla `data/movie/` and SH2:EE 1080p60 `sh2e/movie/`
    let movie_files = [
        "open.bik",
        "toilet.bik",
        "flash.bik",
        "hakaba.bik",
        "gero.bik",
        "hei.bik",
        "knife.bik",
        "deai.bik",
        "saikai.bik",
        "rouya.bik",
        "korosu_a.bik",
        "korosu_b.bik",
        "murder.bik",
        "end.bik",
        "water.bik",
        "end_dog.bik",
        "end_wish.bik",
        "credits.bik",
        "sh2e3.bik",
        "sh3e3.bik",
        "sh3tgs.bik",
    ];

    for movie_name in &movie_files {
        let van_path = format!("{data_dir}/movie/{movie_name}");
        let ee_path = format!("{sh2e_dir}/movie/{movie_name}");

        let van_hdr = parse_bink_file_header(&van_path)
            .unwrap_or_else(|e| panic!("Failed to parse vanilla {van_path}: {e:?}"));
        let ee_hdr = parse_bink_file_header(&ee_path)
            .unwrap_or_else(|e| panic!("Failed to parse SH2:EE {ee_path}: {e:?}"));

        assert!(
            van_hdr.width >= 320 && van_hdr.width <= 1920,
            "Unexpected vanilla width {} for {movie_name}",
            van_hdr.width
        );
        assert!(
            van_hdr.fps() >= 14.0 && van_hdr.fps() <= 60.0,
            "Unexpected vanilla fps {} for {movie_name}",
            van_hdr.fps()
        );
        assert!(
            ee_hdr.width >= 640 && ee_hdr.width <= 1920,
            "Unexpected SH2:EE width {} for {movie_name}",
            ee_hdr.width
        );
        assert!(
            ee_hdr.height >= 360 && ee_hdr.height <= 1230,
            "Unexpected SH2:EE height {} for {movie_name}",
            ee_hdr.height
        );
        assert!(
            ee_hdr.fps() >= 25.0 && ee_hdr.fps() <= 60.0,
            "Unexpected SH2:EE fps {} for {movie_name}",
            ee_hdr.fps()
        );
    }

    // 3. Test decoding the first frame of `sh2e/movie/flash.bik` via BinkVideoStream
    let flash_ee = format!("{sh2e_dir}/movie/flash.bik");
    let mut stream = BinkVideoStream::open(&flash_ee).expect("Failed to open sh2e/movie/flash.bik");
    assert_eq!(stream.header.width, 1920);
    assert_eq!(stream.header.height, 1230);
    let frame0 = stream
        .next_frame()
        .expect("Expected frame 0 from flash.bik");
    assert_eq!(frame0.width, 1920);
    assert_eq!(frame0.height, 1230);
    assert_eq!(frame0.y_plane.len(), 1920 * 1230);

    // 4. Verify TextureLoader::load_map_with_hd_overrides prioritizes `sh2e/bg/` high-res maps
    let loader = TextureLoader::new(&data_dir, Some(&sh2e_dir));
    let ob_map = loader
        .load_map_with_hd_overrides("bg/ob/ob01.map", "ob01")
        .expect("Failed to load ob01.map with HD overrides");
    assert!(!ob_map.textures.is_empty());
}
