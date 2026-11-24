struct PostUniforms {
    time: f32,
    grain_intensity: f32,
    exposure: f32,
    frame_count: u32,
    dof_tap_offset_px: vec2<f32>,
    dof_strength: f32,
    motion_blur_strength: f32,
    flags: u32,
    glow_threshold: f32,
    glow_strength: f32,
    glow_step_px: f32,
    fade_rgba: vec4<f32>,
};

const POST_FLAG_ACES_TONEMAP: u32 = 1u;
const POST_FLAG_PS2_LCG_NOISE: u32 = 2u;

// PS2 `sh2gfw_SendDraw_Noise` (`0x00177f30`) & `Noise_and_FontDraw` (`0x0019ff70`) GS constants:
// - `noise_clut` (`0x002ab5c0`): 16-entry PSMT4 CLUT with RGB = `0x00` or `0x18` (`24 / 255.0`).
// - `DAT_002adb55 = 0xe6` (`230`): vertex RGB modulation (`TFX = 0 MODULATE` -> `230.0 / 128.0`).
// - `DAT_002adb52 = 0x60` (`96`): `ALPHA_1 = (0x60 << 32) | 0x21` (`A=Cd, B=Cs, C=FIX(96), D=Cs`
//   => `Out = (Cd - Cs) * (96/128) + Cs = mix(Cd, Cs, 32/128)`).
const PS2_NOISE_CLUT_HIGH: f32 = 24.0 / 255.0;
const PS2_NOISE_VTX_MODULATE: f32 = 230.0 / 128.0;
const PS2_NOISE_BLEND_WEIGHT: f32 = (128.0 - 96.0) / 128.0;
// PS2 `512x448` framebuffer displays `4.0` horizontal x `448 / 128 = 3.5` vertical tiles of `128x128`
// noise (`sh2gfw_SendDraw_Noise` `0x00177f30`). Scaling X by `aspect` gives exact `4.0 x 3.5` at
// `512x448` and keeps grain texels square on 16:9 / 21:9 / 4K displays.
const PS2_NOISE_TILES_Y: f32 = 448.0 / 128.0;

// PS2 `sh2gfw_Swap_Soft` (`0x001785e0`) & `sh2gfw_Swap_GlowSoft` (`0x00178d60`) 12.4 sub-pixel
// half-texel bias (`-4` in 12.4 fixed-point = `-4.0 / 16.0 = -0.25` px) and PS2 NTSC vertical
// framebuffer height (`448.0` px) used to scale virtual tap offsets proportionally at 1080p/1440p/4K.
const PS2_SWAP_SUBPIXEL_BIAS_PX: f32 = -0.25;
const PS2_VIRTUAL_HEIGHT_PX: f32 = 448.0;

@group(0) @binding(0) var<uniform> post: PostUniforms;
@group(0) @binding(1) var hdr_in: texture_2d<f32>;
@group(0) @binding(2) var ldr_out: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var ps2_noise_tex: texture_2d<f32>;
@group(0) @binding(4) var ps2_noise_sampler: sampler;

// Optional ACES filmic tonemapping approximation (native-upgrade, disabled in strict Vanilla PS2 mode)
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Exact IEC 61966-2-1 sRGB transfer functions so PS2 8-bit display-space GS blends
// (`sh2gfw_SendDraw_Noise` `ALPHA_1 = (0x60 << 32) | 0x21`) operate in display sRGB space
// rather than linear light space before `Bgra8UnormSrgb` swapchain presentation.
fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let clamped = max(c, vec3<f32>(0.0));
    let lo = clamped * 12.92;
    let hi = 1.055 * pow(clamped, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, clamped <= vec3<f32>(0.0031308));
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let clamped = max(c, vec3<f32>(0.0));
    let lo = clamped / 12.92;
    let hi = pow((clamped + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, clamped <= vec3<f32>(0.04045));
}

// Resolution-independent PCG hash fallback when PS2 tiled noise flag is off
fn hash_u32(x: u32) -> u32 {
    var state = x * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn random_val(pixel: vec2<u32>, frame: u32) -> f32 {
    let seed = (pixel.x * 1597334677u) ^ (pixel.y * 3812015801u) ^ (frame * 104729u);
    return f32(hash_u32(seed)) / 4294967295.0;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(hdr_in);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }
    let pixel_coords = vec2<i32>(global_id.xy);
    let max_coord = vec2<i32>(dims) - vec2<i32>(1, 1);
    let res_scale = max(f32(dims.y) / PS2_VIRTUAL_HEIGHT_PX, 1.0);
    var hdr_color = textureLoad(hdr_in, pixel_coords, 0).rgb * post.exposure;

    // 0a. PS2 `sh2gfw_Swap_Soft` (`0x001785e0`) 2x2 DoF blur:
    //     `post.dof_tap_offset_px` is already display-scaled by `SpecialFxEvaluation`
    //     (`base * (viewport_height / 1080.0)`), so sample the 2x2 box symmetrically at
    //     `[-0.5, +0.5] * dof_tap_offset_px` without double-scaling by `res_scale`, and
    //     accumulate all 4 taps into the running buffer (GS `ALPHA_1 = (FIX << 32) | 0x64`).
    if (post.dof_strength > 0.001) {
        let dof_alpha = clamp(post.dof_strength, 0.0, 1.0);
        let ox0 = -0.5 * post.dof_tap_offset_px.x + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let ox1 = 0.5 * post.dof_tap_offset_px.x + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let oy0 = -0.5 * post.dof_tap_offset_px.y + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let oy1 = 0.5 * post.dof_tap_offset_px.y + PS2_SWAP_SUBPIXEL_BIAS_PX;
        var tap_sum = vec3<f32>(0.0);
        for (var iy: i32 = 0; iy < 2; iy = iy + 1) {
            let dy = select(oy0, oy1, iy == 1);
            for (var ix: i32 = 0; ix < 2; ix = ix + 1) {
                let dx = select(ox0, ox1, ix == 1);
                let delta = vec2<i32>(round(vec2<f32>(dx, dy)));
                let tap = textureLoad(hdr_in, clamp(pixel_coords + delta, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
                tap_sum = tap_sum + tap;
            }
        }
        hdr_color = mix(hdr_color, tap_sum * 0.25, dof_alpha);
    }

    // 0b. PS2 `sh2gfw_Swap_GlowSoft` (`0x00178d60`) 2x2 additive bloom:
    //     Tap offsets (`compute_swap_tap_offsets_12_4(2, step_param)`): `[-1.5 * step_px - 0.25, +0.5 * step_px - 0.25]` px.
    //     GS `TEST_1 = (aref << 4) | 0x3800b` (`ATST = 5 GEQUAL`) and `ALPHA_1 = (FIX << 32) | 0x68`
    //     (`A=Cs, B=0, C=FIX, D=Cd` => `dst = dst + tap * (FIX / 128.0)`).
    if (post.glow_strength > 0.001) {
        let glow_alpha = clamp(post.glow_strength, 0.0, 1.0);
        let go0 = -1.5 * post.glow_step_px + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let go1 = 0.5 * post.glow_step_px + PS2_SWAP_SUBPIXEL_BIAS_PX;
        var acc = hdr_color;
        for (var iy: i32 = 0; iy < 2; iy = iy + 1) {
            let dy = select(go0, go1, iy == 1);
            for (var ix: i32 = 0; ix < 2; ix = ix + 1) {
                let dx = select(go0, go1, ix == 1);
                let delta = vec2<i32>(round(vec2<f32>(dx, dy) * res_scale));
                let tap = textureLoad(hdr_in, clamp(pixel_coords + delta, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
                let luma = max(tap.r, max(tap.g, tap.b));
                if (luma >= post.glow_threshold) {
                    acc = acc + tap * (glow_alpha * 0.25);
                }
            }
        }
        hdr_color = acc;
    }

    // 0c. PS2 `sh2gfw_Filter_Blur` (`0x00177440` / `enSetBlur` `0x00146c30` `param_2 = 0x60`):
    //     GS `ALPHA_1 = 0x44` (`A=Cs, B=Cd, C=As, D=Cd` => `dst = mix(dst, tap, As / 128.0)`)
    //     evaluated over the `Swap_Soft` 2x2 unit-step (`step_px = 1.0`) tap grid.
    if (post.motion_blur_strength > 0.001) {
        let mb_alpha = clamp(post.motion_blur_strength, 0.0, 1.0);
        let mo0 = -0.5 + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let mo1 = 0.5 + PS2_SWAP_SUBPIXEL_BIAS_PX;
        var tap_sum = vec3<f32>(0.0);
        for (var iy: i32 = 0; iy < 2; iy = iy + 1) {
            let dy = select(mo0, mo1, iy == 1);
            for (var ix: i32 = 0; ix < 2; ix = ix + 1) {
                let dx = select(mo0, mo1, ix == 1);
                let delta = vec2<i32>(round(vec2<f32>(dx, dy) * res_scale));
                let tap = textureLoad(hdr_in, clamp(pixel_coords + delta, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
                tap_sum = tap_sum + tap;
            }
        }
        hdr_color = mix(hdr_color, tap_sum * 0.25, mb_alpha);
    }

    // 1. Optional ACES tonemapping (`native-upgrade`) vs strict 1:1 Vanilla PS2 linear clamp
    var ldr: vec3<f32>;
    if ((post.flags & POST_FLAG_ACES_TONEMAP) != 0u) {
        ldr = aces_tonemap(hdr_color);
    } else {
        ldr = clamp(hdr_color, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // 2. PS2 `sh2gfw_test_MakeNoise` (`0x00177c50`) + `sh2gfw_SendDraw_Noise` (`0x00177f30`):
    //    Samples the 128x128 PSMT4 `noise_clut` (`0x002ab5c0`: `0x00` / `0x18`) texture tiled
    //    `4.0 x 3.5` on PS2 `512x448` (aspect-scaled for square grain texels at 16:9 / 4K),
    //    modulates by `DAT_002adb55 = 0xe6` (`230 / 128.0`), and blends with `ALPHA_1 = (0x60 << 32) | 0x21`
    //    (`DAT_002adb52 = 0x60 = 96` => `mix(Cd, Cs, 32 / 128.0)` scaled by user `grain_intensity`).
    if (post.grain_intensity > 0.001) {
        var noise_texel: f32;
        if ((post.flags & POST_FLAG_PS2_LCG_NOISE) != 0u) {
            let fdims = max(vec2<f32>(dims), vec2<f32>(1.0, 1.0));
            let uv = (vec2<f32>(global_id.xy) + vec2<f32>(0.5, 0.5)) / fdims;
            let tiles = vec2<f32>(PS2_NOISE_TILES_Y * (fdims.x / fdims.y), PS2_NOISE_TILES_Y);
            noise_texel = textureSampleLevel(ps2_noise_tex, ps2_noise_sampler, uv * tiles, 0.0).r;
        } else {
            let frame_seed = u32(post.time * 30.0);
            let rand_f = random_val(global_id.xy, frame_seed);
            noise_texel = select(0.0, PS2_NOISE_CLUT_HIGH, rand_f >= 0.5);
        }

        let cs_srgb = vec3<f32>(noise_texel * PS2_NOISE_VTX_MODULATE);
        let grain_weight = PS2_NOISE_BLEND_WEIGHT * clamp(post.grain_intensity, 0.0, 1.0);
        // PS2 GS blends `Cd` and `Cs` (`0x00` or `43/255`) in 8-bit display (sRGB) space.
        let ldr_srgb = linear_to_srgb(ldr);
        let noisy_srgb = clamp(mix(ldr_srgb, cs_srgb, grain_weight), vec3<f32>(0.0), vec3<f32>(1.0));
        ldr = srgb_to_linear(noisy_srgb);
    }

    // 3. PS2 Black/White/Red screen fade (`sh2gfw_Fade2` 0x00179d60, `sh2gfw_Fade3` 0x0017a2e0)
    if (post.fade_rgba.a > 0.001) {
        ldr = mix(ldr, post.fade_rgba.rgb, clamp(post.fade_rgba.a, 0.0, 1.0));
    }

    textureStore(ldr_out, pixel_coords, vec4<f32>(ldr, 1.0));
}
