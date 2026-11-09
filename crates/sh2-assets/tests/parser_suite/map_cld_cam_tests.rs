use sh2_assets::*;
use std::fs;
use std::path::Path;

use crate::ROOT_ASSETS;

#[test]
fn test_town_map_parsing() {
    // Test parsing vanilla town map (rr01.map)
    let map_path = format!("{ROOT_ASSETS}/data/bg/rr/rr01.map");
    if !Path::new(&map_path).exists() {
        println!("Skipping test_town_map_parsing: asset file not found at {map_path}");
        return;
    }

    let data = fs::read(&map_path).expect("Failed to read rr01.map");
    let map = parse_map(&data, "rr01").expect("Failed to parse rr01.map");

    println!(
        "Parsed map: materials={}, textures={}, batches={}",
        map.materials.len(),
        map.textures.len(),
        map.batches.len()
    );

    assert_eq!(map.name, "rr01");
    assert_eq!(map.materials.len(), 14);
    assert_eq!(map.textures.len(), 11);
    assert!(!map.batches.is_empty(), "Expected batches to be parsed");

    // Verify batch vertex attributes integrity
    for batch in &map.batches {
        assert_eq!(batch.positions.len(), batch.uvs.len());
        assert_eq!(batch.positions.len(), batch.colors.len());
        assert_eq!(batch.positions.len(), batch.normals.len());
        assert!(!batch.indices.is_empty());
    }

    // Test decoding embedded texture
    if let Some((_, tex)) = map.textures.iter().next() {
        let rgba = tex
            .to_rgba()
            .expect("Failed to decode DXT embedded texture");
        assert_eq!(rgba.len(), tex.width * tex.height * 4);
    }
}

#[test]
fn test_outdoor_observation_deck_map_parsing() {
    // Test parsing authentic outdoor Observation Deck map (ob01.map)
    let map_path = format!("{ROOT_ASSETS}/data/bg/ob/ob01.map");
    if !Path::new(&map_path).exists() {
        println!("Skipping test_outdoor_observation_deck_map_parsing: ob01.map not found");
        return;
    }

    let data = fs::read(&map_path).expect("Failed to read ob01.map");
    let map = parse_map(&data, "ob01").expect("Failed to parse ob01.map");

    println!(
        "Parsed ob01 map: materials={}, textures={}, batches={}",
        map.materials.len(),
        map.textures.len(),
        map.batches.len()
    );

    let mut min_p = [f32::MAX; 3];
    let mut max_p = [f32::MIN; 3];
    for b in &map.batches {
        for p in &b.positions {
            for i in 0..3 {
                if p[i] < min_p[i] {
                    min_p[i] = p[i];
                }
                if p[i] > max_p[i] {
                    max_p[i] = p[i];
                }
            }
        }
    }
    println!("ob01 min: {:?}, max: {:?}", min_p, max_p);

    assert_eq!(map.name, "ob01");
    assert!(!map.materials.is_empty(), "ob01.map must contain materials");
    assert!(!map.batches.is_empty(), "ob01.map must contain batches");
}

#[test]
fn test_opening_restroom_and_hotel_boiler_map_parsing() {
    // Test parsing authentic opening restroom map (er01.map: stage_toilet / R_BEGIN_BATHROOM = 0x01)
    let er_map_path = format!("{ROOT_ASSETS}/data/bg/er/er01.map");
    if Path::new(&er_map_path).exists() {
        let data = fs::read(&er_map_path).expect("Failed to read er01.map");
        let map = parse_map(&data, "er01").expect("Failed to parse er01.map");
        println!(
            "Parsed er01 (Opening Restroom) map: materials={}, textures={}, batches={}",
            map.materials.len(),
            map.textures.len(),
            map.batches.len()
        );
        let mut min_p = [f32::MAX; 3];
        let mut max_p = [f32::MIN; 3];
        for b in &map.batches {
            for p in &b.positions {
                for i in 0..3 {
                    if p[i] < min_p[i] {
                        min_p[i] = p[i];
                    }
                    if p[i] > max_p[i] {
                        max_p[i] = p[i];
                    }
                }
            }
        }
        println!("er01 min: {:?}, max: {:?}", min_p, max_p);
        assert_eq!(map.name, "er01");
        assert!(!map.batches.is_empty(), "er01.map must contain batches");
    }

    // Test parsing authentic hotel boiler room map (rr21.map: stage_hotel_bf_f / R_HTL_BOILER_RM = 0xA7)
    let rr_map_path = format!("{ROOT_ASSETS}/data/bg/rr/rr21.map");
    if Path::new(&rr_map_path).exists() {
        let data = fs::read(&rr_map_path).expect("Failed to read rr21.map");
        let map = parse_map(&data, "rr21").expect("Failed to parse rr21.map");
        println!(
            "Parsed rr21 (Hotel Boiler Room) map: materials={}, textures={}, batches={}",
            map.materials.len(),
            map.textures.len(),
            map.batches.len()
        );
        let mut min_p = [f32::MAX; 3];
        let mut max_p = [f32::MIN; 3];
        for b in &map.batches {
            for p in &b.positions {
                for i in 0..3 {
                    if p[i] < min_p[i] {
                        min_p[i] = p[i];
                    }
                    if p[i] > max_p[i] {
                        max_p[i] = p[i];
                    }
                }
            }
        }
        println!("rr21 min: {:?}, max: {:?}", min_p, max_p);
        assert_eq!(map.name, "rr21");
        assert!(!map.batches.is_empty(), "rr21.map must contain batches");
    }

    // Test parsing authentic hotel boiler room collision (rr21.cld)
    let rr_cld_path = format!("{ROOT_ASSETS}/data/bg/rr/rr21.cld");
    if Path::new(&rr_cld_path).exists() {
        let data = fs::read(&rr_cld_path).expect("Failed to read rr21.cld");
        let cld = sh2_assets::formats::cld::parse_cld(&data).expect("Failed to parse rr21.cld");
        println!(
            "Parsed rr21.cld (Hotel Boiler Room): origin={:?}, floors={}, walls={}",
            cld.origin,
            cld.floors.len(),
            cld.walls.len()
        );
        assert_eq!(cld.origin, [-120000.0, -80000.0]);
        assert!(
            !cld.floors.is_empty(),
            "rr21.cld must contain valid floor triangles"
        );
        // Verify spatial grid index tables
        assert!(
            cld.b0_cells.iter().any(|c| !c.is_empty()),
            "rr21.cld b0_cells must index floor triangles"
        );
        assert!(
            cld.b1_cells.iter().any(|c| !c.is_empty()),
            "rr21.cld b1_cells must index wall segments"
        );
        assert!(!cld.walls.is_empty(), "rr21.cld must contain wall segments");
    }
}

#[test]
fn test_cam_binary_parsing_er01_and_rr21() {
    let er01_path = format!("{ROOT_ASSETS}/data/bg/er/er01.cam");
    if Path::new(&er01_path).exists() {
        let data = fs::read(&er01_path).expect("Failed to read er01.cam");
        let cam = parse_cam(&data).expect("Failed to parse er01.cam");

        // er01 has 4 active records + 1 terminal record
        assert_eq!(cam.records.len(), 4);

        // Record 2 is the sink/mirror view
        let sink_rec = &cam.records[2];
        assert_eq!(sink_rec.rd_type, RoadType::RoadPrioHigh);
        assert_eq!(sink_rec.projection, 757.0);
        assert_eq!(sink_rec.ofs_watch_hy, -1510.0);
        let (sw_min_x, sw_max_x) = sink_rec.lim_sw.x_bounds();
        let (sw_min_z, sw_max_z) = sink_rec.lim_sw.z_bounds();
        assert_eq!(sw_min_x, -19958.0);
        assert_eq!(sw_max_x, -19310.0);
        assert_eq!(sw_min_z, 20214.0);
        assert_eq!(sw_max_z, 20880.0);

        // Check sentinel substitution on Record 0: ofs_watch_hy=10000.0 -> -300.0, projection=0.0 -> 448.0
        let exit_rec = &cam.records[0];
        assert_eq!(exit_rec.ofs_watch_hy, -300.0);
        assert_eq!(exit_rec.projection, 448.0);
        assert_eq!(exit_rec.rd_type, RoadType::Road);

        // Record 1: Stalls
        let stall_rec = &cam.records[1];
        assert_eq!(stall_rec.ofs_watch_hy, -300.0);
        assert_eq!(stall_rec.projection, 448.0);

        // Volume selection tests in er01:
        // 1. Sink framing
        let active_sink = cam.find_active_record([-19600.0, -1000.0, 20500.0], 0.0);
        assert!(active_sink.is_some());
        assert_eq!(active_sink.unwrap().projection, 757.0);

        // 2. Stalls framing
        let active_stalls = cam.find_active_record([-19000.0, -1000.0, 20500.0], 0.0);
        assert!(active_stalls.is_some());
        assert_eq!(active_stalls.unwrap().rd_type, RoadType::Road);

        // 3. Exit door framing
        let active_exit = cam.find_active_record([-18800.0, -1000.0, 19000.0], 0.0);
        assert!(active_exit.is_some());
        assert_eq!(active_exit.unwrap().rd_type, RoadType::RoadPrioHigh);
    }

    let rr21_path = format!("{ROOT_ASSETS}/data/bg/rr/rr21.cam");
    if Path::new(&rr21_path).exists() {
        let data = fs::read(&rr21_path).expect("Failed to read rr21.cam");
        let cam = parse_cam(&data).expect("Failed to parse rr21.cam");

        // rr21 has 3 active records + 1 terminal record
        assert_eq!(cam.records.len(), 3);
        assert_eq!(cam.records[0].cam_mv_type, CamMovementType::Chase);
        assert_eq!(cam.records[1].cam_mv_type, CamMovementType::FixedAngle);
        assert_eq!(cam.records[2].cam_mv_type, CamMovementType::FixedAngle);
        assert_eq!(cam.records[1].projection, 512.0);
        assert_eq!(cam.records[2].projection, 480.0);
    }
}

#[test]
fn test_sh2_73_shared_area_map_texture_bank_resolution_and_bgra8_decoding() {
    // 1. Verify MapTexture::to_rgba() supports uncompressed 32-bit BGRA8 (kind == 0)
    let bgra_tex = formats::map::MapTexture {
        id: 9999,
        width: 2,
        height: 1,
        kind: 0,
        data: vec![
            10, 20, 30, 255, // Pixel 0: B=10, G=20, R=30, A=255 -> RGBA = [30, 20, 10, 255]
            40, 50, 60, 128, // Pixel 1: B=40, G=50, R=60, A=128 -> RGBA = [60, 50, 40, 128]
        ],
    };
    let rgba = bgra_tex
        .to_rgba()
        .expect("Failed to decode uncompressed BGRA8 MapTexture");
    assert_eq!(rgba, vec![30, 20, 10, 255, 60, 50, 40, 128]);

    // 2. Verify TextureLoader::load_map_with_hd_overrides merges shared stage bank `bg/ob/ob.map` into `ob01.map`
    let data_dir = Path::new(ROOT_ASSETS).join("data");
    if !data_dir.join("bg/ob/ob01.map").exists() {
        println!("Skipping shared area map texture resolution test: retail assets not found");
        return;
    }
    let sh2e_dir = Path::new(ROOT_ASSETS).join("sh2e");
    let loader = formats::TextureLoader::new(
        &data_dir,
        if sh2e_dir.exists() {
            Some(sh2e_dir)
        } else {
            None
        },
    );

    let ob01 = loader
        .load_map_with_hd_overrides("bg/ob/ob01.map", "ob01")
        .expect("Failed to load ob01.map with shared ob.map textures");

    // Material 20 in ob01.map references shared pine foliage texture 4656 from `bg/ob/ob.map`
    assert!(
        ob01.textures.contains_key(&4656),
        "ob01.map must contain shared pine foliage texture 4656 merged from bg/ob/ob.map"
    );

    for (idx, mat) in ob01.materials.iter().enumerate() {
        let tid = mat.texture_id as u32;
        if tid != 0 {
            let tex = ob01.textures.get(&tid).unwrap_or_else(|| {
                panic!("Material {idx} texture_id {tid} missing in loaded ob01.map")
            });
            let decoded = tex.to_rgba().expect("Failed to decode ob01 texture");
            assert_eq!(decoded.len(), tex.width * tex.height * 4);
        }
    }
}
