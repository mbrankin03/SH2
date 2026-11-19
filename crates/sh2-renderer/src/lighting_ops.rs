//! Shadow configuration, `.kg1`/`.kg2` shadow volume upload, lens flare quad upload, and
//! Hyper Spray mist upload helpers for [`RenderContext`] (`SH2-81`).

use crate::{effects, shadow, RenderContext};

impl RenderContext {
    /// Configures Native PCSS vs Classic PS2 / SH2:EE Stencil shadow mode, shadow opacity,
    /// and SH2:EE `SpecularFix` scaling (`SH2-81`).
    pub fn update_shadow_config(
        &self,
        shadow_mode: sh2_core::config::ShadowQualityMode,
        opacity: f32,
        specular_scale: f32,
    ) {
        let pcss_on = matches!(shadow_mode, sh2_core::config::ShadowQualityMode::NativePcss);
        self.pcss_enabled
            .store(pcss_on, std::sync::atomic::Ordering::Relaxed);
        let params = [
            if pcss_on { 1.0 } else { 0.0 },
            opacity.clamp(0.0, 1.0),
            0.024,
            specular_scale.clamp(0.1, 2.0),
        ];
        if let Ok(mut guard) = self.last_pcss_params.lock() {
            *guard = params;
        }
        let vp = self
            .last_shadow_view_proj
            .lock()
            .map(|g| *g)
            .unwrap_or_else(|_| glam::Mat4::IDENTITY.to_cols_array_2d());
        let pcss_u = shadow::PcssShadowUniforms {
            light_view_proj: vp,
            params,
        };
        self.queue
            .write_buffer(&self.pcss_shadow_buffer, 0, bytemuck::bytes_of(&pcss_u));
        let darken_u = shadow::ShadowDarkenUniforms {
            color_opacity: [0.0, 0.0, 0.0, opacity.clamp(0.0, 1.0)],
        };
        self.queue
            .write_buffer(&self.shadow_darken_buffer, 0, bytemuck::bytes_of(&darken_u));
    }

    /// Uploads extruded `.kg1`/`.kg2` stencil shadow volume vertices and/or flattened floor
    /// drop-shadow vertices (`0x445540` / `0x5a7c90`) for the next frame (`SH2-81`).
    pub fn update_shadow_volumes(
        &self,
        stencil_verts: &[shadow::ShadowVolumeVertex],
        drop_verts: &[shadow::ShadowVolumeVertex],
        opacity: f32,
    ) {
        let sv_count = stencil_verts.len().min(shadow::MAX_SHADOW_VOLUME_VERTICES);
        if sv_count > 0 {
            self.queue.write_buffer(
                &self.shadow_volume_buffer,
                0,
                bytemuck::cast_slice(&stencil_verts[..sv_count]),
            );
        }
        self.shadow_volume_vertex_count
            .store(sv_count as u32, std::sync::atomic::Ordering::Relaxed);

        let dv_count = drop_verts.len().min(shadow::MAX_SHADOW_VOLUME_VERTICES);
        if dv_count > 0 {
            self.queue.write_buffer(
                &self.drop_shadow_buffer,
                0,
                bytemuck::cast_slice(&drop_verts[..dv_count]),
            );
        }
        self.drop_shadow_vertex_count
            .store(dv_count as u32, std::sync::atomic::Ordering::Relaxed);

        let darken_u = shadow::ShadowDarkenUniforms {
            color_opacity: [0.0, 0.0, 0.0, opacity.clamp(0.0, 1.0)],
        };
        self.queue
            .write_buffer(&self.shadow_darken_buffer, 0, bytemuck::bytes_of(&darken_u));
    }

    /// Uploads evaluated 7-sprite lens flare quads (`Lens/lens_flare.c`) for the next frame (`SH2-81`).
    pub fn update_lens_flares(&self, quads: &[effects::GpuLensFlareQuad]) {
        let count = quads.len().min(effects::MAX_GPU_LENS_FLARE_QUADS);
        if count > 0 {
            self.queue.write_buffer(
                &self.lens_flare_buffer,
                0,
                bytemuck::cast_slice(&quads[..count]),
            );
        }
        self.lens_flare_quad_count
            .store(count as u32, std::sync::atomic::Ordering::Relaxed);
    }

    /// Uploads Hyper Spray / Lying Figure mist billboard instances (`enEfctDrawSpray`) for the next frame (`SH2-81`).
    pub fn update_spray_particles(&self, particles: &[effects::GpuSprayParticle]) {
        let count = particles.len().min(effects::MAX_GPU_SPRAY_PARTICLES);
        if count > 0 {
            self.queue.write_buffer(
                &self.spray_mist_buffer,
                0,
                bytemuck::cast_slice(&particles[..count]),
            );
        }
        self.spray_mist_particle_count
            .store(count as u32, std::sync::atomic::Ordering::Relaxed);
    }

    /// Replaces the lens flare atlas texture with decoded RGBA8 data from retail `lens_flare.tbn2`.
    pub fn upload_lens_flare_texture(&mut self, rgba: &[u8], width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if rgba.len() < (w * h * 4) as usize {
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Retail Lens Flare Atlas (lens_flare.tbn2)"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba[..(w * h * 4) as usize],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.lens_flare_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.lens_flare_texture = tex;
        self.lens_flare_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lens Flare Texture Bind Group"),
            layout: &self.lens_flare_pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.lens_flare_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.fog_noise_sampler),
                },
            ],
        });
    }

    /// Replaces the Hyper Spray mist texture with decoded RGBA8 data from retail `poison.tbn2` / `fog.tex`.
    pub fn upload_spray_mist_texture(&mut self, rgba: &[u8], width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if rgba.len() < (w * h * 4) as usize {
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Retail Hyper Spray Mist Texture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba[..(w * h * 4) as usize],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.spray_mist_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.spray_mist_texture = tex;
        self.spray_mist_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hyper Spray Mist Bind Group"),
            layout: &self.spray_mist_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.spray_mist_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.fog_noise_sampler),
                },
            ],
        });
    }
}
