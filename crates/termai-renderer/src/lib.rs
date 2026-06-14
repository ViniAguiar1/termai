pub mod atlas;

use atlas::{GlyphAtlas, GlyphInfo};
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::util::DeviceExt;

// Embedded monospace font (JetBrains Mono)
const FONT_BYTES: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    fg_color: [f32; 4],
    bg_color: [f32; 4],
    is_bg: f32,
    _padding: [f32; 1],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

/// A terminal cell to render.
#[derive(Clone)]
pub struct RenderCell {
    pub ch: char,
    pub fg: [f32; 4],
    pub bg: [f32; 4],
    pub underline: bool,
}

/// GPU-accelerated terminal text renderer.
pub struct Renderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    atlas: GlyphAtlas,
    chrome_atlas: GlyphAtlas,
    chrome_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    /// Background clear color (set from theme).
    pub clear_color: [f32; 4],
}

impl Renderer {
    /// Create a new renderer attached to the given window.
    pub fn new(window: Arc<winit::window::Window>, scale_factor: f32, font_size: f32) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("termai-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
            None,
        ))
        .expect("Failed to create device");

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        // Prefer a non-sRGB format: our hex color tokens are already in
        // perceptual sRGB space, so an sRGB surface would double-encode and
        // brighten the rendered output. Falling back to whatever's available
        // if no Unorm match (rare).
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Build glyph atlas scaled for HiDPI
        let pixel_font_size = font_size * scale_factor;
        let atlas = GlyphAtlas::new(FONT_BYTES, pixel_font_size);

        // Upload atlas texture to GPU
        let atlas_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("glyph-atlas"),
                size: wgpu::Extent3d {
                    width: atlas.texture_width,
                    height: atlas.texture_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &atlas.texture_data,
        );

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Uniform buffer
        let uniforms = Uniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0; 2],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniforms"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind-group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // Chrome atlas at smaller font size for UI text (tabs, search bar, AI overlay)
        let chrome_font_size = 12.0 * scale_factor;
        let chrome_atlas = GlyphAtlas::new(FONT_BYTES, chrome_font_size);

        let chrome_atlas_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("chrome-glyph-atlas"),
                size: wgpu::Extent3d {
                    width: chrome_atlas.texture_width,
                    height: chrome_atlas.texture_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &chrome_atlas.texture_data,
        );

        let chrome_atlas_view = chrome_atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let chrome_atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let chrome_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("chrome-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&chrome_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&chrome_atlas_sampler),
                },
            ],
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            device,
            queue,
            surface,
            surface_config,
            pipeline,
            bind_group_layout,
            bind_group,
            uniform_buffer,
            atlas,
            chrome_atlas,
            chrome_bind_group,
            width,
            height,
            clear_color: [0.07, 0.07, 0.09, 1.0],
        }
    }

    /// Handle window resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);

        let uniforms = Uniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Rebuild the glyph atlas with a new font size (in logical pixels).
    pub fn rebuild_atlas(&mut self, font_size: f32, scale_factor: f32) {
        let pixel_font_size = font_size * scale_factor;
        self.atlas = GlyphAtlas::new(FONT_BYTES, pixel_font_size);

        let atlas_texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("glyph-atlas"),
                size: wgpu::Extent3d {
                    width: self.atlas.texture_width,
                    height: self.atlas.texture_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &self.atlas.texture_data,
        );

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Cell dimensions in pixels.
    pub fn cell_size(&self) -> (f32, f32) {
        (self.atlas.cell_width, self.atlas.cell_height)
    }

    /// Cell dimensions for the chrome (UI) atlas in pixels.
    pub fn chrome_cell_size(&self) -> (f32, f32) {
        (self.chrome_atlas.cell_width, self.chrome_atlas.cell_height)
    }

    /// Draw a free-positioned text run (not aligned to the grid) using the chrome atlas.
    /// Glyphs are rasterized from and drawn with `self.chrome_atlas`.
    /// Returns the pixel width consumed by the text.
    pub fn build_chrome_text_run(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        vertices: &mut Vec<Vertex>,
    ) -> f32 {
        let (cell_w, _) = self.chrome_cell_size();
        let mut cursor_x = x;

        for ch in text.chars() {
            if ch == ' ' {
                cursor_x += cell_w;
                continue;
            }
            if let Some(glyph) = self.chrome_atlas.get_or_insert(ch) {
                let glyph = *glyph;
                let x0 = cursor_x + glyph.offset_x;
                let y0 = y + glyph.offset_y;
                let x1 = x0 + glyph.width;
                let y1 = y0 + glyph.height;
                push_quad(
                    vertices,
                    x0, y0, x1, y1,
                    [glyph.uv_x, glyph.uv_y],
                    [glyph.uv_x + glyph.uv_w, glyph.uv_y + glyph.uv_h],
                    color,
                    [0.0; 4],
                    0.0,
                );
            }
            cursor_x += cell_w;
        }

        cursor_x - x
    }

    /// Measure the pixel width a text run would consume in the chrome atlas, without drawing it.
    pub fn measure_chrome_text(&self, text: &str) -> f32 {
        let cell_w = self.chrome_atlas.cell_width;
        text.chars().count() as f32 * cell_w
    }

    /// Build an outline rectangle of configurable thickness (4 thin rects).
    pub fn build_rect_outline(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        thickness: f32,
        color: [f32; 4],
        vertices: &mut Vec<Vertex>,
    ) {
        self.build_rect(x, y, w, thickness, color, vertices);               // top
        self.build_rect(x, y + h - thickness, w, thickness, color, vertices); // bottom
        self.build_rect(x, y, thickness, h, color, vertices);               // left
        self.build_rect(x + w - thickness, y, thickness, h, color, vertices); // right
    }

    /// Grid dimensions that fit the current window.
    pub fn grid_size(&self) -> (u32, u32) {
        let cols = (self.width as f32 / self.atlas.cell_width).floor() as u32;
        let rows = (self.height as f32 / self.atlas.cell_height).floor() as u32;
        (cols.max(1), rows.max(1))
    }

    /// Grid dimensions that fit a given pixel area.
    pub fn grid_size_for(&self, width: f32, height: f32) -> (u32, u32) {
        let cols = (width / self.atlas.cell_width).floor() as u32;
        let rows = (height / self.atlas.cell_height).floor() as u32;
        (cols.max(1), rows.max(1))
    }

    /// A pane region to render in a single frame.
    pub fn build_vertices(
        &mut self,
        cells: &[Vec<RenderCell>],
        offset_x: f32,
        offset_y: f32,
        vertices: &mut Vec<Vertex>,
    ) {
        let (cell_w, cell_h) = self.cell_size();

        for (row_idx, row) in cells.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                let x = offset_x + col_idx as f32 * cell_w;
                let y = offset_y + row_idx as f32 * cell_h;

                push_quad(
                    vertices,
                    x,
                    y,
                    x + cell_w,
                    y + cell_h,
                    [0.0, 0.0],
                    [0.0, 0.0],
                    cell.fg,
                    cell.bg,
                    1.0,
                );

                if cell.ch != ' ' {
                    if let Some(glyph) = self.atlas.get_or_insert(cell.ch) {
                        let glyph = *glyph; // copy to release borrow
                        self.push_glyph_quad(vertices, x, y, &glyph, cell.fg, cell.bg);
                    }
                }

                if cell.underline {
                    let thickness = (cell_h * 0.06).max(1.0).round();
                    let line_y = y + cell_h - thickness - 1.0;
                    push_quad(
                        vertices,
                        x,
                        line_y,
                        x + cell_w,
                        line_y + thickness,
                        [0.0, 0.0],
                        [0.0, 0.0],
                        cell.fg,
                        cell.fg,
                        1.0,
                    );
                }
            }
        }
    }

    /// Add a divider line (for splits).
    pub fn build_divider(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        vertices: &mut Vec<Vertex>,
    ) {
        push_quad(
            vertices,
            x, y, x + w, y + h,
            [0.0, 0.0], [0.0, 0.0],
            color, color, 1.0,
        );
    }

    /// Fill a rectangle with a solid color.
    pub fn build_rect(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        vertices: &mut Vec<Vertex>,
    ) {
        push_quad(
            vertices,
            x, y, x + w, y + h,
            [0.0, 0.0], [0.0, 0.0],
            color, color, 1.0,
        );
    }

    /// Check if the atlas texture needs to be re-uploaded to the GPU.
    pub fn atlas_needs_reupload(&self) -> bool {
        self.atlas.is_dirty()
    }

    /// Re-upload the atlas texture to the GPU after new glyphs were rasterized.
    pub fn reupload_atlas(&mut self) {
        let atlas_texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("glyph-atlas"),
                size: wgpu::Extent3d {
                    width: self.atlas.texture_width,
                    height: self.atlas.texture_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &self.atlas.texture_data,
        );

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        self.atlas.clear_dirty();
    }

    /// Check if the chrome atlas texture needs to be re-uploaded to the GPU.
    pub fn chrome_atlas_needs_reupload(&self) -> bool {
        self.chrome_atlas.is_dirty()
    }

    /// Re-upload the chrome atlas texture to the GPU after new glyphs were rasterized.
    pub fn reupload_chrome_atlas(&mut self) {
        let chrome_atlas_texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("chrome-glyph-atlas"),
                size: wgpu::Extent3d {
                    width: self.chrome_atlas.texture_width,
                    height: self.chrome_atlas.texture_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &self.chrome_atlas.texture_data,
        );

        let chrome_atlas_view = chrome_atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let chrome_atlas_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.chrome_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("chrome-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&chrome_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&chrome_atlas_sampler),
                },
            ],
        });

        self.chrome_atlas.clear_dirty();
    }

    /// Render a grid of cells to the screen (single pane, backwards compatible).
    pub fn render(&mut self, cells: &[Vec<RenderCell>]) -> Result<(), wgpu::SurfaceError> {
        let mut vertices: Vec<Vertex> = Vec::new();
        self.build_vertices(cells, 0.0, 0.0, &mut vertices);
        self.submit_frame(&vertices, &[])
    }

    /// Render pre-built vertices to the screen.
    ///
    /// `main_vertices` use the main (content) atlas.
    /// `chrome_vertices` use the chrome atlas.
    pub fn submit_frame(
        &self,
        main_vertices: &[Vertex],
        chrome_vertices: &[Vertex],
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let main_vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("main-vertex-buffer"),
            contents: bytemuck::cast_slice(main_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let chrome_vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chrome-vertex-buffer"),
            contents: bytemuck::cast_slice(chrome_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear_color[0] as f64,
                            g: self.clear_color[1] as f64,
                            b: self.clear_color[2] as f64,
                            a: self.clear_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            render_pass.set_pipeline(&self.pipeline);

            if !main_vertices.is_empty() {
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, main_vb.slice(..));
                render_pass.draw(0..main_vertices.len() as u32, 0..1);
            }

            if !chrome_vertices.is_empty() {
                render_pass.set_bind_group(0, &self.chrome_bind_group, &[]);
                render_pass.set_vertex_buffer(0, chrome_vb.slice(..));
                render_pass.draw(0..chrome_vertices.len() as u32, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn push_glyph_quad(
        &self,
        vertices: &mut Vec<Vertex>,
        cell_x: f32,
        cell_y: f32,
        glyph: &GlyphInfo,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        let x0 = cell_x + glyph.offset_x;
        let y0 = cell_y + glyph.offset_y;
        let x1 = x0 + glyph.width;
        let y1 = y0 + glyph.height;

        let uv_x0 = glyph.uv_x;
        let uv_y0 = glyph.uv_y;
        let uv_x1 = glyph.uv_x + glyph.uv_w;
        let uv_y1 = glyph.uv_y + glyph.uv_h;

        push_quad(
            vertices,
            x0,
            y0,
            x1,
            y1,
            [uv_x0, uv_y0],
            [uv_x1, uv_y1],
            fg,
            bg,
            0.0,
        );
    }
}

fn push_quad(
    vertices: &mut Vec<Vertex>,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    fg: [f32; 4],
    bg: [f32; 4],
    is_bg: f32,
) {
    let v = |px: f32, py: f32, u: f32, v: f32| Vertex {
        position: [px, py],
        uv: [u, v],
        fg_color: fg,
        bg_color: bg,
        is_bg,
        _padding: [0.0],
    };

    vertices.push(v(x0, y0, uv_min[0], uv_min[1]));
    vertices.push(v(x1, y0, uv_max[0], uv_min[1]));
    vertices.push(v(x0, y1, uv_min[0], uv_max[1]));

    vertices.push(v(x1, y0, uv_max[0], uv_min[1]));
    vertices.push(v(x1, y1, uv_max[0], uv_max[1]));
    vertices.push(v(x0, y1, uv_min[0], uv_max[1]));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_quad_emits_six_vertices() {
        let mut v: Vec<Vertex> = Vec::new();
        push_quad(&mut v, 0.0, 0.0, 10.0, 10.0, [0.0; 2], [1.0; 2], [1.0; 4], [0.0; 4], 1.0);
        assert_eq!(v.len(), 6);
    }
}
