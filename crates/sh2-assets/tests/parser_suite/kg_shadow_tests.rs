//! Retail integration tests for `.kg1` character shadow models, `.kg2` room shadow models,
//! `lens_flare.tbn2`, and `poison.tbn2` (`SH2-81`).

use super::ROOT_ASSETS;
use sh2_assets::formats::kg::{parse_kg1, parse_kg2};
use sh2_assets::formats::texture::parse_texture_container;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_files_with_ext(root: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_ext(&path, ext, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        {
            out.push(path);
        }
    }
}

#[test]
fn parse_all_retail_kg1_character_shadow_models() {
    let data_dir = Path::new(ROOT_ASSETS).join("data");
    if !data_dir.exists() {
        return;
    }

    let mut files = Vec::new();
    collect_files_with_ext(&data_dir, "kg1", &mut files);
    assert!(
        files.len() >= 100,
        "Expected >= 100 retail .kg1 files, found {}",
        files.len()
    );

    let mut with_parts = 0usize;
    let mut total_tris = 0usize;
    for path in &files {
        let bytes = fs::read(path).expect("read retail .kg1");
        let kg = parse_kg1(&bytes).unwrap_or_else(|e| panic!("failed to parse {path:?}: {e}"));
        if !kg.parts.is_empty() {
            with_parts += 1;
            total_tris += kg.total_triangle_count();
        }
    }
    assert!(with_parts >= 60);
    assert!(total_tris > 5_000);
}

#[test]
fn parse_all_retail_kg2_room_shadow_models() {
    let data_dir = Path::new(ROOT_ASSETS).join("data");
    if !data_dir.exists() {
        return;
    }

    let mut files = Vec::new();
    collect_files_with_ext(&data_dir, "kg2", &mut files);
    assert!(
        files.len() >= 300,
        "Expected >= 300 retail .kg2 files, found {}",
        files.len()
    );

    let mut total_parts = 0usize;
    let mut total_tris = 0usize;
    for path in &files {
        let bytes = fs::read(path).expect("read retail .kg2");
        let kg = parse_kg2(&bytes).unwrap_or_else(|e| panic!("failed to parse {path:?}: {e}"));
        total_parts += kg.parts.len();
        total_tris += kg.total_triangle_count();
    }
    assert!(total_parts > 1000);
    assert!(total_tris > 5000);
}

#[test]
fn parse_retail_lens_flare_and_spray_textures() {
    let effect_dir = Path::new(ROOT_ASSETS).join("data/pic/effect");
    if !effect_dir.exists() {
        return;
    }

    let flare_bytes = fs::read(effect_dir.join("lens_flare.tbn2")).expect("read lens_flare.tbn2");
    let flare_texs = parse_texture_container(&flare_bytes, 0).expect("parse lens_flare.tbn2");
    assert!(
        !flare_texs.is_empty(),
        "lens_flare.tbn2 must contain at least 1 texture"
    );
    assert_eq!(flare_texs[0].width, 128);
    assert_eq!(flare_texs[0].height, 128);

    let spray_bytes = fs::read(effect_dir.join("poison.tbn2")).expect("read poison.tbn2");
    let spray_texs = parse_texture_container(&spray_bytes, 0).expect("parse poison.tbn2");
    assert!(
        !spray_texs.is_empty(),
        "poison.tbn2 must contain at least 1 texture"
    );
}
