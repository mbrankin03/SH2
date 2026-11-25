//! Visual & pixel-bound parity verification for Title Screen (`start00.tex` + `start01e.tex`)
//! and Options Menu (`otn_option.c`).

use sh2_assets::{AssetPathResolver, TextureLoader};
use sh2_core::config::AppConfig;
use sh2_ui::button_atlas::ButtonSpriteSheet;
use sh2_ui::font::TextRasterizer;
use sh2_ui::menu::{
    draw_difficulty_selection, draw_pulsing_title_logo_for_bg, draw_scenario_selection,
    draw_title_screen, TitlePhase, TitleStateMachine,
};
use sh2_ui::options::{draw_options_screen, OptionsMenu};
use sh2_ui::AtlasFont;

fn save_png(path: &str, rast: &TextRasterizer) {
    let img =
        image::RgbaImage::from_raw(rast.width as u32, rast.height as u32, rast.pixels.clone())
            .expect("valid rgba buffer");
    img.save(path).expect("save png");
}

fn fill_bg_cropped(
    rast: &mut TextRasterizer,
    bg_rgba: &[u8],
    bg_w: usize,
    bg_h: usize,
    uv_rect: (f32, f32, f32, f32),
) {
    let (u0, v0, u1, v1) = uv_rect;
    for y in 0..rast.height {
        let v = v0 + (y as f32 / rast.height as f32) * (v1 - v0);
        let sy = ((v * bg_h as f32).floor() as usize).min(bg_h - 1);
        for x in 0..rast.width {
            let u = u0 + (x as f32 / rast.width as f32) * (u1 - u0);
            let sx = ((u * bg_w as f32).floor() as usize).min(bg_w - 1);
            let src = (sy * bg_w + sx) * 4;
            let dst = (y * rast.width + x) * 4;
            rast.pixels[dst..dst + 4].copy_from_slice(&bg_rgba[src..src + 4]);
        }
    }
}

#[test]
fn render_title_and_options_headless_snapshots() {
    let resolver = AssetPathResolver::resolve();
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    let Ok(start00) = loader.load_texture("pic/etc/start00.tex") else {
        return;
    };
    let Ok(start01e) = loader.load_texture("pic/etc/start01e.tex") else {
        return;
    };
    let mut atlas = ButtonSpriteSheet::new(start01e.width, start01e.height, start01e.rgba);
    let font = AtlasFont::shared(None);
    let out_dir = std::env::var("SH2_PREVIEW_OUT").ok();

    // 16:9 aspect-preserving UV crop for 2732x2048 start00.tex
    let tex_aspect = 2732.0 / 2048.0;
    let win_aspect = 16.0 / 9.0;
    let v_span = tex_aspect / win_aspect;
    let pad = (1.0 - v_span) * 0.5;
    let uv_rect = (0.0f32, pad, 1.0f32, 1.0 - pad);

    // 1. Main Menu (16:9, bright logo phase)
    let mut sm = TitleStateMachine::new();
    sm.show_phase_immediately(TitlePhase::MainMenu);
    sm.elapsed_timer = 0.0; // bright white logo phase
    let mut rast_main = TextRasterizer::new(1280, 720);
    fill_bg_cropped(
        &mut rast_main,
        &start00.rgba,
        start00.width,
        start00.height,
        uv_rect,
    );
    let full_vp = (0.0f32, 0.0f32, 1280.0f32, 720.0f32);
    draw_pulsing_title_logo_for_bg(&sm, &atlas, &mut rast_main, uv_rect, full_vp, full_vp);
    draw_title_screen(&sm, Some(&atlas), font, &mut rast_main);
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/title_main_menu_16x9.png"), &rast_main);
    }

    // 2. Scenario Select (16:9, mid-cycle logo phase)
    sm.show_phase_immediately(TitlePhase::ScenarioSelect);
    sm.elapsed_timer = 2.5;
    let mut rast_scen = TextRasterizer::new(1280, 720);
    fill_bg_cropped(
        &mut rast_scen,
        &start00.rgba,
        start00.width,
        start00.height,
        uv_rect,
    );
    draw_pulsing_title_logo_for_bg(&sm, &atlas, &mut rast_scen, uv_rect, full_vp, full_vp);
    draw_scenario_selection(&sm, Some(&atlas), font, &mut rast_scen);
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/title_scenario_16x9.png"), &rast_scen);
    }

    // 3. Difficulty Select (RiddleDifficultySelect — only confirmed Action row visible)
    sm.show_phase_immediately(TitlePhase::RiddleDifficultySelect);
    sm.elapsed_timer = 1.2;
    let mut rast_diff = TextRasterizer::new(1280, 720);
    fill_bg_cropped(
        &mut rast_diff,
        &start00.rgba,
        start00.width,
        start00.height,
        uv_rect,
    );
    draw_pulsing_title_logo_for_bg(&sm, &atlas, &mut rast_diff, uv_rect, full_vp, full_vp);
    draw_difficulty_selection(&sm, Some(&atlas), font, &mut rast_diff);
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/title_difficulty_16x9.png"), &rast_diff);
    }

    // 4. 10-Row Root O P T I O N S screen
    let opt_menu = OptionsMenu::new(AppConfig::default());
    let mut rast_opt = TextRasterizer::new(1280, 720);
    rast_opt.fill_rect(0, 0, 1280, 720, [0, 0, 0, 255]);
    draw_options_screen(&opt_menu, font, &mut rast_opt);
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/options_root_10row.png"), &rast_opt);
        let mut adv_menu = OptionsMenu::new(AppConfig::default());
        adv_menu.navigate_vertical(1); // Advanced Options
        adv_menu.enter_submenu();
        adv_menu.handle_mouse_wheel(3, Some((256.0, 150.0)));
        let mut rast_adv = TextRasterizer::new(1280, 720);
        rast_adv.fill_rect(0, 0, 1280, 720, [0, 0, 0, 255]);
        draw_options_screen(&adv_menu, font, &mut rast_adv);
        save_png(&format!("{dir}/options_advanced_scrolled.png"), &rast_adv);
        let ps2_dir = resolver.game_root.join("sh2e/ps2");
        let ps2_loader = TextureLoader::new(&ps2_dir, None::<&std::path::Path>);
        if let (Some(s0), Some(s1)) = (
            loader
                .load_4k_named_override("ps2", "etc/start00")
                .or_else(|| ps2_loader.load_texture("etc/start00.tex").ok()),
            loader
                .load_4k_named_override("ps2", "etc/start01")
                .or_else(|| ps2_loader.load_texture("etc/start01.tex").ok()),
        ) {
            atlas.set_original_title_logo(s1.width, s1.height, s1.rgba);
            let mut sm_orig = TitleStateMachine::new();
            sm_orig.show_phase_immediately(TitlePhase::MainMenu);
            sm_orig.elapsed_timer = 0.0;
            let mut rast_orig = TextRasterizer::new(1280, 720);
            fill_bg_cropped(&mut rast_orig, &s0.rgba, s0.width, s0.height, uv_rect);
            // White pulsing logo overlay is SH2:EE-only; Original PS2 splash draws only the 4K pack background + menu buttons.
            draw_title_screen(&sm_orig, Some(&atlas), font, &mut rast_orig);
            save_png(&format!("{dir}/title_main_menu_original.png"), &rast_orig);
        }
    }
}
