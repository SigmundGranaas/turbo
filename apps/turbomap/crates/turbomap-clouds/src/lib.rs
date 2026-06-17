//! Procedural cloud overlay for the turbomap wgpu engine.
//!
//! Turns a low-resolution "blocky" radar grid (precipitation + cloud
//! coverage — see [`data`]) into a soft, volumetric-looking cloud layer
//! rendered entirely on the GPU in a single fragment pass. Darker cloud =
//! heavier rain. Two timesteps are crossfaded so a time slider can scrub
//! smoothly forward and backward while procedural detail keeps drifting.
//!
//! The public surface is [`CloudScene`]: create it once, [`upload`] radar
//! frames into its two slots, then [`render`] with the desired time /
//! crossfade. It is deliberately self-contained (its own fullscreen pass,
//! no depth, no MSAA) so it can be dropped onto any wgpu target — the
//! offscreen golden harness in the demo, or the live map's surface.
//!
//! [`upload`]: CloudScene::upload
//! [`render`]: CloudScene::render

use bytemuck::{Pod, Zeroable};

pub mod data;
pub mod metrics;

pub use data::{Cell, RadarFrame, SyntheticStorm};

/// Which quantity the shader writes out, for diagnostics. [`DebugView::Final`]
/// is the real overlay; every other variant isolates one internal stage of
/// the pipeline as an opaque greyscale (or colour) image, so the look can be
/// decomposed and measured. Mirrors the `debug_view` switch in `clouds.wgsl`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DebugView {
    /// The production premultiplied-alpha composite.
    Final,
    /// De-blocked radar precipitation channel (drives darkness).
    RadarPrecip,
    /// De-blocked radar coverage channel (drives where cloud exists).
    RadarCoverage,
    /// Raw domain-warped fractal cloud field, before thresholding.
    CloudField,
    /// Density after the coverage-driven threshold (the silhouette).
    Density,
    /// Relief lighting term (sun slope + self-shadow + rim), normalised.
    Light,
    /// Final composited opacity.
    Alpha,
    /// Unlit rain-coloured albedo (precip → darkness), before lighting.
    Albedo,
}

impl DebugView {
    /// All views in pipeline order — handy for a decomposition montage.
    pub const ALL: [DebugView; 8] = [
        DebugView::Final,
        DebugView::RadarPrecip,
        DebugView::RadarCoverage,
        DebugView::CloudField,
        DebugView::Density,
        DebugView::Light,
        DebugView::Alpha,
        DebugView::Albedo,
    ];

    /// The `debug_view` code passed to the shader.
    pub fn code(self) -> u32 {
        match self {
            DebugView::Final => 0,
            DebugView::RadarPrecip => 1,
            DebugView::RadarCoverage => 2,
            DebugView::CloudField => 3,
            DebugView::Density => 4,
            DebugView::Light => 5,
            DebugView::Alpha => 6,
            DebugView::Albedo => 7,
        }
    }

    /// Short human label for montage captions / logs.
    pub fn label(self) -> &'static str {
        match self {
            DebugView::Final => "final",
            DebugView::RadarPrecip => "radar precip",
            DebugView::RadarCoverage => "radar coverage",
            DebugView::CloudField => "cloud field",
            DebugView::Density => "density",
            DebugView::Light => "light",
            DebugView::Alpha => "alpha",
            DebugView::Albedo => "albedo",
        }
    }
}

/// GPU-side per-frame parameters, mirrored exactly by the `Uniforms`
/// block in `clouds.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    blend: f32,
    wind: [f32; 2],
    sun_dir: [f32; 2],
    map_scale: f32,
    erosion: f32,
    softness: f32,
    intensity: f32,
    parallax: f32,
    sun_elevation: f32,
    extinction: f32,
    light_extinction: f32,
    debug_view: u32,
    _pad: [u32; 3],
}

/// Tunable look of the cloud overlay for one rendered frame.
#[derive(Copy, Clone, Debug)]
pub struct CloudParams {
    /// Target size in pixels.
    pub resolution: [f32; 2],
    /// Animation clock in seconds — drives cloud drift and "boil".
    pub time: f32,
    /// Crossfade between the two uploaded radar frames, `0` = slot A,
    /// `1` = slot B.
    pub blend: f32,
    /// Wind vector (screen-space-ish); sets drift direction/speed.
    pub wind: [f32; 2],
    /// 2D sun azimuth used for fake self-shadowing and lighting.
    pub sun_dir: [f32; 2],
    /// Cloud feature frequency relative to the screen. Higher = smaller,
    /// more numerous puffs.
    pub map_scale: f32,
    /// Strength of high-frequency edge erosion (wispy cloud edges).
    pub erosion: f32,
    /// Width of the soft alpha edge.
    pub softness: f32,
    /// Overall opacity of the overlay.
    pub intensity: f32,
    /// View parallax through the cloud layer, from the map camera's pitch.
    /// `0` = straight-down (the cloud field renders flat, top-down); higher
    /// = the view ray rakes through the slab and reveals the puff *sides*
    /// (real 3D when the map is tilted). Roughly `tan(pitch) · layer_depth`.
    pub parallax: f32,
    /// Sun elevation above the horizon, `0` = grazing (long shadows), `1` =
    /// overhead (flat). Low values give the strongest 3D relief from above.
    pub sun_elevation: f32,
    /// View-ray extinction coefficient — how fast the cloud becomes opaque
    /// (edge crispness vs. translucency).
    pub extinction: f32,
    /// Light-ray extinction — strength of the self-shadowing toward the sun.
    pub light_extinction: f32,
    /// Diagnostic output selector. [`DebugView::Final`] renders the real
    /// overlay; other variants emit an internal pipeline stage so it can be
    /// inspected and measured. Always [`DebugView::Final`] in production.
    pub debug_view: DebugView,
}

impl Default for CloudParams {
    fn default() -> Self {
        Self {
            resolution: [960.0, 640.0],
            time: 0.0,
            blend: 0.0,
            wind: [1.0, 0.25],
            sun_dir: [0.7, -0.5],
            map_scale: 6.0,
            erosion: 0.5,
            softness: 0.5,
            intensity: 0.95,
            parallax: 0.0,
            sun_elevation: 0.28,
            extinction: 7.0,
            light_extinction: 11.0,
            debug_view: DebugView::Final,
        }
    }
}

/// Owns the cloud + basemap pipelines and the two radar data textures.
pub struct CloudScene {
    basemap_pipeline: wgpu::RenderPipeline,
    cloud_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    tex_a: wgpu::Texture,
    tex_b: wgpu::Texture,
    data_w: u32,
    data_h: u32,
}

impl CloudScene {
    /// Build the scene for a given target colour `format` and radar grid
    /// dimensions. The two data textures are allocated up front and
    /// refilled per frame via [`CloudScene::upload`].
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        data_w: u32,
        data_h: u32,
    ) -> Self {
        let cloud_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-clouds-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("clouds.wgsl").into()),
        });
        let basemap_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-clouds-basemap-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("basemap.wgsl").into()),
        });

        let tex_a = create_data_texture(device, data_w, data_h, "radar-a");
        let tex_b = create_data_texture(device, data_w, data_h, "radar-b");
        let view_a = tex_a.create_view(&Default::default());
        let view_b = tex_b.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-clouds-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-clouds-uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-clouds-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-clouds-bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let cloud_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-clouds-layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let basemap_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-clouds-basemap-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let basemap_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-clouds-basemap-pipeline"),
            layout: Some(&basemap_layout),
            vertex: wgpu::VertexState {
                module: &basemap_shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &basemap_shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let cloud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-clouds-pipeline"),
            layout: Some(&cloud_layout),
            vertex: wgpu::VertexState {
                module: &cloud_shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &cloud_shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Premultiplied-alpha compositing onto the basemap.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            basemap_pipeline,
            cloud_pipeline,
            uniform_buffer,
            bind_group,
            tex_a,
            tex_b,
            data_w,
            data_h,
        }
    }

    /// Upload a radar frame into slot `0` (A) or `1` (B). The frame's
    /// dimensions must match the scene's grid.
    pub fn upload(&self, queue: &wgpu::Queue, slot: usize, frame: &RadarFrame) {
        assert_eq!(
            (frame.width, frame.height),
            (self.data_w, self.data_h),
            "radar frame dimensions must match the scene grid"
        );
        let tex = if slot == 0 { &self.tex_a } else { &self.tex_b };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.to_rgba8(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.data_w * 4),
                rows_per_image: Some(self.data_h),
            },
            wgpu::Extent3d {
                width: self.data_w,
                height: self.data_h,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Record one composited frame (basemap, then cloud overlay) into
    /// `encoder`, drawing into `view`. When `draw_basemap` is false only
    /// the premultiplied cloud layer is drawn, leaving whatever is already
    /// in the target (e.g. the live map) showing through.
    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        params: &CloudParams,
        draw_basemap: bool,
    ) {
        let uniforms = Uniforms {
            resolution: params.resolution,
            time: params.time,
            blend: params.blend.clamp(0.0, 1.0),
            wind: params.wind,
            sun_dir: params.sun_dir,
            map_scale: params.map_scale,
            erosion: params.erosion,
            softness: params.softness,
            intensity: params.intensity,
            parallax: params.parallax,
            sun_elevation: params.sun_elevation,
            extinction: params.extinction,
            light_extinction: params.light_extinction,
            debug_view: params.debug_view.code(),
            _pad: [0; 3],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let load = if draw_basemap {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.16,
                g: 0.28,
                b: 0.42,
                a: 1.0,
            })
        } else {
            wgpu::LoadOp::Load
        };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("turbomap-clouds-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        if draw_basemap {
            pass.set_pipeline(&self.basemap_pipeline);
            pass.draw(0..3, 0..1);
        }

        pass.set_pipeline(&self.cloud_pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn create_data_texture(device: &wgpu::Device, w: u32, h: u32, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
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
    })
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}
