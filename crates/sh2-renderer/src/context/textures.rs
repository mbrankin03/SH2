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
    /// Pre-encoded `(mip_width, mip_height, bytes)` tuples from level `0` down to the smallest valid mip.
    pub mip_levels: Vec<(u32, u32, Vec<u8>)>,
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
    let w0 = width.max(1);
    let h0 = height.max(1);
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
            let has_alpha = rgba.as_chunks::<4>().0.iter().any(|px| px[3] < 255);
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
        mip_levels: mips,
    }
}

/// Returns whether the active `wgpu::Device` supports hardware BC1/BC3 texture compression.
impl RenderContext {
    /// Creates a GPU diffuse texture bind group with bilinear filtering using a specific texture view.
    pub fn create_texture_bind_group_from_view(
        &self,
        view: &wgpu::TextureView,
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
                    resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                },
            ],
        }))
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

    /// Uploads a CPU-prepared [`PreparedGpuTexture`] directly to VRAM and returns its bind group.
    #[must_use]
    pub fn upload_prepared_texture_bind_group(
        &self,
        prepared: &PreparedGpuTexture,
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
        self.create_texture_bind_group_from_view(&view, label)
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
            format: wgpu::TextureFormat::Rgba8Unorm,
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
