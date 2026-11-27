//! [`RenderContext`] initialization (`new_headless` and `from_device`).

use std::sync::Arc;

use crate::{effects, fog, pipeline, post, shadow, skeletal};

use super::buffers::create_initial_buffers;
use super::initial_textures::create_initial_textures;
use super::{RenderContext, RendererConfig};

impl RenderContext {
    pub fn new_headless(config: RendererConfig) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .or_else(|| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
        })?;

        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::empty();
        if adapter_features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
        }
        if adapter_features.contains(wgpu::Features::TEXTURE_COMPRESSION_ASTC) {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_ASTC;
        }

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("SH2 Render Device"),
                required_features,
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        Some(Self::from_device(instance, adapter, device, queue, config))
    }

    pub fn from_device(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: RendererConfig,
    ) -> Self {
        let (
            (depth_texture, depth_view),
            (hdr_color_texture, hdr_color_view),
            (fog_color_texture, fog_color_view),
            (ldr_output_texture, ldr_output_view),
            (prev_frame_texture, prev_frame_view),
        ) = Self::create_offscreen_targets(&device, config.width, config.height);

        let geometry_pipeline =
            pipeline::GeometryPipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let skeletal_pipeline = skeletal::SkeletalPipeline::new(
            &device,
            Self::HDR_FORMAT,
            Self::DEPTH_FORMAT,
            &geometry_pipeline.bind_group_layout,
            &geometry_pipeline.texture_bind_group_layout,
        );
        let debug_line_pipeline = pipeline::DebugLinePipeline::new(
            &device,
            Self::HDR_FORMAT,
            Self::DEPTH_FORMAT,
            &geometry_pipeline.bind_group_layout,
        );
        let fog_pipeline = fog::FogComputePipeline::new(&device);
        let fog_particle_pipeline =
            fog::FogParticlePipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let spotlight_shadow_pipeline =
            fog::SpotlightShadowPipeline::new(&device, Self::DEPTH_FORMAT);
        let pcss_shadow_pipeline = shadow::PcssShadowPipeline::new(
            &device,
            Self::DEPTH_FORMAT,
            &skeletal_pipeline.bone_bind_group_layout,
        );
        let stencil_shadow_pipeline = shadow::StencilShadowPipeline::new(
            &device,
            Self::HDR_FORMAT,
            Self::DEPTH_FORMAT,
            &geometry_pipeline.bind_group_layout,
        );
        let shadow_blur_pipeline = shadow::ShadowBlurPipeline::new(
            &device,
            Self::HDR_FORMAT,
            shadow::StencilShadowPipeline::STENCIL_FORMAT,
        );
        shadow_blur_pipeline.update_uniforms(
            &queue,
            (shadow::PS2_DEFAULT_ROOM_SHADOW_COLOR as f32) / 128.0,
        );
        let lens_flare_pipeline = effects::LensFlarePipeline::new(&device, Self::HDR_FORMAT);
        let spray_mist_pipeline =
            effects::SprayMistPipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let water_pipeline = crate::water::LakeWaterPipeline::new(&device, Self::HDR_FORMAT);
        let mirror_pipeline = crate::mirror::PlanarMirrorPipeline::new(
            &device,
            &queue,
            Self::HDR_FORMAT,
            Self::DEPTH_FORMAT,
            &geometry_pipeline.bind_group_layout,
        );
        let dust_pipeline =
            effects::DustParticlePipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let billboard_fx_pipeline =
            effects::BillboardFxPipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let blood_pipeline =
            effects::BloodEffectPipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let post_pipeline = post::PostProcessPipeline::new(&device, &queue);

        let buffers = create_initial_buffers(&device);
        let textures = create_initial_textures(&device, &queue, config, &shadow_blur_pipeline);

        let create_geometry_bind_group = |label: &'static str, cam_buf: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &geometry_pipeline.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: cam_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffers.spotlight_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buffers.pcss_shadow_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(
                            &textures.pcss_shadow_depth_view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            &textures.specular_sphere_view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&textures.specular_sphere_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: buffers.fog_buffer.as_entire_binding(),
                    },
                ],
            })
        };
        let geometry_bind_group =
            create_geometry_bind_group("Geometry Bind Group", &buffers.camera_buffer);
        let water_geometry_bind_group = create_geometry_bind_group(
            "Water Reflection Geometry Bind Group",
            &buffers.water_camera_buffer,
        );
        let mirror_geometry_bind_group = create_geometry_bind_group(
            "Planar Mirror Geometry Bind Group",
            &buffers.mirror_camera_buffer,
        );

        let spotlight_shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Spotlight Shadow Bind Group"),
            layout: &spotlight_shadow_pipeline.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.spotlight_shadow_buffer.as_entire_binding(),
            }],
        });

        let pcss_shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PCSS Shadow Bind Group"),
            layout: &pcss_shadow_pipeline.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.pcss_shadow_buffer.as_entire_binding(),
            }],
        });

        let shadow_darken_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Darken Bind Group"),
            layout: &stencil_shadow_pipeline.darken_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.shadow_darken_buffer.as_entire_binding(),
            }],
        });

        let lens_flare_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lens Flare Texture Bind Group"),
            layout: &lens_flare_pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&textures.lens_flare_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&textures.fog_noise_sampler),
                },
            ],
        });

        let spray_mist_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hyper Spray Mist Bind Group"),
            layout: &spray_mist_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&textures.spray_mist_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&textures.fog_noise_sampler),
                },
            ],
        });

        let dust_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Airborne Dust Mote Bind Group"),
            layout: &dust_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&textures.dust_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&textures.fog_noise_sampler),
                },
            ],
        });

        let billboard_fx_bind_groups: [wgpu::BindGroup; effects::FX_TEXTURE_SLOT_COUNT] =
            std::array::from_fn(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("Billboard FX Bind Group Slot {i}")),
                    layout: &billboard_fx_pipeline.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffers.camera_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&textures.dust_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&textures.fog_noise_sampler),
                        },
                    ],
                })
            });

        let blood_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Effect2 Blood Bind Group"),
            layout: &blood_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&textures.blood_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&textures.fog_noise_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buffers.fog_buffer.as_entire_binding(),
                },
            ],
        });

        let fog_particle_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fog Particle Bind Group"),
            layout: &fog_particle_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&textures.fog_noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&textures.fog_noise_sampler),
                },
            ],
        });

        let mut skeletal_light_buffers = Vec::with_capacity(Self::MAX_SKELETAL_LIGHT_SLOTS);
        let mut skeletal_light_bind_groups = Vec::with_capacity(Self::MAX_SKELETAL_LIGHT_SLOTS);
        for slot in 0..Self::MAX_SKELETAL_LIGHT_SLOTS {
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Skeletal CharacterLightPool Slot {slot} Buffer")),
                size: std::mem::size_of::<crate::light::SpotlightUniforms>() as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!(
                    "Skeletal CharacterLightPool Slot {slot} Bind Group"
                )),
                layout: &geometry_pipeline.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffers.camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buffers.pcss_shadow_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(
                            &textures.pcss_shadow_depth_view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            &textures.specular_sphere_view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&textures.specular_sphere_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: buffers.fog_buffer.as_entire_binding(),
                    },
                ],
            });
            skeletal_light_buffers.push(buf);
            skeletal_light_bind_groups.push(bg);
        }

        let (fog_bind_group, post_bind_group_fog, post_bind_group_direct) =
            Self::create_compute_bind_groups(
                &device,
                &fog_pipeline,
                &post_pipeline,
                &buffers.fog_buffer,
                &buffers.camera_buffer,
                &buffers.spotlight_buffer,
                &buffers.post_buffer,
                &depth_view,
                &hdr_color_view,
                &fog_color_view,
                &textures.shadow_depth_view,
                &textures.fog_noise_view,
                &textures.fog_noise_sampler,
                &ldr_output_view,
                &prev_frame_view,
            );

        // Default 1x1 white texture for untextured geometry fallback
        let default_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default 1x1 White Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &default_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let default_view = default_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let create_anisotropic_sampler =
            |label: &'static str,
             address_mode_u: wgpu::AddressMode,
             address_mode_v: wgpu::AddressMode| {
                device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some(label),
                    address_mode_u,
                    address_mode_v,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    anisotropy_clamp: 16,
                    ..Default::default()
                })
            };

        let default_sampler = create_anisotropic_sampler(
            "Default Anisotropic 16x Sampler",
            wgpu::AddressMode::Repeat,
            wgpu::AddressMode::Repeat,
        );
        let clamp_sampler = create_anisotropic_sampler(
            "Clamp Anisotropic 16x Sampler",
            wgpu::AddressMode::ClampToEdge,
            wgpu::AddressMode::ClampToEdge,
        );
        let repeat_u_clamp_v_sampler = create_anisotropic_sampler(
            "Repeat-U Clamp-V Anisotropic 16x Sampler",
            wgpu::AddressMode::Repeat,
            wgpu::AddressMode::ClampToEdge,
        );
        let create_bilinear_sampler =
            |label: &'static str,
             address_mode_u: wgpu::AddressMode,
             address_mode_v: wgpu::AddressMode| {
                device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some(label),
                    address_mode_u,
                    address_mode_v,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    anisotropy_clamp: 1,
                    ..Default::default()
                })
            };

        let clamp_u_repeat_v_sampler = create_anisotropic_sampler(
            "Clamp-U Repeat-V Anisotropic 16x Sampler",
            wgpu::AddressMode::ClampToEdge,
            wgpu::AddressMode::Repeat,
        );
        let bilinear_repeat_sampler = create_bilinear_sampler(
            "Bilinear Repeat 1x Sampler",
            wgpu::AddressMode::Repeat,
            wgpu::AddressMode::Repeat,
        );
        let bilinear_clamp_sampler = create_bilinear_sampler(
            "Bilinear Clamp 1x Sampler",
            wgpu::AddressMode::ClampToEdge,
            wgpu::AddressMode::ClampToEdge,
        );
        let bilinear_repeat_u_clamp_v_sampler = create_bilinear_sampler(
            "Bilinear Repeat-U Clamp-V 1x Sampler",
            wgpu::AddressMode::Repeat,
            wgpu::AddressMode::ClampToEdge,
        );
        let bilinear_clamp_u_repeat_v_sampler = create_bilinear_sampler(
            "Bilinear Clamp-U Repeat-V 1x Sampler",
            wgpu::AddressMode::ClampToEdge,
            wgpu::AddressMode::Repeat,
        );

        let default_texture_bind_group =
            Arc::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Default Diffuse Texture Bind Group"),
                layout: &geometry_pipeline.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&default_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&default_sampler),
                    },
                ],
            }));

        Self {
            instance,
            adapter,
            device,
            queue,
            config,
            depth_format: Self::DEPTH_FORMAT,
            hdr_format: Self::HDR_FORMAT,
            output_format: Self::OUTPUT_FORMAT,
            depth_texture,
            depth_view,
            hdr_color_texture,
            hdr_color_view,
            fog_color_texture,
            fog_color_view,
            ldr_output_texture,
            ldr_output_view,
            prev_frame_texture,
            prev_frame_view,
            geometry_pipeline,
            skeletal_pipeline,
            debug_line_pipeline,
            fog_pipeline,
            fog_particle_pipeline,
            spotlight_shadow_pipeline,
            pcss_shadow_pipeline,
            stencil_shadow_pipeline,
            shadow_blur_pipeline,
            lens_flare_pipeline,
            spray_mist_pipeline,
            water_pipeline,
            mirror_pipeline,
            dust_pipeline,
            billboard_fx_pipeline,
            blood_pipeline,
            post_pipeline,
            camera_buffer: buffers.camera_buffer,
            water_camera_buffer: buffers.water_camera_buffer,
            mirror_camera_buffer: buffers.mirror_camera_buffer,
            spotlight_buffer: buffers.spotlight_buffer,
            spotlight_shadow_buffer: buffers.spotlight_shadow_buffer,
            pcss_shadow_buffer: buffers.pcss_shadow_buffer,
            shadow_darken_buffer: buffers.shadow_darken_buffer,
            shadow_volume_buffer: buffers.shadow_volume_buffer,
            drop_shadow_buffer: buffers.drop_shadow_buffer,
            lens_flare_buffer: buffers.lens_flare_buffer,
            spray_mist_buffer: buffers.spray_mist_buffer,
            dust_buffer: buffers.dust_buffer,
            billboard_fx_additive_buffer: buffers.billboard_fx_additive_buffer,
            billboard_fx_alpha_buffer: buffers.billboard_fx_alpha_buffer,
            blood_buffer: buffers.blood_buffer,
            fog_buffer: buffers.fog_buffer,
            fog_particle_buffer: buffers.fog_particle_buffer,
            post_buffer: buffers.post_buffer,
            debug_line_buffer: buffers.debug_line_buffer,
            shadow_depth_texture: textures.shadow_depth_texture,
            shadow_depth_view: textures.shadow_depth_view,
            pcss_shadow_depth_texture: textures.pcss_shadow_depth_texture,
            pcss_shadow_depth_view: textures.pcss_shadow_depth_view,
            stencil_depth_texture: textures.stencil_depth_texture,
            stencil_depth_view: textures.stencil_depth_view,
            shadow_mask_texture: textures.shadow_mask_texture,
            shadow_mask_view: textures.shadow_mask_view,
            shadow_mask_bind_group: textures.shadow_mask_bind_group,
            fog_noise_texture: textures.fog_noise_texture,
            fog_noise_view: textures.fog_noise_view,
            fog_noise_sampler: textures.fog_noise_sampler,
            lens_flare_texture: textures.lens_flare_texture,
            lens_flare_view: textures.lens_flare_view,
            spray_mist_texture: textures.spray_mist_texture,
            spray_mist_view: textures.spray_mist_view,
            dust_texture: textures.dust_texture,
            dust_view: textures.dust_view,
            blood_texture: textures.blood_texture,
            blood_view: textures.blood_view,
            specular_sphere_texture: textures.specular_sphere_texture,
            specular_sphere_view: textures.specular_sphere_view,
            specular_sphere_sampler: textures.specular_sphere_sampler,
            geometry_bind_group,
            water_geometry_bind_group,
            mirror_geometry_bind_group,
            spotlight_shadow_bind_group,
            pcss_shadow_bind_group,
            shadow_darken_bind_group,
            lens_flare_bind_group,
            spray_mist_bind_group,
            dust_bind_group,
            billboard_fx_bind_groups,
            blood_bind_group,
            fog_particle_bind_group,
            fog_bind_group,
            post_bind_group_fog,
            post_bind_group_direct,
            skeletal_light_buffers,
            skeletal_light_bind_groups,
            skeletal_light_active_slots: std::sync::atomic::AtomicUsize::new(0),
            last_spotlight_uniforms: std::sync::Mutex::new(
                crate::light::SpotlightUniforms::default(),
            ),
            gameplay_char_light_pool: std::sync::Mutex::new(None),
            filter_retain_frozen: std::sync::atomic::AtomicBool::new(false),
            fog_particle_count: std::sync::atomic::AtomicU32::new(0),
            shadow_volume_vertex_count: std::sync::atomic::AtomicU32::new(0),
            drop_shadow_vertex_count: std::sync::atomic::AtomicU32::new(0),
            lens_flare_quad_count: std::sync::atomic::AtomicU32::new(0),
            spray_mist_particle_count: std::sync::atomic::AtomicU32::new(0),
            dust_particle_count: std::sync::atomic::AtomicU32::new(0),
            billboard_fx_batches: std::sync::Mutex::new(effects::FxDrawBatches::default()),
            blood_quad_count: std::sync::atomic::AtomicU32::new(0),
            blood_subtractive_mode: std::sync::atomic::AtomicBool::new(true),
            pcss_enabled: std::sync::atomic::AtomicBool::new(false),
            lake_water_enabled: std::sync::atomic::AtomicBool::new(false),
            water_reflection_pass_enabled: std::sync::atomic::AtomicBool::new(false),
            volumetric_god_rays_enabled: std::sync::atomic::AtomicBool::new(false),
            soft_shadow_blur_enabled: std::sync::atomic::AtomicBool::new(true),
            shadow_composite_mode: std::sync::atomic::AtomicU8::new(1),
            shadow_draw_order: std::sync::atomic::AtomicU8::new(
                shadow::ShadowDrawOrder::CharacterBeforeShadow as u8,
            ),
            clear_color_rgba: std::sync::atomic::AtomicU32::new(u32::from_le_bytes(
                post::PS2_DEFAULT_NIGHT_CLEAR_RGBA,
            )),
            lens_flare_smoother: std::sync::Mutex::new(effects::LensFlareSmoother::default()),
            last_shadow_view_proj: std::sync::Mutex::new(buffers.initial_shadow_vp),
            last_pcss_view_proj: std::sync::Mutex::new(buffers.initial_pcss_vp),
            last_pcss_params: std::sync::Mutex::new(buffers.initial_pcss_params),
            last_camera_uniforms: std::sync::Mutex::new(pipeline::CameraUniforms::default()),
            planar_mirror: std::sync::Mutex::new(None),
            map_visibility_filter: std::sync::Mutex::new(None),
            mirror_glass_overlay_enabled: std::sync::atomic::AtomicBool::new(false),
            water_plane_height_mm: std::sync::atomic::AtomicU32::new(
                crate::water::DEFAULT_WATER_HEIGHT_MM.to_bits(),
            ),
            default_sampler,
            clamp_sampler,
            repeat_u_clamp_v_sampler,
            clamp_u_repeat_v_sampler,
            bilinear_repeat_sampler,
            bilinear_clamp_sampler,
            bilinear_repeat_u_clamp_v_sampler,
            bilinear_clamp_u_repeat_v_sampler,
            default_view,
            default_texture_bind_group,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn headless_render_context_defaults_to_strict_vanilla_ps2() {
        let Some(ctx) = RenderContext::new_headless(RendererConfig::default()) else {
            return;
        };
        assert!(
            !ctx.pcss_enabled.load(Ordering::Relaxed),
            "pcss_enabled must default to false for strict Vanilla PS2 parity"
        );
        assert!(
            !ctx.volumetric_god_rays_enabled.load(Ordering::Relaxed),
            "volumetric_god_rays_enabled must default to false for strict Vanilla PS2 parity"
        );
        assert!(!ctx.lake_water_enabled.load(Ordering::Relaxed));
        assert!(!ctx.water_reflection_pass_enabled.load(Ordering::Relaxed));
    }
}
