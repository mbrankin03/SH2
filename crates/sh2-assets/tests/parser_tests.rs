#[cfg(test)]
mod tests {
    use sh2_assets::*;
    use std::fs;
    use std::path::Path;

    const ROOT_ASSETS: &str = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";

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
    fn test_james_model_parsing() {
        // Test parsing James model (hhh_jms.mdl)
        let mdl_path = format!("{ROOT_ASSETS}/data/chr/jms/hhh_jms.mdl");
        if !Path::new(&mdl_path).exists() {
            println!("Skipping test_james_model_parsing: asset file not found at {mdl_path}");
            return;
        }

        let data = fs::read(&mdl_path).expect("Failed to read hhh_jms.mdl");
        let model = parse_mdl(&data).expect("Failed to parse hhh_jms.mdl");

        assert_eq!(model.bone_count, 71);
        assert_eq!(model.parents.len(), 71);
        assert_eq!(model.initial_matrices.len(), 71);
        // 15 opaque primitives + 1 transparent primitive = 16 total
        assert_eq!(model.primitives.len(), 16);
        assert_eq!(
            model
                .primitives
                .iter()
                .filter(|p| !p.is_transparent)
                .count(),
            15
        );
        assert_eq!(
            model.primitives.iter().filter(|p| p.is_transparent).count(),
            1
        );
        // 4897 opaque vertices + 378 transparent vertices = 5275 total
        assert_eq!(model.vertices.positions.len(), 5275);
        assert_eq!(model.vertices.normals.len(), 5275);
        assert_eq!(model.vertices.uvs.len(), 5275);
        assert_eq!(model.vertices.bone_indices.len(), 5275);
        assert_eq!(model.vertices.weights.len(), 5275);
        assert_eq!(
            model.textures.len(),
            4,
            "Expected 4 textures extracted from hhh_jms.mdl container"
        );
        for (&tid, tex) in &model.textures {
            let rgba = tex.to_rgba().expect("Failed to decode model texture DXT");
            assert_eq!(rgba.len(), tex.width * tex.height * 4);
            println!("Decoded model texture {tid}: {}x{}", tex.width, tex.height);
        }

        // Verify low-LOD James Sunderland model (hll_jms.mdl) with hand pose variants
        let low_mdl_path = format!("{ROOT_ASSETS}/data/chr/jms/hll_jms.mdl");
        if Path::new(&low_mdl_path).exists() {
            let low_data = fs::read(&low_mdl_path).expect("Failed to read hll_jms.mdl");
            let low_model = parse_mdl(&low_data).expect("Failed to parse hll_jms.mdl");

            assert_eq!(low_model.bone_count, 41);
            // 31 opaque primitives + 1 transparent primitive = 32 total
            assert_eq!(low_model.primitives.len(), 32);
            // 7311 opaque vertices + 378 transparent vertices = 7689 total
            assert_eq!(low_model.vertices.positions.len(), 7689);

            // Verify all vertex weights sum to 1.0 (normalized)
            for (idx, w) in low_model.vertices.weights.iter().enumerate() {
                let sum = w[0] + w[1] + w[2] + w[3];
                assert!(
                    (sum - 1.0).abs() < 1e-4,
                    "Vertex {idx} weights not normalized: sum = {sum}"
                );
            }

            // Verify hand poses 3..20 exist in primitives
            let pose_indices: Vec<u8> = low_model.primitives.iter().map(|p| p.pose_index).collect();
            assert!(pose_indices.contains(&3), "Right hand pose 3 must exist");
            assert!(pose_indices.contains(&14), "Left hand pose 14 must exist");
        }

        // Verify Maria character models (hhh_mar.mdl and lll_mar.mdl)
        let mar_high_path = format!("{ROOT_ASSETS}/data/chr/mar/hhh_mar.mdl");
        if Path::new(&mar_high_path).exists() {
            let mar_data = fs::read(&mar_high_path).expect("Failed to read hhh_mar.mdl");
            let mar_model = parse_mdl(&mar_data).expect("Failed to parse hhh_mar.mdl");

            assert_eq!(mar_model.bone_count, 73);
            // 25 opaque primitives + 5 transparent hair/eyelash primitives = 30 total
            assert_eq!(mar_model.primitives.len(), 30);
            assert_eq!(
                mar_model
                    .primitives
                    .iter()
                    .filter(|p| !p.is_transparent)
                    .count(),
                25
            );
            assert_eq!(
                mar_model
                    .primitives
                    .iter()
                    .filter(|p| p.is_transparent)
                    .count(),
                5
            );
            // 4940 opaque vertices + 331 transparent vertices = 5271 total
            assert_eq!(mar_model.vertices.positions.len(), 5271);

            for (idx, w) in mar_model.vertices.weights.iter().enumerate() {
                let sum = w[0] + w[1] + w[2] + w[3];
                assert!(
                    (sum - 1.0).abs() < 1e-4,
                    "Maria hhh vertex {idx} weights not normalized: sum = {sum}"
                );
            }
        }
    }

    #[test]
    fn test_animation_parsing() {
        // Test parsing James rowing animation (jms_boat.anm)
        let anm_path = format!("{ROOT_ASSETS}/data/chr/jms/jms_boat.anm");
        let mdl_path = format!("{ROOT_ASSETS}/data/chr/jms/hll_jms.mdl");
        if !Path::new(&anm_path).exists() || !Path::new(&mdl_path).exists() {
            println!("Skipping test_animation_parsing: assets not found");
            return;
        }

        let mdl_data = fs::read(&mdl_path).expect("Failed to read hll_jms.mdl");
        let model = parse_mdl(&mdl_data).expect("Failed to parse hll_jms.mdl");

        let anm_data = fs::read(&anm_path).expect("Failed to read jms_boat.anm");
        let anm = parse_anm(&anm_data, model.bone_count, &model.parents)
            .expect("Failed to parse jms_boat.anm");

        assert_eq!(anm.bone_count, model.bone_count);
        assert!(!anm.frames.is_empty(), "Animation should contain frames");

        let jms_basic_path = format!("{ROOT_ASSETS}/data/chr/jms/jms_basic1.anm");
        if Path::new(&jms_basic_path).exists() {
            let b_data = fs::read(&jms_basic_path).expect("read jms_basic1");
            let b_anm =
                parse_anm(&b_data, model.bone_count, &model.parents).expect("parse jms_basic1");
            println!("JMS_BASIC1 total frames: {}", b_anm.frames.len());

            // Check where discontinuities or keyframe changes occur
            let mut diffs = Vec::new();
            for i in 1..b_anm.frames.len() {
                // compute root translation delta or bone 0/1 delta
                let f0 = &b_anm.frames[i - 1];
                let f1 = &b_anm.frames[i];
                let mut max_rot_diff: f32 = 0.0;
                for b in 0..model.bone_count {
                    if let (Some(t0), Some(t1)) = (&f0.bones[b], &f1.bones[b]) {
                        if let (Some(r0), Some(r1)) = (t0.rotation, t1.rotation) {
                            let d = (r0[0] - r1[0])
                                .abs()
                                .max((r0[1] - r1[1]).abs())
                                .max((r0[2] - r1[2]).abs());
                            if d > max_rot_diff {
                                max_rot_diff = d;
                            }
                        }
                    }
                }
                if max_rot_diff > 1.2 {
                    diffs.push(i);
                }
            }
            println!("JMS_BASIC1 clip cuts (>1.2 rad jump): {:?}", diffs);
        }
    }

    #[test]
    fn test_texture_container_parsing_and_tbn2() {
        let tbn2_path = format!("{ROOT_ASSETS}/data/pic/effect/fog.tbn2");
        if !Path::new(&tbn2_path).exists() {
            println!("Skipping test_texture_container_parsing_and_tbn2: asset file not found");
            return;
        }

        let data = fs::read(&tbn2_path).expect("Failed to read fog.tbn2");
        let textures =
            parse_texture_container(&data, 0).expect("Failed to parse fog.tbn2 container");
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
            assert_eq!((w, h), (4, 4));
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
    fn test_sddata_audio_parsing() {
        let sddata_path = format!("{ROOT_ASSETS}/data/sound/sddata.bin");
        if !Path::new(&sddata_path).exists() {
            println!("Skipping test_sddata_audio_parsing: sddata.bin not found");
            return;
        }

        let data = fs::read(&sddata_path).expect("Failed to read sddata.bin");
        // Read first 1MB for fast unit test verification
        let sample_slice = &data[..1_000_000.min(data.len())];
        let bank = parse_sddata(sample_slice).expect("Failed to parse sound bank");
        assert!(
            !bank.entries.is_empty(),
            "Expected WAV entries in sddata slice"
        );
    }

    #[test]
    fn test_mes_dialogue_parsing() {
        let mes_path = format!("{ROOT_ASSETS}/sh2e/etc/message/stage_toilet_msg_e.mes");
        if !Path::new(&mes_path).exists() {
            println!("Skipping test_mes_dialogue_parsing: mes file not found at {mes_path}");
            return;
        }

        let data = fs::read(&mes_path).expect("Failed to read stage_toilet_msg_e.mes");
        let table = parse_mes(&data).expect("Failed to parse .mes file");

        assert!(
            !table.messages.is_empty(),
            "Expected messages in .mes table"
        );
        let msg0 = table.get(0).expect("Expected message 0");
        assert!(
            msg0.text.contains("Mary..."),
            "Message 0 should contain 'Mary...', found: {}",
            msg0.text
        );
    }

    #[test]
    fn test_adx_audio_decoding() {
        let adx_path = format!("{ROOT_ASSETS}/sh2e/sound/adx/apart/bgm_001.adx");
        if !Path::new(&adx_path).exists() {
            println!("Skipping test_adx_audio_decoding: adx file not found at {adx_path}");
            return;
        }

        let data = fs::read(&adx_path).expect("Failed to read bgm_001.adx");
        let header = parse_adx_header(&data).expect("Failed to parse ADX header");
        assert_eq!(header.channels, 2);
        assert_eq!(header.sample_rate, 48000);
        assert!(header.has_loop);
        assert!(header.loop_start_sample > 0);
        assert!(header.loop_end_sample > header.loop_start_sample);

        // Decode first slice (e.g. 50,000 bytes) to test decode pipeline speed
        let test_slice_len = 50_000.min(data.len());
        let mut truncated_data = data[..test_slice_len].to_vec();
        // adjust total samples in header to match available packets
        let avail_packets = (test_slice_len - header.data_offset) / (18 * header.channels as usize);
        let avail_samples = (avail_packets * 32) as u32;
        truncated_data[12..16].copy_from_slice(&avail_samples.to_be_bytes());

        let decoded = decode_adx(&truncated_data).expect("Failed to decode ADX slice");
        assert_eq!(decoded.frames.len(), avail_samples as usize);
    }

    #[test]
    fn test_sdm_sdb_parsing() {
        // Construct synthetic SDM header and SDB payload
        let mut sdm = Vec::new();
        sdm.extend_from_slice(b"SDM1");
        sdm.extend_from_slice(&2u32.to_le_bytes()); // 2 entries

        // Entry 0: 22050 Hz, offset 0, len 64, mono 16-bit, no loop
        sdm.extend_from_slice(&22050u32.to_le_bytes());
        sdm.extend_from_slice(&0u32.to_le_bytes());
        sdm.extend_from_slice(&64u32.to_le_bytes());
        sdm.extend_from_slice(&1u16.to_le_bytes());
        sdm.extend_from_slice(&16u16.to_le_bytes());
        sdm.extend_from_slice(&0u16.to_le_bytes()); // loop
        sdm.extend_from_slice(&0u16.to_le_bytes()); // pad
        sdm.extend_from_slice(&0u32.to_le_bytes()); // loop_start
        sdm.extend_from_slice(&0u32.to_le_bytes()); // loop_end

        // Entry 1: 44100 Hz, offset 64, len 128, stereo 16-bit, loop
        sdm.extend_from_slice(&44100u32.to_le_bytes());
        sdm.extend_from_slice(&64u32.to_le_bytes());
        sdm.extend_from_slice(&128u32.to_le_bytes());
        sdm.extend_from_slice(&2u16.to_le_bytes());
        sdm.extend_from_slice(&16u16.to_le_bytes());
        sdm.extend_from_slice(&1u16.to_le_bytes()); // loop
        sdm.extend_from_slice(&0u16.to_le_bytes()); // pad
        sdm.extend_from_slice(&100u32.to_le_bytes()); // loop_start
        sdm.extend_from_slice(&2000u32.to_le_bytes()); // loop_end

        let sdb = vec![0x42u8; 256];

        let bank = parse_sdm_sdb(&sdm, &sdb).expect("Failed to parse SDM/SDB");
        assert_eq!(bank.entries.len(), 2);
        assert_eq!(bank.entries[0].sample_rate, 22050);
        assert_eq!(bank.entries[0].data.len(), 64);
        assert!(!bank.entries[0].loops);

        assert_eq!(bank.entries[1].sample_rate, 44100);
        assert_eq!(bank.entries[1].data.len(), 128);
        assert!(bank.entries[1].loops);
        assert_eq!(bank.entries[1].loop_start, 100);
    }

    #[test]
    fn test_sh2_36_bestiary_catalog_models_and_animations_parsing() {
        if !Path::new(&format!("{ROOT_ASSETS}/data")).exists() {
            println!("Skipping test_sh2_36_bestiary_catalog_models_and_animations_parsing: retail assets not present");
            return;
        }

        // Verify every single entry in `sh2_debug::BESTIARY_CATALOG` (29 models across
        // Protagonists, Story NPCs, Enemies, Bosses, Weapons & Key Items):
        let catalog_samples: &[(&str, Option<&str>)] = &[
            // 1. Playable Protagonists
            (
                "data/chr/jms/hhh_jms.mdl",
                Some("data/chr/jms/jms_basic1.anm"),
            ),
            (
                "data/chr/jms/hll_jms.mdl",
                Some("data/chr/jms/jms_basic1.anm"),
            ),
            (
                "data/chr2/mar/lxx_mar.mdl",
                Some("data/chr2/mar/xmar_wpnone.anm"),
            ),
            (
                "data/chr/mar/hhh_mar.mdl",
                Some("data/chr/mar/p_hhh_mar.anm"),
            ),
            ("data/chr/mar/lll_mar.mdl", Some("data/chr/mar/lll_mar.anm")),
            // 2. Story NPCs
            ("data/chr/edi/hhh_edi.mdl", Some("data/chr/edi/p_edi.anm")),
            ("data/chr/edi/lll_edi.mdl", Some("data/chr/edi/lll_edi.anm")),
            ("data/chr/lau/lau.mdl", Some("data/chr/lau/p_lau.anm")),
            ("data/chr/mry/mry.mdl", Some("data/chr/mry/p_mry.anm")),
            ("data/chr/agl/agl.mdl", Some("data/chr/agl/p_agl.anm")),
            // 3. Enemies
            ("data/chr/scu/scu.mdl", Some("data/chr/scu/scu.anm")),
            ("data/chr/mkn/mkn.mdl", Some("data/chr/mkn/mkn.anm")),
            ("data/chr/nse/nse.mdl", Some("data/chr/nse/nse.anm")),
            ("data/chr/xoo/xoo.mdl", Some("data/chr/xoo/xoo.anm")),
            ("data/chr/arm/arm.mdl", Some("data/chr/arm/arm.anm")),
            ("data/chr/tyu/tyu.mdl", Some("data/chr/tyu/tyu.anm")),
            // 4. Bosses
            ("data/chr/red/red.mdl", Some("data/chr/red/red.anm")),
            ("data/chr/oni/oni.mdl", Some("data/chr/oni/oni.anm")),
            ("data/chr/ike/ike.mdl", Some("data/chr/ike/ike.anm")),
            ("data/chr/pap/pap.mdl", Some("data/chr/pap/pap.anm")),
            ("data/chr/bos/bos.mdl", Some("data/chr/bos/bos.anm")),
            // 5. Weapons & Key Items
            ("data/chr/wp/wp_pipe.mdl", None),
            ("data/chr/wp/wp_kakuzai.mdl", None),
            ("data/chr/wp/wp_handgun.mdl", None),
            ("data/chr/wp/wp_shotgun.mdl", None),
            ("data/chr/wp/wp_riflgun.mdl", None),
            ("data/chr/wp/wp_nata.mdl", None),
            ("data/chr/wp/wp_sp.mdl", None),
            ("data/chr/wp/wp_csaw.mdl", None),
            ("data/chr/item/i_letter.mdl", None),
        ];

        for &(mdl_rel, anm_opt) in catalog_samples {
            let mdl_full = format!("{ROOT_ASSETS}/{mdl_rel}");
            assert!(
                Path::new(&mdl_full).exists(),
                "Expected Bestiary model at {mdl_full}"
            );
            let mdl_bytes = fs::read(&mdl_full).expect("Failed to read model file");
            let model = parse_mdl(&mdl_bytes)
                .unwrap_or_else(|e| panic!("Failed to parse {mdl_rel}: {e:?}"));

            assert!(model.bone_count > 0, "{mdl_rel} must have at least 1 bone");
            assert!(
                !model.primitives.is_empty(),
                "{mdl_rel} must have primitives"
            );
            assert!(
                !model.vertices.positions.is_empty(),
                "{mdl_rel} must have vertices"
            );

            let tex_ids: Vec<u32> = model.textures.keys().copied().collect();
            let prim_tex_ids: Vec<u16> = model.primitives.iter().map(|p| p.texture_index).collect();
            println!(
                "Model {mdl_rel}: bones={}, prims={}, verts={}, tex_ids={tex_ids:?}, prim_tex_ids={prim_tex_ids:?}",
                model.bone_count,
                model.primitives.len(),
                model.vertices.positions.len()
            );

            if let Some(anm_rel) = anm_opt {
                let anm_full = format!("{ROOT_ASSETS}/{anm_rel}");
                assert!(
                    Path::new(&anm_full).exists(),
                    "Expected Bestiary animation at {anm_full}"
                );
                let anm_bytes = fs::read(&anm_full).expect("Failed to read animation file");
                let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents)
                    .unwrap_or_else(|e| panic!("Failed to parse {anm_rel}: {e:?}"));
                assert!(
                    !anm.frames.is_empty(),
                    "{anm_rel} must have animation frames"
                );
            }

            // Verify embedded texture mapping and DXT decoding
            for pr in &model.primitives {
                let tid = pr.texture_index as u32;
                if !model.textures.is_empty() {
                    let tex = model.textures.get(&tid).unwrap_or_else(|| {
                        panic!(
                            "{mdl_rel}: primitive texture_index={tid} not found in model.textures (keys={:?})",
                            model.textures.keys().collect::<Vec<_>>()
                        )
                    });
                    let rgba = tex.to_rgba().unwrap_or_else(|e| {
                        panic!("{mdl_rel}: failed to decode texture {tid}: {e:?}")
                    });
                    assert_eq!(rgba.len(), tex.width * tex.height * 4);
                }
            }
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
        let mut stream =
            BinkVideoStream::open(&flash_ee).expect("Failed to open sh2e/movie/flash.bik");
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

    #[test]
    fn test_aix_container_demux_and_decoding() {
        let resolver = sh2_assets::AssetPathResolver::resolve();
        let sound_dir = resolver.data_dir.join("sound/adx");
        if !sound_dir.exists() {
            println!("Skipping test_aix_container_demux_and_decoding: sound/adx not found");
            return;
        }

        // 1. Test bgm_118.aix (apart/bgm_118.aix) - multi-stream 5-channel ambient
        let bgm_118_path = sound_dir.join("apart/bgm_118.aix");
        if bgm_118_path.exists() {
            let data = fs::read(&bgm_118_path).expect("Failed to read bgm_118.aix");
            let raw_streams = formats::aix::demux_aix(&data).expect("Failed to demux bgm_118.aix");
            assert_eq!(raw_streams.len(), 5);

            // Test decoding full mixed ADX stereo
            let mixed =
                formats::aix::decode_aix_mixed(&data).expect("Failed to decode mixed bgm_118.aix");
            assert_eq!(mixed.header.sample_rate, 24000);
            assert_eq!(mixed.header.channels, 2);
            assert!(!mixed.frames.is_empty());
        }

        // 2. Test bgm_114_b.aix (forest/bgm_114_b.aix) - 6-stream 44.1kHz container
        let bgm_114_b_path = sound_dir.join("forest/bgm_114_b.aix");
        if bgm_114_b_path.exists() {
            let data = fs::read(&bgm_114_b_path).expect("Failed to read bgm_114_b.aix");
            let raw_streams =
                formats::aix::demux_aix(&data).expect("Failed to demux bgm_114_b.aix");
            assert_eq!(raw_streams.len(), 6);

            let mixed = formats::aix::decode_aix_mixed(&data)
                .expect("Failed to decode mixed bgm_114_b.aix");
            assert_eq!(mixed.header.sample_rate, 44100);
            assert_eq!(mixed.header.channels, 2);
            assert!(!mixed.frames.is_empty());
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
    fn test_sh2_34_primary_npc_models_reflection_fallbacks_and_animations() {
        use sh2_assets::formats::mdls::companion_texture_mdl_path;

        // Verify companion texture path mapping for `no_tex = 1` reflection/variant models
        assert_eq!(
            companion_texture_mdl_path("data/chr/agl/ragl.mdl"),
            Some("data/chr/agl/agl.mdl")
        );
        assert_eq!(
            companion_texture_mdl_path("data/chr/edi/rhhh_edi.mdl"),
            Some("data/chr/edi/hhh_edi.mdl")
        );
        assert_eq!(
            companion_texture_mdl_path("data/chr/edi/hhh_edi_notex.mdl"),
            Some("data/chr/edi/hhh_edi.mdl")
        );
        assert_eq!(
            companion_texture_mdl_path("data/chr/item/rlau.mdl"),
            Some("data/chr/lau/lau.mdl")
        );
        assert_eq!(
            companion_texture_mdl_path("data/chr/item/rmry.mdl"),
            Some("data/chr/mry/mry.mdl")
        );

        let agl_path = format!("{ROOT_ASSETS}/data/chr/agl/agl.mdl");
        if !Path::new(&agl_path).exists() {
            println!("Skipping SH2-34 retail asset test: retail assets not present");
            return;
        }

        // Ground-truth primary NPC skeleton & geometry specifications from PS2 SLUS_202.28:
        let primary_models = [
            ("data/chr/agl/agl.mdl", 75usize, 105usize, 19usize),
            ("data/chr/edi/hhh_edi.mdl", 64, 85, 31),
            ("data/chr/edi/lll_edi.mdl", 35, 47, 26),
            ("data/chr/lau/lau.mdl", 89, 167, 21),
            ("data/chr/mry/mry.mdl", 81, 94, 22),
        ];

        for (rel_mdl, exp_bones, exp_pairs, exp_prims) in primary_models {
            let bytes = fs::read(format!("{ROOT_ASSETS}/{rel_mdl}"))
                .expect("Failed to read primary NPC mdl");
            let mdl = parse_mdl(&bytes).expect("Failed to parse primary NPC mdl");
            assert_eq!(
                mdl.bone_count, exp_bones,
                "Bone count mismatch for {rel_mdl}"
            );
            assert_eq!(
                mdl.pairs.len(),
                exp_pairs,
                "Bone pair mismatch for {rel_mdl}"
            );
            assert_eq!(
                mdl.primitives.len(),
                exp_prims,
                "Primitive count mismatch for {rel_mdl}"
            );
            assert!(
                !mdl.textures.is_empty(),
                "Expected embedded textures in {rel_mdl}"
            );
            assert_eq!(mdl.parents[0], 255);
            assert_eq!(mdl.parents[1], 255);

            // Verify every primitive's texture_index maps to a valid embedded texture
            for prim in &mdl.primitives {
                let tid = prim.texture_index as u32;
                assert!(
                    mdl.textures.contains_key(&tid),
                    "Primary model {rel_mdl} primitive texture_index {tid} missing in embedded textures"
                );
            }

            // Verify every embedded texture decodes cleanly to RGBA8
            for tex in mdl.textures.values() {
                let rgba = tex.to_rgba().expect("Failed to decode NPC DXT texture");
                assert_eq!(rgba.len(), tex.width * tex.height * 4);
            }
        }

        // Verify reflection/untextured models (`no_tex = 1`) match companion texture IDs 1:1
        let reflection_models = [
            "data/chr/agl/ragl.mdl",
            "data/chr/edi/rhhh_edi.mdl",
            "data/chr/edi/hhh_edi_notex.mdl",
            "data/chr/item/rlau.mdl",
            "data/chr/item/rmry.mdl",
        ];

        for refl_rel in reflection_models {
            let refl_bytes = fs::read(format!("{ROOT_ASSETS}/{refl_rel}"))
                .expect("Failed to read reflection mdl");
            let refl_mdl = parse_mdl(&refl_bytes).expect("Failed to parse reflection mdl");
            assert!(
                refl_mdl.textures.is_empty(),
                "Expected no_tex=1 reflection model {refl_rel} to have 0 embedded textures"
            );

            let comp_rel = companion_texture_mdl_path(refl_rel).expect("Missing companion path");
            let comp_bytes = fs::read(format!("{ROOT_ASSETS}/{comp_rel}"))
                .expect("Failed to read companion mdl");
            let comp_mdl = parse_mdl(&comp_bytes).expect("Failed to parse companion mdl");
            assert_eq!(refl_mdl.bone_count, comp_mdl.bone_count);

            for prim in &refl_mdl.primitives {
                let tid = prim.texture_index as u32;
                assert!(
                    comp_mdl.textures.contains_key(&tid),
                    "Reflection model {refl_rel} primitive texture_index {tid} missing in companion {comp_rel}"
                );
            }
        }

        // Verify delta/hold frame persistence (`t == 0` in PS2 `shExec` at `0x001002c0`):
        // In `data/demo/knife_agl/agl.anm` (Mirror Room cutscene), keyframes at 10-15 Hz are
        // followed by 1-2 hold frames (`t == 0` for all 75 bones). Verify that held frames retain
        // the previous frame's bone transforms and never snap back to `None` / standing bind pose.
        let agl_mdl =
            parse_mdl(&fs::read(&agl_path).expect("read agl.mdl")).expect("parse agl.mdl");
        let knife_bytes = fs::read(format!("{ROOT_ASSETS}/data/demo/knife_agl/agl.anm"))
            .expect("read knife_agl/agl.anm");
        let knife_anm = parse_anm(&knife_bytes, agl_mdl.bone_count, &agl_mdl.parents)
            .expect("parse knife_agl/agl.anm");
        assert_eq!(knife_anm.frames.len(), 6371);
        for f_idx in 217..=390 {
            let frame = &knife_anm.frames[f_idx];
            for b_idx in 0..agl_mdl.bone_count {
                let bt = frame.bones[b_idx].as_ref().unwrap_or_else(|| {
                    panic!("knife_agl/agl.anm frame {f_idx} bone {b_idx} lost held transform")
                });
                assert!(
                    bt.rotation.is_some() && bt.translation.is_some(),
                    "knife_agl/agl.anm frame {f_idx} bone {b_idx} missing held rot/trans"
                );
            }
        }
    }
}
