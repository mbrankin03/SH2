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
        assert_eq!(model.primitives.len(), 15);
        assert_eq!(model.vertices.positions.len(), 4897);
        assert_eq!(model.vertices.normals.len(), 4897);
        assert_eq!(model.vertices.uvs.len(), 4897);
        assert_eq!(model.vertices.bone_indices.len(), 4897);
        assert_eq!(model.vertices.weights.len(), 4897);
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
            assert_eq!(low_model.primitives.len(), 31);
            assert_eq!(low_model.vertices.positions.len(), 7311);

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
            assert_eq!(mar_model.primitives.len(), 25);
            assert_eq!(mar_model.vertices.positions.len(), 4940);

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

        // Verify representative models across all 5 Bestiary categories:
        // Protagonists (hhh_jms, lll_mar), Story NPCs (hhh_edi, lau, mry, agl),
        // Enemies (scu, mkn, nse, xoo, arm, tyu), Bosses (red, oni, ike, pap, bos),
        // Weapons & Items (wp_pipe, wp_handgun, wp_shotgun, wp_nata, i_letter)
        let catalog_samples: &[(&str, Option<&str>)] = &[
            ("data/chr/edi/hhh_edi.mdl", Some("data/chr/edi/lll_edi.anm")),
            ("data/chr/lau/lau.mdl", Some("data/chr/lau/p_lau.anm")),
            ("data/chr/mry/mry.mdl", Some("data/chr/mry/p_mry.anm")),
            ("data/chr/agl/agl.mdl", Some("data/chr/agl/p_agl.anm")),
            ("data/chr/scu/scu.mdl", Some("data/chr/scu/scu.anm")),
            ("data/chr/mkn/mkn.mdl", Some("data/chr/mkn/mkn.anm")),
            ("data/chr/nse/nse.mdl", Some("data/chr/nse/nse.anm")),
            ("data/chr/xoo/xoo.mdl", Some("data/chr/xoo/xoo.anm")),
            ("data/chr/arm/arm.mdl", Some("data/chr/arm/arm.anm")),
            ("data/chr/tyu/tyu.mdl", Some("data/chr/tyu/tyu.anm")),
            ("data/chr/red/red.mdl", Some("data/chr/red/red.anm")),
            ("data/chr/oni/oni.mdl", Some("data/chr/oni/oni.anm")),
            ("data/chr/ike/ike.mdl", Some("data/chr/ike/ike.anm")),
            ("data/chr/pap/pap.mdl", Some("data/chr/pap/pap.anm")),
            ("data/chr/bos/bos.mdl", Some("data/chr/bos/bos.anm")),
            ("data/chr/wp/wp_pipe.mdl", None),
            ("data/chr/wp/wp_handgun.mdl", None),
            ("data/chr/wp/wp_shotgun.mdl", None),
            ("data/chr/wp/wp_nata.mdl", None),
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
        }
    }
}
