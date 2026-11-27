//! CPU mipmap/BC texture preparation and GPU texture bind group creation for [`RenderContext`].

use std::sync::Arc;

use sh2_assets::formats::texture::generate_mipmap_chain_rgba;

use crate::SurfaceBlitter;

use super::RenderContext;

/// CPU-prepared mipmap pyramid (either BC1/BC3 block-compressed or raw RGBA8) ready for fast GPU upload.
pub struct PreparedGpuTexture {
    /// Base mip width in texels (`>= 1`).
    pub width: u32,
    /// Base mip height in texels (`>= 1`).
    pub height: u32,
    /// Target `wgpu::TextureFormat` (`Bc1RgbaUnorm`, `Bc3RgbaUnorm`, or `Rgba8Unorm`).
    pub format: wgpu::TextureFormat,
    /// Bytes per 4x4 BC block (`8` for BC1, `16` for BC3) or `4` for uncompressed RGBA8.
    pub block_bytes: u32,
    /// Whether `mip_levels` holds BC-compressed blocks (`true`) or raw RGBA8 pixels (`false`).
    pub is_bc: bool,
    /// When `true`, sample with 1x bilinear/trilinear filtering (`anisotropy_clamp = 1`) instead of
    /// 16x anisotropic filtering so zero-gutter cutout `.map` atlases never bleed across sub-rects.
    pub use_bilinear_1x: bool,
    /// Pre-encoded `(mip_width, mip_height, bytes)` tuples from level `0` down to the smallest valid mip.
    pub mip_levels: Vec<(u32, u32, Vec<u8>)>,
}

/// Native PS2/PC `.map` cutout atlas resolution ceiling (`512x512`).
///
/// 4K AI Master cutout atlases (`2048x2048`, e.g. `4660.dds` power lines and `4656.dds` pine
/// boughs) are exact 4x upscales of native `512x512` atlases: downsampling down to `512x512`
/// (`2048 -> 1024 -> 512`) never crosses a native zero-gutter sub-rect boundary while preventing
/// 4x minification aliasing from dropping thin 4-texel features such as `ob01` power lines.
const MAP_CUTOUT_NATIVE_MIP_FLOOR: u32 = 512;

/// Runs pure CPU preparation for a `.map` world texture with 1:1 PS2/PC mipmap parity (`SH2-161`):
///
/// - **Native cutout `.map` textures** (`<= 512x512` containing transparent texels `alpha < 4`,
///   such as packed zero-gutter foliage/sign atlases like `ob.map` `tid=4656`, `ob01.map`
///   `tid=4650`/`4660`): uploaded with **1 mip level (level 0 only)**, matching `SLUS_202.28`
///   `shGs_InitDefaultRegsEnv` (`0x001909e0`, `TEX1_1 (0x14) = 0x60` -> `MXL = 0`) and `sh2pc.exe`
///   `FUN_00457200` (`0x00457200`) / `FUN_004590c0` (`0x004590c0`) / `FUN_0045aaa0` (`0x0045aaa0`)
///   where `.map` texture header byte `+0x15` (`mip_flag == 0`) passes `Levels = 1` to `CreateTexture`.
/// - **4K AI Master cutout `.map` textures** (`> 512x512`): capped mipmap chain down to
///   [`MAP_CUTOUT_NATIVE_MIP_FLOOR`] (`512x512`, e.g. `2048 -> 1024 -> 512`) with 1x bilinear
///   sampling (`use_bilinear_1x = true`), preserving zero-gutter sub-rect isolation while preventing
///   thin features like `ob01.map` `tid=4660` power lines from aliasing out between texel rows.
/// - **Opaque `.map` textures** (no cutout alpha): full mipmap pyramid + 16x anisotropic filtering.
#[must_use]
pub fn prepare_map_texture_cpu(
    width: u32,
    height: u32,
    rgba: &[u8],
    supports_bc: bool,
) -> PreparedGpuTexture {
    prepare_map_texture_cpu_owned(width, height, rgba.to_vec(), supports_bc)
}

/// Owned-buffer counterpart of [`prepare_map_texture_cpu`] that moves `rgba` directly into
/// level-0 mipmap generation without cloning a 16 MB 4K surface (`SH2-181`).
#[must_use]
pub fn prepare_map_texture_cpu_owned(
    width: u32,
    height: u32,
    mut rgba: Vec<u8>,
    supports_bc: bool,
) -> PreparedGpuTexture {
    let w0 = width.max(1);
    let h0 = height.max(1);
    let expected0 = (w0 as usize) * (h0 as usize) * 4;
    let has_cutout = rgba.len() >= expected0
        && rgba[..expected0]
            .as_chunks::<4>()
            .0
            .iter()
            .any(|px| px[3] < 4);

    if !has_cutout {
        return prepare_mipmapped_texture_cpu_owned(width, height, rgba, supports_bc);
    }

    let raw_mips: Vec<(u32, u32, Vec<u8>)> =
        if w0 > MAP_CUTOUT_NATIVE_MIP_FLOOR || h0 > MAP_CUTOUT_NATIVE_MIP_FLOOR {
            rgba.truncate(expected0);
            generate_mipmap_chain_rgba(rgba, w0, h0)
                .into_iter()
                .take_while(|&(mw, mh, _)| {
                    mw >= MAP_CUTOUT_NATIVE_MIP_FLOOR || mh >= MAP_CUTOUT_NATIVE_MIP_FLOOR
                })
                .collect()
        } else {
            rgba.truncate(expected0);
            vec![(w0, h0, rgba)]
        };

    let can_use_bc =
        supports_bc && w0 >= 16 && h0 >= 16 && w0.is_multiple_of(4) && h0.is_multiple_of(4);
    if can_use_bc {
        let mip_levels = raw_mips
            .into_iter()
            .take_while(|&(mw, mh, _)| {
                mw >= 4 && mh >= 4 && mw.is_multiple_of(4) && mh.is_multiple_of(4)
            })
            .map(|(mw, mh, mip_rgba)| {
                let bc_bytes = sh2_assets::formats::dxt::encode_dxt_rgba(
                    &mip_rgba,
                    mw as usize,
                    mh as usize,
                    5u8,
                );
                (mw, mh, bc_bytes)
            })
            .collect();
        return PreparedGpuTexture {
            width: w0,
            height: h0,
            format: wgpu::TextureFormat::Bc3RgbaUnorm,
            block_bytes: 16,
            is_bc: true,
            use_bilinear_1x: true,
            mip_levels,
        };
    }

    PreparedGpuTexture {
        width: w0,
        height: h0,
        format: wgpu::TextureFormat::Rgba8Unorm,
        block_bytes: 4,
        is_bc: false,
        use_bilinear_1x: true,
        mip_levels: raw_mips,
    }
}

/// Runs pure CPU mipmap pyramid generation and optional BC1/BC3 (`DXT1`/`DXT5`) block compression
/// so callers can parallelize texture preparation across CPU cores before uploading to `wgpu`.
#[must_use]
pub fn prepare_mipmapped_texture_cpu(
    width: u32,
    height: u32,
    rgba: &[u8],
    supports_bc: bool,
) -> PreparedGpuTexture {
    prepare_mipmapped_texture_cpu_owned(width, height, rgba.to_vec(), supports_bc)
}

/// Owned-buffer counterpart of [`prepare_mipmapped_texture_cpu`] that avoids cloning `rgba` into
/// mip level 0 (`SH2-181`).
#[must_use]
pub fn prepare_mipmapped_texture_cpu_owned(
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    supports_bc: bool,
) -> PreparedGpuTexture {
    let w0 = width.max(1);
    let h0 = height.max(1);
    let has_alpha = rgba.as_chunks::<4>().0.iter().any(|px| px[3] < 255);
    let mips = generate_mipmap_chain_rgba(rgba, w0, h0);

    let can_use_bc =
        supports_bc && w0 >= 16 && h0 >= 16 && w0.is_multiple_of(4) && h0.is_multiple_of(4);
    if can_use_bc {
        let valid_bc_mips: Vec<&(u32, u32, Vec<u8>)> = mips
            .iter()
            .take_while(|(mw, mh, _)| {
                *mw >= 4 && *mh >= 4 && mw.is_multiple_of(4) && mh.is_multiple_of(4)
            })
            .collect();

        if !valid_bc_mips.is_empty() {
            let (format, dxt_kind, block_bytes) = if has_alpha {
                (wgpu::TextureFormat::Bc3RgbaUnorm, 5u8, 16u32)
            } else {
                (wgpu::TextureFormat::Bc1RgbaUnorm, 1u8, 8u32)
            };
            let mip_levels = valid_bc_mips
                .into_iter()
                .map(|(mip_w, mip_h, mip_rgba)| {
                    let bc_bytes = sh2_assets::formats::dxt::encode_dxt_rgba(
                        mip_rgba,
                        *mip_w as usize,
                        *mip_h as usize,
                        dxt_kind,
                    );
                    (*mip_w, *mip_h, bc_bytes)
                })
                .collect();
            return PreparedGpuTexture {
                width: w0,
                height: h0,
                format,
                block_bytes,
                is_bc: true,
                use_bilinear_1x: false,
                mip_levels,
            };
        }
    }

    PreparedGpuTexture {
        width: w0,
        height: h0,
        format: wgpu::TextureFormat::Rgba8Unorm,
        block_bytes: 4,
        is_bc: false,
        use_bilinear_1x: false,
        mip_levels: mips,
    }
}

/// Returns whether the active `wgpu::Device` supports hardware BC1/BC3 texture compression.
impl RenderContext {
    /// Returns the 16x anisotropic sampler matching the requested `(wrap_u, wrap_v)` UV addressing.
    #[must_use]
    pub fn sampler_for_wrap(&self, wrap_u: bool, wrap_v: bool) -> &wgpu::Sampler {
        match (wrap_u, wrap_v) {
            (true, true) => &self.default_sampler,
            (false, false) => &self.clamp_sampler,
            (true, false) => &self.repeat_u_clamp_v_sampler,
            (false, true) => &self.clamp_u_repeat_v_sampler,
        }
    }

    /// Returns the 1x bilinear sampler (`anisotropy_clamp = 1`) matching the requested
    /// `(wrap_u, wrap_v)` UV addressing for single-level cutout `.map` atlases (`SH2-161`).
    #[must_use]
    pub fn bilinear_sampler_for_wrap(&self, wrap_u: bool, wrap_v: bool) -> &wgpu::Sampler {
        match (wrap_u, wrap_v) {
            (true, true) => &self.bilinear_repeat_sampler,
            (false, false) => &self.bilinear_clamp_sampler,
            (true, false) => &self.bilinear_repeat_u_clamp_v_sampler,
            (false, true) => &self.bilinear_clamp_u_repeat_v_sampler,
        }
    }

    /// Creates a GPU diffuse texture bind group with bilinear filtering using a specific texture view
    /// and explicit sampler.
    pub fn create_texture_bind_group_from_view_with_sampler(
        &self,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        Arc::new(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: &self.geometry_pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        }))
    }

    /// Creates a GPU diffuse texture bind group with bilinear filtering using a specific texture view.
    pub fn create_texture_bind_group_from_view(
        &self,
        view: &wgpu::TextureView,
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        self.create_texture_bind_group_from_view_with_sampler(view, &self.default_sampler, label)
    }

    /// Creates a GPU diffuse texture bind group with full mipmap chain and 16x anisotropic filtering from raw RGBA8 pixels.
    pub fn create_texture_bind_group(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        self.create_mipmapped_texture_bind_group(width, height, rgba, label)
    }

    /// Returns `true` when hardware BC1/BC3 (`DXT1`/`DXT5`) block compression is enabled on the GPU device.
    #[must_use]
    pub fn supports_bc_compression(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    }

    /// Uploads a CPU-prepared [`PreparedGpuTexture`] directly to VRAM and returns its bind group
    /// using [`Self::default_sampler`] (`AddressMode::Repeat`).
    #[must_use]
    pub fn upload_prepared_texture_bind_group(
        &self,
        prepared: &PreparedGpuTexture,
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        self.upload_prepared_texture_bind_group_with_wrap(prepared, true, true, label)
    }

    /// Uploads a CPU-prepared [`PreparedGpuTexture`] directly to VRAM and returns its bind group
    /// with per-axis UV addressing (`wrap_u`, `wrap_v`).
    #[must_use]
    pub fn upload_prepared_texture_bind_group_with_wrap(
        &self,
        prepared: &PreparedGpuTexture,
        wrap_u: bool,
        wrap_v: bool,
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: prepared.width,
                height: prepared.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: (prepared.mip_levels.len() as u32).max(1),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: prepared.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (level, (mip_w, mip_h, bytes)) in prepared.mip_levels.iter().enumerate() {
            let (bytes_per_row, rows_per_image) = if prepared.is_bc {
                ((mip_w / 4) * prepared.block_bytes, mip_h / 4)
            } else {
                (4 * (*mip_w), *mip_h)
            };
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(rows_per_image),
                },
                wgpu::Extent3d {
                    width: *mip_w,
                    height: *mip_h,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = if prepared.use_bilinear_1x || prepared.mip_levels.len() <= 1 {
            self.bilinear_sampler_for_wrap(wrap_u, wrap_v)
        } else {
            self.sampler_for_wrap(wrap_u, wrap_v)
        };
        self.create_texture_bind_group_from_view_with_sampler(&view, sampler, label)
    }

    /// Creates a GPU diffuse texture bind group generating a full alpha-coverage-preserving mipmap chain
    /// and 16x anisotropic sampler. Uses hardware BC1/BC3 (`DXT1`/`DXT5`) block compression when
    /// `wgpu::Features::TEXTURE_COMPRESSION_BC` is enabled and the dimensions are multiples of 4
    /// (`>= 16x16`), reducing GPU VRAM usage by 4x–8x (`SH2-92`).
    pub fn create_mipmapped_texture_bind_group(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        let prepared =
            prepare_mipmapped_texture_cpu(width, height, rgba, self.supports_bc_compression());
        self.upload_prepared_texture_bind_group(&prepared, label)
    }

    /// Creates a 2D UI presentation texture bind group from raw RGBA8 pixels using the SurfaceBlitter bind group layout.
    pub fn create_blitter_bind_group(
        &self,
        blitter: &SurfaceBlitter,
        width: u32,
        height: u32,
        rgba: &[u8],
        uniform_buffer: &wgpu::Buffer,
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // 2D splash/UI textures are sRGB-encoded; uploading as Rgba8UnormSrgb prevents
            // double-gamma encoding when blitted onto the sRGB swapchain (`SurfaceBlitter`).
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Arc::new(blitter.create_bind_group_with_uniform_buffer(&self.device, &view, uniform_buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_map_texture_cpu_keeps_native_cutout_at_1_mip_and_caps_4k_at_512() {
        // Native 512x512 cutout atlas: strict 1-mip PS2/PC parity (`MXL = 0`, `Levels = 1`).
        let mut native = vec![255u8; 512 * 512 * 4];
        native[3] = 0; // transparent cutout texel
        let prep_native = prepare_map_texture_cpu(512, 512, &native, false);
        assert!(prep_native.use_bilinear_1x);
        assert_eq!(prep_native.mip_levels.len(), 1);
        assert_eq!(
            (prep_native.mip_levels[0].0, prep_native.mip_levels[0].1),
            (512, 512)
        );

        // 4K 2048x2048 cutout atlas (e.g. `4660.dds` power lines / `4656.dds` pine boughs):
        // capped at `512x512` (2048 -> 1024 -> 512 = 3 levels) with 1x bilinear filtering so
        // thin wires never alias out while zero-gutter 512x512 sub-rect boundaries stay isolated.
        let mut atlas_4k = vec![0u8; 2048 * 2048 * 4];
        for y in 2040..2044 {
            for x in 0..2048 {
                atlas_4k[(y * 2048 + x) * 4 + 3] = 255;
            }
        }
        let prep_4k = prepare_map_texture_cpu(2048, 2048, &atlas_4k, false);
        assert!(prep_4k.use_bilinear_1x);
        assert_eq!(prep_4k.mip_levels.len(), 3);
        assert_eq!(
            (prep_4k.mip_levels[0].0, prep_4k.mip_levels[0].1),
            (2048, 2048)
        );
        assert_eq!(
            (prep_4k.mip_levels[1].0, prep_4k.mip_levels[1].1),
            (1024, 1024)
        );
        assert_eq!(
            (prep_4k.mip_levels[2].0, prep_4k.mip_levels[2].1),
            (512, 512)
        );
        let mip512 = &prep_4k.mip_levels[2].2;
        assert!(
            mip512[(510 * 512 + 256) * 4 + 3] > 200,
            "4-texel wire must survive into 512x512 mip level"
        );
    }
}
