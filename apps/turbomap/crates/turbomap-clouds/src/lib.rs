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
pub mod noise3d;

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
    /// Diagnostic: the per-pixel parallax shift (R=x, G=y, grey=zero).
    Parallax,
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
            DebugView::Parallax => 8,
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
            DebugView::Parallax => "parallax",
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
    use_ray: u32,
    cloud_alt_base: f32,
    cloud_alt_top: f32,
    inv_view_proj: [[f32; 4]; 4],
    world_to_field: [f32; 2],
    fuv_origin: [f32; 2],
    fuv_dx: [f32; 2],
    fuv_dy: [f32; 2],
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
    /// When `true`, reconstruct the real per-pixel camera ray from
    /// [`Self::inv_view_proj`] and shift the cloud field by the ground↔
    /// cloud-altitude parallax — so a *pitched* map reveals the puff sides
    /// (true 3D). When `false`, the flat top-down path is used (with the
    /// [`Self::parallax`] uniform fallback). Set by the map from its camera.
    pub use_camera_ray: bool,
    /// Inverse view-projection (clip → world), column-major. Lets the shader
    /// turn each screen pixel back into a world-space ray.
    pub inv_view_proj: [[f32; 4]; 4],
    /// Cloud layer bottom / top altitude in world units (same frame as
    /// `inv_view_proj`). The slab the ray is intersected against.
    pub cloud_alt_base: f32,
    /// Cloud layer top altitude in world units.
    pub cloud_alt_top: f32,
    /// World (mercator) → field-uv scale, per axis (`= 1/radar_span`). Converts
    /// the camera-ray's world-space parallax offset into a field-uv shift so a
    /// pitched map rakes through the (world-locked) cloud volume.
    pub world_to_field: [f32; 2],
    /// Diagnostic output selector. [`DebugView::Final`] renders the real
    /// overlay; other variants emit an internal pipeline stage so it can be
    /// inspected and measured. Always [`DebugView::Final`] in production.
    pub debug_view: DebugView,
    /// Screen-uv → field-uv affine that world-locks the cloud field:
    /// `field_uv = origin + uv.x·dx + uv.y·dy`. Identity
    /// (`[0,0] / [1,0] / [0,1]`) leaves the field screen-locked (offscreen /
    /// golden); the live map fills it from its camera + the radar's geo box so
    /// the clouds pan and zoom with the terrain. See [`field_uv`] in the shader.
    pub field_uv_origin: [f32; 2],
    pub field_uv_dx: [f32; 2],
    pub field_uv_dy: [f32; 2],
}

impl Default for CloudParams {
    fn default() -> Self {
        Self {
            resolution: [960.0, 640.0],
            time: 0.0,
            blend: 0.0,
            wind: [1.0, 0.25],
            sun_dir: [0.7, -0.5],
            map_scale: 8.0,
            erosion: 0.5,
            softness: 0.5,
            // Translucent overlay, not an opaque cover: you should see the map
            // (and terrain) through the weather. Was 1.0 (fully opaque) which
            // read as "clouds cover the whole screen" over real overcast data.
            // Slightly thinner veil so the shipped overlay reads as cloud, not a
            // flat white film over the map ("spilled milk").
            intensity: 0.50,
            parallax: 0.0,
            sun_elevation: 0.35,
            extinction: 15.0,
            // Stronger self-shadowing → thick cloud goes genuinely dark
            // underneath, so puffs read as 3D form instead of a uniform wash.
            light_extinction: 18.0,
            use_camera_ray: false,
            inv_view_proj: IDENTITY4,
            cloud_alt_base: 0.0,
            cloud_alt_top: 0.0,
            world_to_field: [0.0, 0.0],
            debug_view: DebugView::Final,
            // Identity affine → field_uv == screen uv (screen-locked default).
            field_uv_origin: [0.0, 0.0],
            field_uv_dx: [1.0, 0.0],
            field_uv_dy: [0.0, 1.0],
        }
    }
}

/// Column-major 4×4 identity, the no-op default for `inv_view_proj`.
const IDENTITY4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// A lazily-(re)allocated half-resolution cloud target plus the bind group
/// that samples it in the upscale pass. Recreated when the viewport resizes.
struct HalfResTarget {
    _tex: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
}

/// Owns the cloud + basemap pipelines and the two radar data textures.
pub struct CloudScene {
    basemap_pipeline: wgpu::RenderPipeline,
    cloud_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    tex_a: wgpu::Texture,
    tex_b: wgpu::Texture,
    /// Precomputed tileable 3D cloud noise, sampled by the shader (kept alive
    /// for the bind group).
    _noise_tex: wgpu::Texture,
    data_w: u32,
    data_h: u32,
    /// Cheap cloneable device handle, kept so the live overlay path can
    /// (re)allocate its half-res target on viewport resize.
    device: wgpu::Device,
    /// Surface colour format — also used for the half-res cloud target so the
    /// same `cloud_pipeline` renders into both.
    format: wgpu::TextureFormat,
    upscale_pipeline: wgpu::RenderPipeline,
    upscale_bgl: wgpu::BindGroupLayout,
    upscale_sampler: wgpu::Sampler,
    /// The half-res offscreen cloud buffer for the live overlay path; `None`
    /// until the first `render_overlay_downsampled` call (and the diagnostic
    /// full-res `render` path never touches it).
    half_res: Option<HalfResTarget>,
}

impl CloudScene {
    /// Build the scene for a given target colour `format` and radar grid
    /// dimensions. The two data textures are allocated up front and
    /// refilled per frame via [`CloudScene::upload`]; the 3D noise volume is
    /// generated + uploaded once here (needs `queue`).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
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

        // Precomputed tileable 3D noise — sampled in the shader instead of
        // computing analytic Perlin-Worley/Worley per march step.
        let noise_n = 64u32;
        let noise_data = noise3d::generate(noise_n);
        let noise_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("turbomap-clouds-noise3d"),
            size: wgpu::Extent3d {
                width: noise_n,
                height: noise_n,
                depth_or_array_layers: noise_n,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &noise_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &noise_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(noise_n * 4),
                rows_per_image: Some(noise_n),
            },
            wgpu::Extent3d {
                width: noise_n,
                height: noise_n,
                depth_or_array_layers: noise_n,
            },
        );
        let noise_view = noise_tex.create_view(&Default::default());
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-clouds-noise-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&noise_sampler),
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

        // ---- Half-res upscale-composite path -----------------------------
        // The march runs into a half-res offscreen target (same `format`, so it
        // reuses `cloud_pipeline`); this pass samples it bilinearly and
        // composites onto the full-res surface. Keeps the live overlay cheap
        // enough for mobile / software GPUs without changing the look.
        let upscale_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-clouds-upscale-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("upscale.wgsl").into()),
        });
        let upscale_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-clouds-upscale-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let upscale_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-clouds-upscale-bgl"),
            entries: &[
                texture_entry(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let upscale_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-clouds-upscale-layout"),
            bind_group_layouts: &[Some(&upscale_bgl)],
            immediate_size: 0,
        });
        let upscale_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-clouds-upscale-pipeline"),
            layout: Some(&upscale_layout),
            vertex: wgpu::VertexState {
                module: &upscale_shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &upscale_shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Same premultiplied composite as the direct cloud pass.
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
            _noise_tex: noise_tex,
            data_w,
            data_h,
            device: device.clone(),
            format,
            upscale_pipeline,
            upscale_bgl,
            upscale_sampler,
            half_res: None,
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
            use_ray: params.use_camera_ray as u32,
            cloud_alt_base: params.cloud_alt_base,
            cloud_alt_top: params.cloud_alt_top,
            inv_view_proj: params.inv_view_proj,
            world_to_field: params.world_to_field,
            fuv_origin: params.field_uv_origin,
            fuv_dx: params.field_uv_dx,
            fuv_dy: params.field_uv_dy,
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

    /// Live-overlay path: run the volumetric march into a **half-resolution**
    /// offscreen buffer, then upscale-composite it onto `target`. The march
    /// dominates cost (≈28×5 noise-textured samples per pixel), so quartering
    /// the pixel count keeps it within budget on mobile / software GPUs while
    /// the bilinear upscale preserves the soft look. `target` keeps whatever it
    /// already holds (the live map) and the cloud layer composites over it.
    ///
    /// `scale` is the linear downsample factor (`2` = half-res, quarter the
    /// pixels); the diagnostic full-res look is unchanged via [`Self::render`].
    pub fn render_overlay_downsampled(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        params: &CloudParams,
        scale: u32,
    ) {
        let scale = scale.max(1);
        let full_w = params.resolution[0].max(1.0) as u32;
        let full_h = params.resolution[1].max(1.0) as u32;
        let hw = full_w.div_ceil(scale).max(1);
        let hh = full_h.div_ceil(scale).max(1);

        // The march samples the field in uv space, so the uniforms (incl. the
        // full-res `resolution`) are identical to the direct path — only the
        // sampling density drops. Write them once for the cloud pass.
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
            use_ray: params.use_camera_ray as u32,
            cloud_alt_base: params.cloud_alt_base,
            cloud_alt_top: params.cloud_alt_top,
            inv_view_proj: params.inv_view_proj,
            world_to_field: params.world_to_field,
            fuv_origin: params.field_uv_origin,
            fuv_dx: params.field_uv_dx,
            fuv_dy: params.field_uv_dy,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // (Re)allocate the half-res target when the viewport size changes. The
        // target uses the surface `format`, so `cloud_pipeline` renders into it
        // and the upscale pass samples it back (sRGB round-trips premultiplied
        // colour correctly; alpha is linear in both directions).
        if self.half_res.as_ref().map(|h| h.size) != Some((hw, hh)) {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("turbomap-clouds-halfres-target"),
                size: wgpu::Extent3d {
                    width: hw,
                    height: hh,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("turbomap-clouds-upscale-bg"),
                layout: &self.upscale_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.upscale_sampler),
                    },
                ],
            });
            self.half_res = Some(HalfResTarget {
                _tex: tex,
                view,
                bind_group,
                size: (hw, hh),
            });
        }
        let hr = self.half_res.as_ref().expect("half-res target just set");

        // Pass A — march into the half-res buffer (cleared to transparent).
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("turbomap-clouds-halfres-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &hr.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.cloud_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // Pass B — upscale-composite the half-res cloud onto the live surface.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("turbomap-clouds-upscale-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.upscale_pipeline);
            pass.set_bind_group(0, &hr.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
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

#[cfg(test)]
mod uniform_layout {
    use super::Uniforms;
    use std::mem::{offset_of, size_of};

    // The WGSL `Uniforms` struct (std140-like) places each field at these
    // byte offsets; the Rust mirror MUST match or writing one field corrupts
    // another on the GPU. Verified against clouds.wgsl.
    #[test]
    fn matches_wgsl_layout() {
        assert_eq!(size_of::<Uniforms>(), 176, "struct size");
        assert_eq!(offset_of!(Uniforms, map_scale), 32);
        assert_eq!(offset_of!(Uniforms, intensity), 44);
        assert_eq!(offset_of!(Uniforms, parallax), 48);
        assert_eq!(offset_of!(Uniforms, light_extinction), 60);
        assert_eq!(offset_of!(Uniforms, debug_view), 64);
        assert_eq!(offset_of!(Uniforms, use_ray), 68);
        assert_eq!(offset_of!(Uniforms, cloud_alt_base), 72);
        assert_eq!(offset_of!(Uniforms, cloud_alt_top), 76);
        // mat4x4<f32> requires 16-byte alignment in WGSL → offset 80.
        assert_eq!(offset_of!(Uniforms, inv_view_proj), 80);
        // vec2<f32>s (8-byte aligned) packed after the mat4 (ends at 144).
        assert_eq!(offset_of!(Uniforms, world_to_field), 144);
        assert_eq!(offset_of!(Uniforms, fuv_origin), 152);
        assert_eq!(offset_of!(Uniforms, fuv_dx), 160);
        assert_eq!(offset_of!(Uniforms, fuv_dy), 168);
    }
}
