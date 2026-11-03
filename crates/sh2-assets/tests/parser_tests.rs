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
    fn test_texture_loader_and_ee_fallback() {
        let data_dir = format!("{ROOT_ASSETS}/data");
        let sh2e_dir = format!("{ROOT_ASSETS}/sh2e");

        let loader = TextureLoader::new(&data_dir, Some(&sh2e_dir));

        // Test resolving and decoding a PNG texture from sh2e pack
        let png_res = loader.load_texture("pic/etc/RedCrossIcon.png");
        assert!(
            png_res.is_ok(),
            "Failed to load RedCrossIcon.png: {:?}",
            png_res.err()
        );
        let tex = png_res.unwrap();
        assert!(tex.width > 0 && tex.height > 0);
        assert_eq!(tex.rgba.len(), tex.width * tex.height * 4);

        // Test resolving vanilla DDS
        let dds_res = loader.load_texture("demo/reizouko_a/reizouko_jms.dds");
        // May or may not be standard DDS container
        if let Ok(dds_tex) = dds_res {
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
}
