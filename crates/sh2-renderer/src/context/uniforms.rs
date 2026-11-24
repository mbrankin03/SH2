//! Runtime uniform updates, render-state setters, fog noise upload, and skeletal mesh compilation.

use sh2_assets::formats::mdls::ModelFile;

use crate::{effects, fog, light, pipeline, post, shadow, GpuSkeletalMesh};

use super::RenderContext;

impl RenderContext {
    pub fn update_camera(&self, view_proj: glam::Mat4, inv_proj: glam::Mat4, cam_pos: glam::Vec3) {
        self.update_camera_with_mode(view_proj, inv_proj, cam_pos, 0.0);
    }

    /// Sets the dynamic PS2 room clear color (`SLUS_202.28` `kari_sh2setGS_clearcolor` `0x00175930`).
    pub fn set_room_clear_color(&self, is_night: bool, room_clear_rgba: Option<[u8; 4]>) {
        let rgba = post::resolve_ps2_room_clear_rgba(is_night, room_clear_rgba);
        self.clear_color_rgba.store(
            u32::from_le_bytes(rgba),
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    /// Returns the active `wgpu::Color` for the main scene render pass (`kari_sh2setGS_clearcolor`).
    #[must_use]
    pub fn room_clear_color(&self) -> wgpu::Color {
        let rgba = self
            .clear_color_rgba
            .load(std::sync::atomic::Ordering::Relaxed)
            .to_le_bytes();
        post::ps2_clear_rgba_to_wgpu(rgba)
    }

    /// Enables or disables the 4-pass PS2 ping-pong soft stencil shadow blur (`0x001767e0`).
    pub fn set_soft_shadow_blur_enabled(&self, enabled: bool) {
        self.soft_shadow_blur_enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    /// Sets the PS2 GS stencil shadow composite blend mode (`0x00175f90`: `0x8000000052` vs `0x8000000046`).
    pub fn set_shadow_composite_mode(&self, mode: shadow::ShadowCompositeMode) {
        let val = match mode {
            shadow::ShadowCompositeMode::Subtractive => 0u8,
            shadow::ShadowCompositeMode::Multiplicative => 1u8,
        };
        self.shadow_composite_mode
            .store(val, std::sync::atomic::Ordering::Relaxed);
    }

    /// Returns the active PS2 GS stencil shadow composite blend mode (`0x00175f90`).
    #[must_use]
    pub fn shadow_composite_mode(&self) -> shadow::ShadowCompositeMode {
        let val = self
            .shadow_composite_mode
            .load(std::sync::atomic::Ordering::Relaxed);
        shadow::ShadowCompositeMode::from_filter_param(val as i32)
    }

    /// Updates the room shadow blur composite opacity (`Get_ShadowColor` `0x00176040`).
    pub fn set_shadow_blur_opacity(&self, opacity: f32) {
        self.shadow_blur_pipeline
            .update_uniforms(&self.queue, opacity);
    }

    /// Sets the PS2 character vs. shadow draw order (`Check_DrawOrder` `0x001a08a0`).
    pub fn set_shadow_draw_order(&self, order: shadow::ShadowDrawOrder) {
        self.shadow_draw_order
            .store(order as u8, std::sync::atomic::Ordering::Relaxed);
    }

    /// Returns the active PS2 character vs. shadow draw order (`Check_DrawOrder` `0x001a08a0`).
    #[must_use]
    pub fn shadow_draw_order(&self) -> shadow::ShadowDrawOrder {
        if self
            .shadow_draw_order
            .load(std::sync::atomic::Ordering::Relaxed)
            == shadow::ShadowDrawOrder::ShadowBeforeCharacter as u8
        {
            shadow::ShadowDrawOrder::ShadowBeforeCharacter
        } else {
            shadow::ShadowDrawOrder::CharacterBeforeShadow
        }
    }

    /// Steps the stateful PS2 [`effects::LensFlareSmoother`] (`kari_shLensFlareDraw` `0x00184ef0`,
    /// `shLensFlareLightCenterIsVisible` `0x00187090`, `shLensFlareDrawCommon` `0x00184bf0`),
    /// applies `(rate + 3.0) / 4.0` size and `512.0 / 448.0` Y-aspect scaling to `quads`, and
    /// uploads the smoothed quads to the GPU lens-flare buffer.
    pub fn update_lens_flares_smoothed(
        &self,
        quads: &mut [effects::GpuLensFlareQuad],
        center_visible: bool,
        target_rate: f32,
        camera_is_smooth: bool,
    ) -> f32 {
        let rate = self
            .lens_flare_smoother
            .lock()
            .map(|mut s| s.step(center_visible, target_rate, camera_is_smooth))
            .unwrap_or(0.0);
        effects::apply_ps2_lens_flare_smoothing(quads, rate);
        self.update_lens_flares(quads);
        rate
    }

    /// Framerate-independent variant of [`Self::update_lens_flares_smoothed`] for 60/120/240/unlimited Hz
    /// rendering (`LensFlareSmoother::step_dt`).
    pub fn update_lens_flares_smoothed_dt(
        &self,
        quads: &mut [effects::GpuLensFlareQuad],
        center_visible: bool,
        target_rate: f32,
        camera_is_smooth: bool,
        dt_secs: f32,
    ) -> f32 {
        let rate = self
            .lens_flare_smoother
            .lock()
            .map(|mut s| s.step_dt(center_visible, target_rate, camera_is_smooth, dt_secs))
            .unwrap_or(0.0);
        effects::apply_ps2_lens_flare_smoothing(quads, rate);
        self.update_lens_flares(quads);
        rate
    }

    /// Builds a right-handed `Depth32Float` perspective `(proj, inv_proj)` pair honoring a custom
    /// `near_clip` in SH2 world units (e.g. `8.0` in First-Person vs `25.0` in Third-Person/Rail) (`SH2-93`).
    #[inline]
    pub fn perspective_projection(
        &self,
        fov_y_radians: f32,
        near_clip: f32,
        far_clip: f32,
    ) -> (glam::Mat4, glam::Mat4) {
        let aspect = (self.config.width.max(1) as f32) / (self.config.height.max(1) as f32);
        let z_near = near_clip.max(1.0);
        let z_far = far_clip.max(z_near + 100.0);
        let proj = glam::Mat4::perspective_rh(fov_y_radians, aspect, z_near, z_far);
        (proj, proj.inverse())
    }

    pub fn update_camera_with_mode(
        &self,
        view_proj: glam::Mat4,
        inv_proj: glam::Mat4,
        cam_pos: glam::Vec3,
        shade_mode: f32,
    ) {
        let inv_vp = if view_proj.determinant().abs() > 1e-12 {
            view_proj.inverse()
        } else {
            glam::Mat4::IDENTITY
        };
        let uniforms = pipeline::CameraUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            inv_proj: inv_proj.to_cols_array_2d(),
            cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, shade_mode],
            inv_view_proj: inv_vp.to_cols_array_2d(),
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn update_fog(&self, uniforms: &fog::FogUniforms) {
        let mut effective = *uniforms;
        if let Ok(guard) = self.last_shadow_view_proj.lock() {
            effective.shadow_view_proj = *guard;
        }
        self.queue
            .write_buffer(&self.fog_buffer, 0, bytemuck::bytes_of(&effective));
    }

    pub fn update_spotlight(&self, uniforms: &light::SpotlightUniforms) {
        self.queue
            .write_buffer(&self.spotlight_buffer, 0, bytemuck::bytes_of(uniforms));
        let shadow_vp = fog::compute_spotlight_view_proj(uniforms).to_cols_array_2d();
        let shadow_u = fog::SpotlightShadowUniforms {
            light_view_proj: shadow_vp,
        };
        self.queue.write_buffer(
            &self.spotlight_shadow_buffer,
            0,
            bytemuck::bytes_of(&shadow_u),
        );
        if let Ok(mut guard) = self.last_shadow_view_proj.lock() {
            *guard = shadow_vp;
        }
        let pcss_vp = shadow::compute_pcss_spotlight_view_proj(uniforms).to_cols_array_2d();
        let pcss_params =
            self.last_pcss_params
                .lock()
                .map(|g| *g)
                .unwrap_or([1.0, 170.0 / 255.0, 0.024, 0.5]);
        let pcss_u = shadow::PcssShadowUniforms {
            light_view_proj: pcss_vp,
            params: pcss_params,
        };
        self.queue
            .write_buffer(&self.pcss_shadow_buffer, 0, bytemuck::bytes_of(&pcss_u));
        let vp_offset = std::mem::offset_of!(fog::FogUniforms, shadow_view_proj) as u64;
        self.queue
            .write_buffer(&self.fog_buffer, vp_offset, bytemuck::bytes_of(&shadow_vp));
    }

    /// Uploads localized 3D proximity fog sprite instances (`fogMakePacket` @ `0x0016bb70`)
    /// for rendering in the next scene pass (`SH2-32`).
    pub fn update_fog_particles(&self, particles: &[fog::GpuFogParticle]) {
        let count = particles.len().min(fog::MAX_GPU_FOG_PARTICLES);
        if count > 0 {
            self.queue.write_buffer(
                &self.fog_particle_buffer,
                0,
                bytemuck::cast_slice(&particles[..count]),
            );
        }
        self.fog_particle_count
            .store(count as u32, std::sync::atomic::Ordering::Relaxed);
    }

    /// Replaces the tileable fog noise texture with decoded RGBA8 data (e.g. retail `data/pic/effect/fog.tex`).
    pub fn upload_fog_noise_texture(&mut self, rgba: &[u8], width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if rgba.len() < (w * h * 4) as usize {
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Retail Fog Noise Texture (fog.tex)"),
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
        self.fog_noise_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.fog_noise_texture = tex;

        self.fog_particle_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fog Particle Bind Group"),
            layout: &self.fog_particle_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.fog_noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.fog_noise_sampler),
                },
            ],
        });

        let (fog_bind_group, post_bind_group_fog, post_bind_group_direct) =
            Self::create_compute_bind_groups(
                &self.device,
                &self.fog_pipeline,
                &self.post_pipeline,
                &self.fog_buffer,
                &self.camera_buffer,
                &self.spotlight_buffer,
                &self.post_buffer,
                &self.depth_view,
                &self.hdr_color_view,
                &self.fog_color_view,
                &self.shadow_depth_view,
                &self.fog_noise_view,
                &self.fog_noise_sampler,
                &self.ldr_output_view,
            );
        self.fog_bind_group = fog_bind_group;
        self.post_bind_group_fog = post_bind_group_fog;
        self.post_bind_group_direct = post_bind_group_direct;
    }

    /// Update post-processing uniforms buffer and, when PS2 LCG film grain is active,
    /// step and upload the bit-exact `128x128` `R8Unorm` PS2 noise texture (`0x00177c50`)
    /// on the authentic 30 Hz PS2 cadence (framerate-independent at 120/240/unlimited Hz).
    pub fn update_post(&self, uniforms: &post::PostUniforms) {
        self.queue
            .write_buffer(&self.post_buffer, 0, bytemuck::bytes_of(uniforms));
        if (uniforms.flags & post::POST_FLAG_PS2_LCG_NOISE) != 0 && uniforms.grain_intensity > 0.001
        {
            self.post_pipeline
                .step_ps2_noise_at_time(&self.queue, uniforms.time);
        }
    }

    /// Compiles a ModelFile into a GpuSkeletalMesh.
    pub fn compile_skeletal_mesh(&self, model: &ModelFile) -> GpuSkeletalMesh {
        GpuSkeletalMesh::from_model(
            &self.device,
            &self.skeletal_pipeline.bone_bind_group_layout,
            &self.skeletal_pipeline.material_bind_group_layout,
            model,
        )
    }
}
