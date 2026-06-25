//! HDR post-process: bloom + filmic tonemap.
//!
//! The frame pass resolves into a linear HDR texture
//! ([`FrameTargets::hdr_resolve_view`]) where highlights exceed 1.0. This stage
//! turns that into the final sRGB image with four fullscreen passes (all from
//! `post_shader.wgsl`):
//!
//!   1. bright   — half-res soft-knee bright-pass: `hdr_resolve` → `bloom_a`.
//!   2. blur_h   — separable Gaussian, horizontal:  `bloom_a` → `bloom_b`.
//!   3. blur_v   — separable Gaussian, vertical:    `bloom_b` → `bloom_a`.
//!   4. tonemap  — scene + upsampled bloom, ACES filmic: `hdr_resolve` +
//!                 `bloom_a` → the surface.
//!
//! Bind groups are rebuilt per frame from the (resize-recreated) target views;
//! they're trivially cheap and it keeps the post-process stateless w.r.t. size.

use super::{targets::FrameTargets, HDR_FORMAT};

pub(crate) struct PostProcess {
    bright: wgpu::RenderPipeline,
    blur_h: wgpu::RenderPipeline,
    blur_v: wgpu::RenderPipeline,
    tonemap: wgpu::RenderPipeline,
    /// One texture+sampler layout, shared by every pass's input binding (and by
    /// the tonemap's second, bloom, group).
    tex_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl PostProcess {
    pub(crate) fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-post-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post_shader.wgsl").into()),
        });

        let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-post-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Single-input passes (bright, blur_h, blur_v) bind one texture group.
        let single_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-post-single-layout"),
            bind_group_layouts: &[Some(&tex_bgl)],
            immediate_size: 0,
        });
        // The tonemap pass binds the scene (group 0) and the bloom (group 1).
        let tonemap_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-post-tonemap-layout"),
            bind_group_layouts: &[Some(&tex_bgl), Some(&tex_bgl)],
            immediate_size: 0,
        });

        let make = |label: &str,
                    layout: &wgpu::PipelineLayout,
                    fs_entry: &str,
                    format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fullscreen"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs_entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                // No depth and single-sample throughout the post chain.
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let bright = make(
            "turbomap-post-bright",
            &single_layout,
            "fs_bright",
            HDR_FORMAT,
        );
        let blur_h = make(
            "turbomap-post-blur-h",
            &single_layout,
            "fs_blur_h",
            HDR_FORMAT,
        );
        let blur_v = make(
            "turbomap-post-blur-v",
            &single_layout,
            "fs_blur_v",
            HDR_FORMAT,
        );
        // The final pass writes the sRGB surface, so it OETF-encodes on store.
        let tonemap = make(
            "turbomap-post-tonemap",
            &tonemap_layout,
            "fs_tonemap",
            surface_format,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-post-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            bright,
            blur_h,
            blur_v,
            tonemap,
            tex_bgl,
            sampler,
        }
    }

    /// Build a one-texture bind group from `view` + the shared sampler.
    fn tex_bind(&self, device: &wgpu::Device, view: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-post-bg"),
            layout: &self.tex_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// One fullscreen pass: bind `inputs` (in group order), draw the triangle
    /// into `target`. Clears the target (the triangle covers it anyway, but a
    /// `Clear` load op is the cheaper choice on tiled mobile GPUs).
    fn full_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        inputs: &[&wgpu::BindGroup],
        target: &wgpu::TextureView,
        label: &str,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        for (i, bg) in inputs.iter().enumerate() {
            pass.set_bind_group(i as u32, *bg, &[]);
        }
        pass.draw(0..3, 0..1);
    }

    /// Run the full bloom + tonemap chain: read `targets.hdr_resolve`, write the
    /// final image to `surface_target`. Call after the frame pass has resolved.
    pub(crate) fn run(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        targets: &FrameTargets,
        surface_target: &wgpu::TextureView,
    ) {
        let scene_bg = self.tex_bind(device, targets.hdr_resolve_view());
        let bloom_a_bg = self.tex_bind(device, targets.bloom_a_view());
        let bloom_b_bg = self.tex_bind(device, targets.bloom_b_view());

        // 1. bright-pass + downsample → bloom_a
        self.full_pass(
            encoder,
            &self.bright,
            &[&scene_bg],
            targets.bloom_a_view(),
            "turbomap-post-bright-pass",
        );
        // 2. horizontal blur: bloom_a → bloom_b
        self.full_pass(
            encoder,
            &self.blur_h,
            &[&bloom_a_bg],
            targets.bloom_b_view(),
            "turbomap-post-blur-h-pass",
        );
        // 3. vertical blur: bloom_b → bloom_a
        self.full_pass(
            encoder,
            &self.blur_v,
            &[&bloom_b_bg],
            targets.bloom_a_view(),
            "turbomap-post-blur-v-pass",
        );
        // 4. composite + tonemap: scene + bloom_a → surface
        self.full_pass(
            encoder,
            &self.tonemap,
            &[&scene_bg, &bloom_a_bg],
            surface_target,
            "turbomap-post-tonemap-pass",
        );
    }
}
