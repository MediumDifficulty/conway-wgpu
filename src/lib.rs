pub mod gui;

use std::sync::Arc;

use glam::{uvec2, UVec2};
use gui::GuiRenderer;
use rand::Rng;
use wgpu::{include_wgsl, CommandEncoder, Texture, TextureUsages, TextureView};
use winit::{
    application::ApplicationHandler, event::{ElementState, KeyEvent, WindowEvent}, event_loop::EventLoop, keyboard::{KeyCode, PhysicalKey}, window::Window
};

struct AppState {
    renderer: RendererContext<'static>,
    game_of_life: GameOfLifeState,
    gui: GuiRenderer,
    window: Arc<Window>, // TODO: I really dislike the use of an `Arc` here but I can't find a way around it
}

#[derive(Default)]
pub struct App {
    state: Option<AppState>
}

pub struct RendererContext<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,   
}

struct GameOfLifeState {
    render_pipeline: wgpu::RenderPipeline,
    fragment_bind_groups: [wgpu::BindGroup; 2],
    compute_pipeline: wgpu::ComputePipeline,
    compute_bind_groups: [wgpu::BindGroup; 2],
    frame_polarity: bool,
}

pub fn run() {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app: App = App::default();

    event_loop.run_app(&mut app).unwrap();
}


impl AppState {
    async fn new(window: Window) -> Self {
        let window = Arc::new(window);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    
        let renderer = RendererContext::new(window.clone(), &instance).await;
        let game_of_life = GameOfLifeState::new(&renderer);
        let gui = GuiRenderer::new(&renderer.device, renderer.config.format, None, 1, &window);

        Self {
            game_of_life,
            gui,
            renderer,
            window
        }
    }
}

impl App {
    async fn set_state(&mut self, window: Window) {
        if self.state.is_some() {
            return;
        }

        let state = AppState::new(window).await;
        self.state = Some(state);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();

        pollster::block_on(self.set_state(window));
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(r) => r,
            None => return
        };

        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.renderer.resize(size);
            }
            WindowEvent::RedrawRequested => {
                state.window.request_redraw();
            
                match state.renderer.render(&state.window, &mut state.gui, &mut state.game_of_life) {
                    Ok(_) => {}

                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        state.renderer.resize(state.renderer.size)
                    }

                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        eprintln!("OutOfMemory");
                        event_loop.exit();
                    }

                    Err(wgpu::SurfaceError::Timeout) => {
                        eprintln!("Surface timeout")
                    }
                }
            }
            _ => {}
        };
        state.gui.handle_input(&state.window, &event);
    }
}

const WORLD_SIZE: UVec2 = uvec2(4096, 4096);

impl GameOfLifeState {
    pub fn new(renderer: &RendererContext) -> Self {
        let textures: [Texture; 2] = (0..2)
            .map(|_| {
                renderer.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: WORLD_SIZE.x,
                        height: WORLD_SIZE.y,
                        depth_or_array_layers: 1,
                    },
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R32Uint,
                    mip_level_count: 1,
                    sample_count: 1,
                    usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST,
                    view_formats: &[],
                })
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let fragment_bind_group_layout =
            renderer.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    count: None,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                }],
            });

        let fragment_bind_groups = textures
            .iter()
            .map(|texture| {
                renderer.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    }],
                    label: Some("bind_group"),
                    layout: &fragment_bind_group_layout,
                })
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let render_pipeline_layout =
            renderer.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render Pipeline Layout"),
                bind_group_layouts: &[&fragment_bind_group_layout],
                push_constant_ranges: &[],
            });
        
        let graphics_shader = renderer.device.create_shader_module(include_wgsl!("conway_renderer.wgsl"));
        let render_pipeline = renderer.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &graphics_shader,
                entry_point: Some("vs_main"), // Unnecessary
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &graphics_shader,
                entry_point: Some("fs_main"), // Unnecessary
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: renderer.config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let compute_shader = renderer.device.create_shader_module(wgpu::include_wgsl!("conway_compute.wgsl"));

        let compute_bind_group_layout = renderer.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compute_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    count: None,
                    ty: wgpu::BindingType::StorageTexture { 
                        access: wgpu::StorageTextureAccess::ReadOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2
                    },
                    visibility: wgpu::ShaderStages::COMPUTE
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    count: None,
                    ty: wgpu::BindingType::StorageTexture { 
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2
                    },
                    visibility: wgpu::ShaderStages::COMPUTE
                },
            ]
        });

        let compute_bind_groups = [[&textures[0], &textures[1]], [&textures[1], &textures[0]]]
            .into_iter()
            .map(|[texture_a, texture_b]| {
                renderer.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture_a.create_view(&wgpu::TextureViewDescriptor::default()))
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&texture_b.create_view(&wgpu::TextureViewDescriptor::default()))
                        }
                    ],
                    layout: &compute_bind_group_layout
                })
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        
        let compute_pipeline_layout = renderer.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&compute_bind_group_layout],
            label: Some("compute_pipeline_layout"),
            push_constant_ranges: &[]
        });

        let compute_pipeline = renderer.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            cache: None,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            entry_point: Some("update"),
            label: Some("update_pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader
        });

        let mut rng = rand::thread_rng();
        renderer.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &textures[0],
                mip_level: 0,
                aspect: wgpu::TextureAspect::All,
                origin: wgpu::Origin3d::ZERO,
            },
            &(0..WORLD_SIZE.element_product())
                .flat_map(|_| (rng.gen_bool(0.3) as u32).to_le_bytes())
                .collect::<Vec<_>>(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(WORLD_SIZE.x * size_of::<u32>() as u32),
                rows_per_image: Some(WORLD_SIZE.y),
            },
            wgpu::Extent3d {
                depth_or_array_layers: 1,
                width: WORLD_SIZE.x,
                height: WORLD_SIZE.y,
            },
        );

        Self {
            compute_bind_groups,
            compute_pipeline,
            fragment_bind_groups,
            frame_polarity: false,
            render_pipeline
        }
    }

    pub fn render(&mut self, renderer: &RendererContext, view: &TextureView, render_encoder: &mut CommandEncoder) {
        let mut encoder = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder")
        });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_groups[self.frame_polarity as usize], &[]);
            compute_pass.dispatch_workgroups(WORLD_SIZE.x / 8, WORLD_SIZE.y / 8, 1);
        }
        renderer.queue.submit(std::iter::once(encoder.finish()));
            
        {
            let mut render_pass = render_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.fragment_bind_groups[self.frame_polarity as usize], &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.frame_polarity = !self.frame_polarity;
    }
}

impl RendererContext<'static> {
    async fn new(window: Arc<Window>, instance: &wgpu::Instance) -> RendererContext<'static> {
        let size = window.inner_size();
        let surface: wgpu::Surface = instance.create_surface(window).unwrap();
        
        // Handle to the graphics card
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        // Connection to the device
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                    required_limits: wgpu::Limits::default(),
                    label: None,
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        println!("{:?}", surface_format);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        Self {
            config,
            device,
            queue,
            size,
            surface,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn render(&mut self, window: &Window, gui: &mut GuiRenderer, gol: &mut GameOfLifeState) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut render_encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render encoder")
        });

        gol.render(self, &view, &mut render_encoder);

        gui.draw(window, &self.device, &self.queue, &self.config, gui::gui, &mut render_encoder, &view);

        self.queue.submit(core::iter::once(render_encoder.finish()));

        output.present();

        Ok(())
    }
}
