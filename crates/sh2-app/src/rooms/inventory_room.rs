//! Interactive 85-Item Catalog, 3D `.mdl` / 2D `.tex` Inspect, Combinations & Healing Debug Room (`SH2-15`).
//!
//! Provides an interactive 3D and 2D studio environment for testing all inventory and item systems:
//! 1. **Full 85-Item Catalog Browser (`1..=85`)**:
//!    - Live 3D rendering of the selected item's `.mdl` model (`data/chr/item/*.mdl` or `data/chr2/item/*.mdl`)
//!      with automatic bounding-box centering/scaling and turntable rotation.
//!    - Live 2D preview of the selected item's `.tex` inspect texture (`pic/item/*.tex` or `pic/add/*.tex`,
//!      including PS2 `0xa7a7a7a7` 8-bit swizzled CLUT textures).
//!    - Displays authentic Name, Category, Inventory Description, and Examination Text (`item_x_msg_e.mes`).
//! 2. **Inventory Limits, `Bullet Adjust` (`1x..=3x`), Healing Overheal & Secret Items**:
//!    - `[Enter / G]` to pick up (`item_get_now`), `[F]` to jump stack to `990` to test the `999` cap,
//!      `[B]` to cycle `Bullet Adjust` (`1x..=3x`), `[U]` to use healing consumables (`#1..#3`) on
//!      simulated James HP (`[-/=]` to adjust HP), and `[E]` to test item examination (`Mary` photo/letter
//!      cap at `2` & `Angela's Knife` flag `630`).
//! 3. **Interactive 11-Recipe `ITEM_COMBOS` (`86..=96`) Workbench**:
//!    - `[Left / Right]` to select a combination recipe, `[C]` to auto-grant ingredients and combine,
//!      and `[K]` to consume the combined result and verify ingredient retention (`93..=95`) vs consumption (`96`).

use std::path::PathBuf;
use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::assets::{load_character_asset, CharacterAsset};
use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_assets::formats::anm::{
    compute_skin_matrices_with_inv_into, evaluate_pose_into, interpolate_frames_into,
};
use sh2_assets::TextureLoader;
use sh2_audio::{play_menu_sound, SoundCueKind};
use sh2_core::{
    item_metadata, CATALOG_ITEM_COUNT, ITEM_CATALOG, ITEM_COMBOS, MAX_ITEM_STACK,
};
use sh2_ui::menu::TitlePhase;

/// Runtime state for [`sh2_ui::menu::TestRoomKind::InventoryAndItemsRoom`] (`SH2-15`).
pub(crate) struct InventoryDebugRoomState {
    /// Currently selected item ID (`1..=85`).
    pub selected_item_id: u8,
    /// Currently selected combination recipe index (`0..ITEM_COMBOS.len()`).
    pub selected_combo_idx: usize,
    /// Simulated player HP (`0.0..=100.0`) for testing healing items and overheal tracking.
    pub simulated_hp: f32,
    /// Maximum simulated player HP (`100.0`).
    pub max_hp: f32,
    /// Whether the 3D item model turntable rotates automatically.
    pub turntable_enabled: bool,
    /// Current turntable yaw angle in radians.
    pub turntable_yaw: f32,
    /// Camera zoom multiplier (`0.4..=2.5`).
    pub zoom_scale: f32,
    /// Currently loaded 3D/2D item ID cache key.
    pub loaded_item_id: Option<u8>,
    /// Loaded 3D `.mdl` character/item asset for the currently selected item, if present on disk.
    pub item_3d_asset: Option<CharacterAsset>,
    /// Decoded 2D `.tex` inspect texture `(width, height, rgba, format_label)` for the selected item.
    pub item_2d_tex: Option<(u32, u32, Vec<u8>, String)>,
    /// Status banner describing the most recent inventory operation.
    pub last_action_log: String,
}

impl Default for InventoryDebugRoomState {
    fn default() -> Self {
        Self {
            selected_item_id: 4, // Handgun (has both 3D x_handgun.mdl and 2D inspect texture)
            selected_combo_idx: 0,
            simulated_hp: 50.0,
            max_hp: 100.0,
            turntable_enabled: true,
            turntable_yaw: 0.4,
            zoom_scale: 1.0,
            loaded_item_id: None,
            item_3d_asset: None,
            item_2d_tex: None,
            last_action_log:
                "Press [Up/Down] to browse 85 items, [Enter/G] to pick up, [C] to test combinations"
                    .to_string(),
        }
    }
}

pub(crate) struct InventoryRoomController;

impl RoomController for InventoryRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        if !pressed {
            return;
        }
        match key_code {
            KeyCode::Escape | KeyCode::Backspace if !repeat => {
                state.exit_room_to_menu(TitlePhase::TestRoomsSelect);
                tracing::info!("Returned from Inventory & Item Inspect Room to Test Rooms menu.");
            }
            KeyCode::ArrowUp | KeyCode::KeyW => {
                let cur = state.inventory_room.selected_item_id;
                state.inventory_room.selected_item_id = if cur <= 1 {
                    CATALOG_ITEM_COUNT as u8
                } else {
                    cur - 1
                };
                play_cursor_sound(state);
            }
            KeyCode::ArrowDown | KeyCode::KeyS => {
                let cur = state.inventory_room.selected_item_id;
                state.inventory_room.selected_item_id = if cur >= CATALOG_ITEM_COUNT as u8 {
                    1
                } else {
                    cur + 1
                };
                play_cursor_sound(state);
            }
            KeyCode::PageUp => {
                let cur = state.inventory_room.selected_item_id as i32;
                let next = ((cur - 1 - 10).rem_euclid(CATALOG_ITEM_COUNT as i32) + 1) as u8;
                state.inventory_room.selected_item_id = next;
                play_cursor_sound(state);
            }
            KeyCode::PageDown => {
                let cur = state.inventory_room.selected_item_id as i32;
                let next = ((cur - 1 + 10).rem_euclid(CATALOG_ITEM_COUNT as i32) + 1) as u8;
                state.inventory_room.selected_item_id = next;
                play_cursor_sound(state);
            }
            KeyCode::ArrowLeft | KeyCode::KeyA if !repeat => {
                if state.inventory_room.selected_combo_idx == 0 {
                    state.inventory_room.selected_combo_idx = ITEM_COMBOS.len() - 1;
                } else {
                    state.inventory_room.selected_combo_idx -= 1;
                }
                play_cursor_sound(state);
            }
            KeyCode::ArrowRight | KeyCode::KeyD if !repeat => {
                state.inventory_room.selected_combo_idx =
                    (state.inventory_room.selected_combo_idx + 1) % ITEM_COMBOS.len();
                play_cursor_sound(state);
            }
            // [Enter / G]: Pick up (`item_get_now`) the selected item
            KeyCode::Enter | KeyCode::KeyG if !repeat => {
                let id = state.inventory_room.selected_item_id;
                let before = state.sim_core.scenario.inventory_mgr.count(id);
                let after = state.sim_core.scenario.inventory_mgr.item_get_now(id);
                state.sim_core.scenario.sync_legacy_views();
                let name = item_metadata(id).map(|m| m.name).unwrap_or("Unknown");
                state.inventory_room.last_action_log = format!(
                    "Picked up #{id} ({name}): count {before} -> {after} (BulletAdjust={}x, SecretMask=0x{:02X})",
                    state.sim_core.scenario.inventory_mgr.bullet_adjust(),
                    state.sim_core.scenario.inventory_mgr.secret_items_mask(),
                );
                play_confirm_sound(state);
            }
            // [Delete / X]: Remove 1 of the selected item
            KeyCode::Delete | KeyCode::KeyX if !repeat => {
                let id = state.inventory_room.selected_item_id;
                let before = state.sim_core.scenario.inventory_mgr.count(id);
                state.sim_core.scenario.inventory_mgr.item_lose(id);
                let after = state.sim_core.scenario.inventory_mgr.count(id);
                state.sim_core.scenario.sync_legacy_views();
                let name = item_metadata(id).map(|m| m.name).unwrap_or("Unknown");
                state.inventory_room.last_action_log =
                    format!("Removed #{id} ({name}): count {before} -> {after}");
                play_cancel_sound(state);
            }
            // [F]: Set selected item stack to 990 to test the 999 cap
            KeyCode::KeyF if !repeat => {
                let id = state.inventory_room.selected_item_id;
                let cur = state.sim_core.scenario.inventory_mgr.count(id) as i32;
                state.sim_core.scenario.inventory_mgr.give_raw(id, 990 - cur);
                state.sim_core.scenario.sync_legacy_views();
                let name = item_metadata(id).map(|m| m.name).unwrap_or("Unknown");
                state.inventory_room.last_action_log = format!(
                    "Set #{id} ({name}) stack to 990 (press [Enter/G] to test {MAX_ITEM_STACK} cap)"
                );
                play_confirm_sound(state);
            }
            // [B]: Cycle Bullet Adjust multiplier (1x -> 2x -> 3x)
            KeyCode::KeyB if !repeat => {
                let cur = state.sim_core.scenario.inventory_mgr.bullet_adjust();
                let next = if cur >= 3 { 1 } else { cur + 1 };
                state
                    .sim_core
                    .scenario
                    .inventory_mgr
                    .set_bullet_adjust(next);
                state.inventory_room.last_action_log = format!(
                    "Switched Bullet Adjust multiplier: {cur}x -> {next}x (Handgun={}, Shotgun={}, Rifle={}, Revolver={})",
                    10 * next,
                    6 * next,
                    4 * next,
                    6 * next
                );
                play_confirm_sound(state);
            }
            // [U]: Use healing item (#1 Health Drink, #2 First-Aid Kit, #3 Ampoule)
            KeyCode::KeyU if !repeat => {
                let id = state.inventory_room.selected_item_id;
                let heal_id = if (1..=3).contains(&id) { id } else { 1 };
                if !state.sim_core.scenario.inventory_mgr.has_item(heal_id) {
                    state.sim_core.scenario.inventory_mgr.item_get_now(heal_id);
                }
                let hp_before = state.inventory_room.simulated_hp;
                let max_hp = state.inventory_room.max_hp;
                if let Some(res) = state
                    .sim_core
                    .scenario
                    .inventory_mgr
                    .use_healing_item(heal_id, hp_before, max_hp)
                {
                    state.inventory_room.simulated_hp = res.new_hp;
                    state.sim_core.scenario.sync_legacy_views();
                    let name = item_metadata(heal_id).map(|m| m.name).unwrap_or("Heal");
                    state.inventory_room.last_action_log = format!(
                        "Used #{heal_id} ({name}): HP {hp_before:.0} -> {:.0}/{max_hp:.0} (overheal={:+.2}, total_overheal={:.2}, ampoule_timer={:.0}s)",
                        res.new_hp,
                        res.overheal_fraction,
                        state.sim_core.scenario.inventory_mgr.overheal_score(),
                        state.sim_core.scenario.inventory_mgr.ampoule_stamina_timer(),
                    );
                    play_confirm_sound(state);
                }
            }
            // [Minus / Equal]: Adjust simulated HP to test partial vs overheal restoration
            KeyCode::Minus => {
                state.inventory_room.simulated_hp =
                    (state.inventory_room.simulated_hp - 15.0).max(5.0);
                state.inventory_room.last_action_log = format!(
                    "Damaged simulated James HP -> {:.0}/{:.0}",
                    state.inventory_room.simulated_hp, state.inventory_room.max_hp
                );
            }
            KeyCode::Equal => {
                state.inventory_room.simulated_hp = (state.inventory_room.simulated_hp + 15.0)
                    .min(state.inventory_room.max_hp);
                state.inventory_room.last_action_log = format!(
                    "Increased simulated James HP -> {:.0}/{:.0}",
                    state.inventory_room.simulated_hp, state.inventory_room.max_hp
                );
            }
            // [E]: Examine selected item (Mary's photo/letter counter & Angela's knife flag 630)
            KeyCode::KeyE if !repeat => {
                let id = state.inventory_room.selected_item_id;
                if let Some((view, set_630)) =
                    state.sim_core.scenario.inventory_mgr.examine_item(id)
                {
                    if set_630 {
                        state.sim_core.scenario.flags.set(630, true);
                    }
                    let text = view
                        .examine_text
                        .unwrap_or(view.description)
                        .replace('\n', " / ");
                    state.inventory_room.last_action_log = format!(
                        "Examined #{id} ({}): \"{text}\" (MaryCount={}/2, Flag630={})",
                        view.name,
                        state.sim_core.scenario.inventory_mgr.mary_examine_count(),
                        state.sim_core.scenario.flags.get(630)
                    );
                    play_confirm_sound(state);
                }
            }
            // [C]: Grant ingredients and execute the selected ITEM_COMBOS recipe (86..=96)
            KeyCode::KeyC if !repeat => {
                let idx = state.inventory_room.selected_combo_idx % ITEM_COMBOS.len();
                let recipe = ITEM_COMBOS[idx];
                for &part in &recipe.parts {
                    if part != 0 && !state.sim_core.scenario.inventory_mgr.has_item(part) {
                        state.sim_core.scenario.inventory_mgr.item_get_now(part);
                    }
                }
                let res = state.sim_core.scenario.inventory_mgr.try_combine_owned(
                    recipe.parts[0],
                    recipe.parts[1],
                    recipe.parts[2],
                );
                state.sim_core.scenario.sync_legacy_views();
                let desc = combo_description(recipe.result_id);
                state.inventory_room.last_action_log = format!(
                    "Combined {:?} -> {:?} ({desc}) [Press [K] to use/consume result #{}]",
                    recipe.parts, res, recipe.result_id
                );
                play_confirm_sound(state);
            }
            // [K]: Use/consume the selected combination result (86..=96) to verify part consumption
            KeyCode::KeyK if !repeat => {
                let idx = state.inventory_room.selected_combo_idx % ITEM_COMBOS.len();
                let recipe = ITEM_COMBOS[idx];
                state
                    .sim_core
                    .scenario
                    .inventory_mgr
                    .item_lose(recipe.result_id);
                state.sim_core.scenario.sync_legacy_views();
                let kept: Vec<u8> = recipe
                    .parts
                    .iter()
                    .copied()
                    .filter(|&p| p != 0 && state.sim_core.scenario.inventory_mgr.has_item(p))
                    .collect();
                let desc = combo_description(recipe.result_id);
                state.inventory_room.last_action_log = format!(
                    "Used combo result #{} ({desc}): remaining parts in inventory = {:?}",
                    recipe.result_id, kept
                );
                play_cancel_sound(state);
            }
            // [T]: Toggle 3D model turntable auto-spin
            KeyCode::KeyT if !repeat => {
                state.inventory_room.turntable_enabled = !state.inventory_room.turntable_enabled;
            }
            KeyCode::KeyQ => {
                state.inventory_room.zoom_scale =
                    (state.inventory_room.zoom_scale - 0.1).max(0.4);
            }
            KeyCode::KeyR => {
                state.inventory_room.zoom_scale =
                    (state.inventory_room.zoom_scale + 0.1).min(2.5);
            }
            _ => {}
        }
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        _dt: Duration,
        dt_sec: f32,
        aspect: f32,
        _grain_pct: u32,
        is_paused: bool,
        _is_in_game: bool,
    ) {
        if !is_paused {
            state.sim_core.scenario.inventory_mgr.tick(dt_sec);
            if state.inventory_room.turntable_enabled {
                state.inventory_room.turntable_yaw =
                    (state.inventory_room.turntable_yaw + 0.95 * dt_sec) % std::f32::consts::TAU;
            }
        }

        // Hot-load 3D `.mdl` and 2D `.tex` whenever `selected_item_id` changes
        let sel_id = state.inventory_room.selected_item_id;
        if state.inventory_room.loaded_item_id != Some(sel_id) {
            state.inventory_room.loaded_item_id = Some(sel_id);
            state.inventory_room.item_3d_asset = None;
            state.inventory_room.item_2d_tex = None;

            if let Some(meta) = item_metadata(sel_id) {
                if let Some(mdl_path) = meta.model_path {
                    state.inventory_room.item_3d_asset =
                        load_character_asset(&state.render_ctx, meta.name, &[mdl_path], &[]);
                }
                if let Some(tex_path) = meta.inspect_texture_path {
                    let resolver = sh2_assets::AssetPathResolver::resolve();
                    let base_roots = [
                        resolver.game_root.clone(),
                        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
                    ];
                    for base in &base_roots {
                        let loader = TextureLoader::new(
                            base.join("data"),
                            Some(base.join("sh2e")),
                        );
                        if let Ok(cut_tex) = loader.load_cutaway_texture(tex_path, true) {
                            let src_label = if cut_tex.is_sh2ee_hd {
                                "SH2:EE HD (sh2e/pic)"
                            } else {
                                "Original PS2/PC (data/pic)"
                            };
                            state.inventory_room.item_2d_tex = Some((
                                cut_tex.texture.width as u32,
                                cut_tex.texture.height as u32,
                                cut_tex.texture.rgba,
                                src_label.to_string(),
                            ));
                            break;
                        }
                    }
                }
            }
        }

        // Frame camera so the 3D item model renders cleanly on the right side of the viewport
        let dist = 420.0 * state.inventory_room.zoom_scale;
        let eye_pos = glam::Vec3::new(-140.0, -220.0, dist);
        let target = glam::Vec3::new(-140.0, -140.0, 0.0);
        let view = glam::Mat4::look_at_rh(eye_pos, target, glam::Vec3::new(0.0, -1.0, 0.0));
        let proj = glam::Mat4::perspective_rh(
            40.0f32.to_radians(),
            aspect,
            super::DEFAULT_CAMERA_Z_NEAR,
            super::DEFAULT_CAMERA_Z_FAR,
        );
        let view_proj = proj * view;
        let inv_proj = proj.inverse();
        state
            .render_ctx
            .update_camera_with_mode(view_proj, inv_proj, eye_pos, 3.0);

        let to_target = (target - eye_pos).normalize_or_zero();
        state
            .render_ctx
            .update_spotlight(&sh2_renderer::light::SpotlightUniforms {
                position: [eye_pos.x, eye_pos.y - 250.0, eye_pos.z, 1.0],
                direction_inner: [to_target.x, to_target.y, to_target.z, 0.75],
                color_outer: [2.2, 2.15, 2.0, 0.45],
            });

        state.skin_matrices_scratch.clear();
        if let Some(ref mut c) = state.inventory_room.item_3d_asset {
            let frame_count = c.anm.frames.len();
            if frame_count > 0 {
                interpolate_frames_into(
                    &c.anm.frames[0],
                    &c.anm.frames[0],
                    0.0,
                    &mut c.frame_scratch_a,
                );
                evaluate_pose_into(
                    &c.model.parents,
                    &c.bind_locals,
                    &c.frame_scratch_a,
                    &mut c.pose_scratch,
                );
                compute_skin_matrices_with_inv_into(
                    &c.pose_scratch,
                    &c.model.inv_initial_matrices,
                    &mut c.skin_scratch,
                );

                // Compute bounding box center & uniform scale so all item models fit nicely in view
                let mut min_p = [f32::INFINITY; 3];
                let mut max_p = [f32::NEG_INFINITY; 3];
                for p in &c.model.vertices.positions {
                    for k in 0..3 {
                        min_p[k] = min_p[k].min(p[k]);
                        max_p[k] = max_p[k].max(p[k]);
                    }
                }
                let (cx, cy, cz, scale) = if min_p[0].is_finite() {
                    let dx = (max_p[0] - min_p[0]).max(1.0);
                    let dy = (max_p[1] - min_p[1]).max(1.0);
                    let dz = (max_p[2] - min_p[2]).max(1.0);
                    let max_extent = dx.max(dy).max(dz);
                    let s = (210.0 / max_extent).clamp(0.25, 4.5);
                    (
                        0.5 * (min_p[0] + max_p[0]),
                        0.5 * (min_p[1] + max_p[1]),
                        0.5 * (min_p[2] + max_p[2]),
                        s,
                    )
                } else {
                    (0.0, 0.0, 0.0, 1.0)
                };

                let (sn, cs) = state.inventory_room.turntable_yaw.sin_cos();
                let scs = scale * cs;
                let ssn = scale * sn;
                let tx = -(scs * cx + ssn * cz);
                let ty = -140.0 - scale * cy;
                let tz = -(-ssn * cx + scs * cz);

                let world_mat = [
                    [scs, 0.0, ssn, tx],
                    [0.0, scale, 0.0, ty],
                    [-ssn, 0.0, scs, tz],
                    [0.0, 0.0, 0.0, 1.0],
                ];

                for m_skin in &c.skin_scratch {
                    let mut combined = [[0.0; 4]; 4];
                    for r in 0..4 {
                        for col in 0..4 {
                            combined[r][col] = world_mat[r][0] * m_skin[0][col]
                                + world_mat[r][1] * m_skin[1][col]
                                + world_mat[r][2] * m_skin[2][col]
                                + world_mat[r][3] * m_skin[3][col];
                        }
                    }
                    state.skin_matrices_scratch.push(combined);
                }
            }
        }

        let mut skeletal_draw = Vec::new();
        if let Some(ref c) = state.inventory_room.item_3d_asset {
            if !state.skin_matrices_scratch.is_empty() {
                skeletal_draw.push((
                    &c.gpu_mesh,
                    state.skin_matrices_scratch.as_slice(),
                    Some(c.hd_texture_bind_groups.as_slice()),
                ));
            }
        }

        state.render_ctx.render_scene(
            Some(&state.gpu_map),
            Some(&state.map_texture_bind_groups),
            &skeletal_draw,
        );
    }

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        state.begin_hud_overlay();

        let yellow = [245, 210, 65, 255];
        let cyan = [105, 215, 235, 255];
        let green = [120, 230, 150, 255];
        let white = [235, 235, 235, 255];
        let orange = [255, 165, 75, 255];
        let muted = [165, 165, 175, 255];

        state.menu_rasterizer.fill_rect(12, 10, 720, 698, [8, 10, 14, 215]);
        state
            .menu_rasterizer
            .draw_rect_outline(12, 10, 720, 698, 2, [70, 85, 110, 255]);

        state.menu_rasterizer.draw_text(
            "SILENT HILL 2 - 85-ITEM CATALOG, 3D/2D INSPECT & COMBINATIONS ROOM [SH2-15]",
            22,
            18,
            2,
            yellow,
            true,
        );

        let inv = &state.sim_core.scenario.inventory_mgr;
        state.menu_rasterizer.draw_text(
            &format!(
                "OWNED KINDS: {} | PICKED UP: {} | SECRET MASK: 0x{:02X} ({}/8) | BULLET ADJUST [B]: {}x",
                inv.owned_items().len(),
                inv.total_items_picked_up(),
                inv.secret_items_mask(),
                inv.secret_items_count(),
                inv.bullet_adjust()
            ),
            22,
            44,
            1,
            green,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "SIMULATED HP [-/=]: {:.0}/{:.0} | OVERHEAL SCORE: {:.2} | AMPOULE STAMINA: {:.1}s | MARY EXAMINES: {}/2 | FLAG 630: {}",
                state.inventory_room.simulated_hp,
                state.inventory_room.max_hp,
                inv.overheal_score(),
                inv.ampoule_stamina_timer(),
                inv.mary_examine_count(),
                state.sim_core.scenario.flags.get(630)
            ),
            22,
            62,
            1,
            cyan,
            true,
        );

        // Selected Item Detail + 2D `.tex` Preview Box
        let sel_id = state.inventory_room.selected_item_id;
        if let Some(meta) = item_metadata(sel_id) {
            let count = inv.count(sel_id);
            state.menu_rasterizer.draw_text(
                &format!(
                    "SELECTED ITEM [{:>2}/85]: #{} {}  |  Category: {:?}  |  Owned: {} / {}",
                    sel_id, meta.id, meta.name, meta.category, count, MAX_ITEM_STACK
                ),
                22,
                86,
                1,
                yellow,
                true,
            );
            state.menu_rasterizer.draw_text(
                &format!("DESCRIPTION : {}", meta.description.replace('\n', " / ")),
                22,
                104,
                1,
                white,
                true,
            );
            state.menu_rasterizer.draw_text(
                &format!(
                    "EXAMINE [E] : {}",
                    meta.examine_text
                        .unwrap_or("(No examine text)")
                        .replace('\n', " / ")
                ),
                22,
                122,
                1,
                orange,
                true,
            );
            let mdl_status = if state.inventory_room.item_3d_asset.is_some() {
                "LOADED (Spinning 3D Viewport ->)"
            } else {
                "None / 2D-Only"
            };
            state.menu_rasterizer.draw_text(
                &format!(
                    "3D MODEL    : {} ({})",
                    meta.model_path.unwrap_or("None"),
                    mdl_status
                ),
                22,
                140,
                1,
                cyan,
                true,
            );
            let tex_status = state
                .inventory_room
                .item_2d_tex
                .as_ref()
                .map(|(w, h, _, fmt)| format!("{w}x{h} {fmt}"))
                .unwrap_or_else(|| "None".to_string());
            state.menu_rasterizer.draw_text(
                &format!(
                    "2D TEXTURE  : {} ({})",
                    meta.inspect_texture_path.unwrap_or("None"),
                    tex_status
                ),
                22,
                158,
                1,
                green,
                true,
            );
        }

        // Draw 2D `.tex` inspect preview box in top-right of the HUD panel (or right side)
        let preview_x = 980usize;
        let preview_y = 28usize;
        let preview_size = 220usize;
        state.menu_rasterizer.fill_rect(
            preview_x - 6,
            preview_y - 6,
            preview_size + 12,
            preview_size + 32,
            [12, 15, 22, 230],
        );
        state.menu_rasterizer.draw_rect_outline(
            preview_x - 6,
            preview_y - 6,
            preview_size + 12,
            preview_size + 32,
            2,
            yellow,
        );
        if let Some((tw, th, ref rgba, _)) = state.inventory_room.item_2d_tex {
            state.menu_rasterizer.blit_rgba_scaled(
                preview_x,
                preview_y,
                preview_size,
                preview_size,
                rgba,
                tw as usize,
                th as usize,
                1.0,
            );
        } else {
            state.menu_rasterizer.draw_text(
                "NO 2D .TEX",
                preview_x + 54,
                preview_y + 100,
                1,
                muted,
                true,
            );
        }
        state.menu_rasterizer.draw_text(
            "2D .TEX INSPECT PREVIEW",
            preview_x + 18,
            preview_y + preview_size + 6,
            1,
            yellow,
            true,
        );

        // 13-Row Scrollable Window into the 85-Item Catalog
        state.menu_rasterizer.draw_text(
            "--- 85-ITEM CATALOG BROWSER ([Up/Down/PgUp/PgDn] Select | [Enter/G] Get | [Del/X] Drop | [F] 990 Stack) ---",
            22,
            184,
            1,
            yellow,
            true,
        );

        let window_rows = 13usize;
        let half = window_rows / 2;
        let cur_idx = (sel_id as usize).saturating_sub(1);
        let start_idx = cur_idx
            .saturating_sub(half)
            .min(CATALOG_ITEM_COUNT.saturating_sub(window_rows));

        for row in 0..window_rows {
            let idx = start_idx + row;
            if idx >= CATALOG_ITEM_COUNT {
                break;
            }
            let item = &ITEM_CATALOG[idx];
            let cnt = inv.count(item.id);
            let is_sel = item.id == sel_id;
            let y = 204 + row * 17;
            if is_sel {
                state
                    .menu_rasterizer
                    .fill_rect(18, y - 2, 704, 16, [45, 65, 95, 220]);
            }
            let col = if is_sel {
                yellow
            } else if cnt > 0 {
                green
            } else {
                white
            };
            let prefix = if is_sel { ">>" } else { "  " };
            let mdl_tag = if item.model_path.is_some() { "3D" } else { "--" };
            let tex_tag = if item.inspect_texture_path.is_some() {
                "2D"
            } else {
                "--"
            };
            state.menu_rasterizer.draw_text(
                &format!(
                    "{} #{:>2} {:<24} | {:<15} | [{}/{}] | Qty: {:>3}",
                    prefix,
                    item.id,
                    item.name,
                    format!("{:?}", item.category),
                    mdl_tag,
                    tex_tag,
                    cnt
                ),
                22,
                y,
                1,
                col,
                true,
            );
        }

        // 11-Recipe ITEM_COMBOS Workbench (86..=96)
        state.menu_rasterizer.draw_text(
            "--- 11 ITEM COMBINATION RECIPES (86..=96) ([Left/Right] Select | [C] Grant & Combine | [K] Use Result) ---",
            22,
            434,
            1,
            yellow,
            true,
        );

        for (idx, combo) in ITEM_COMBOS.iter().enumerate() {
            let y = 454 + idx * 17;
            let is_sel = idx == state.inventory_room.selected_combo_idx;
            if is_sel {
                state
                    .menu_rasterizer
                    .fill_rect(18, y - 2, 704, 16, [55, 45, 85, 220]);
            }
            let col = if is_sel { cyan } else { white };
            let prefix = if is_sel { ">>" } else { "  " };
            let p0 = item_metadata(combo.parts[0]).map(|m| m.name).unwrap_or("-");
            let p1 = item_metadata(combo.parts[1]).map(|m| m.name).unwrap_or("-");
            let p2 = if combo.parts[2] != 0 {
                item_metadata(combo.parts[2]).map(|m| m.name).unwrap_or("-")
            } else {
                "-"
            };
            state.menu_rasterizer.draw_text(
                &format!(
                    "{} Combo #{}: [{}+{}{}] -> {}",
                    prefix,
                    combo.result_id,
                    p0,
                    p1,
                    if combo.parts[2] != 0 {
                        format!("+{p2}")
                    } else {
                        String::new()
                    },
                    combo_description(combo.result_id)
                ),
                22,
                y,
                1,
                col,
                true,
            );
        }

        // Action Log & Footer Controls
        state.menu_rasterizer.draw_text(
            &format!("LAST ACTION: {}", state.inventory_room.last_action_log),
            22,
            652,
            1,
            orange,
            true,
        );
        state.menu_rasterizer.draw_text(
            "[Up/Down/PgUp/PgDn] Item 1..85 | [Enter/G] Pick Up | [Del/X] Drop | [F] Set 990 | [B] Bullet Adjust (1x..3x)",
            22,
            672,
            1,
            yellow,
            true,
        );
        state.menu_rasterizer.draw_text(
            "[U] Use Heal (#1..3) | [-/=] HP | [E] Examine | [Left/Right + C/K] Combinations | [T] Spin | [Esc] Exit",
            22,
            690,
            1,
            muted,
            true,
        );

        state.finish_hud_overlay(encoder, surface_view);
    }
}

fn combo_description(result_id: u8) -> &'static str {
    match result_id {
        86 => "Hospital Box Both Keys (Purple Bull + Lapis Eye)",
        87 => "Shower Drain Hook (Piece of Hair + Bent Needle)",
        88 => "Hospital Door Lady Rings (Copper Ring + Lead Ring)",
        89 => "2 Prison Tablets (Oppressor + Gluttonous Pig)",
        90 => "2 Prison Tablets (Oppressor + Seductress)",
        91 => "2 Prison Tablets (Gluttonous Pig + Seductress)",
        92 => "All 3 Prison Gallows Tablets (59 + 60 + 61)",
        93 => "Incomplete Trapdoor Pair (Horseshoe + Lighter, retains parts)",
        94 => "Incomplete Trapdoor Pair (Horseshoe + Wax Doll, retains parts)",
        95 => "Incomplete Trapdoor Pair (Lighter + Wax Doll, retains parts)",
        96 => "Complete Prison Trapdoor Handle (Wax Doll + Horseshoe + Lighter)",
        _ => "Combination Item",
    }
}

fn play_cursor_sound(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::CursorMove);
    }
}

fn play_confirm_sound(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
    }
}

fn play_cancel_sound(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Cancel);
    }
}
