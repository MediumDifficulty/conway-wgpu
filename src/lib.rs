pub mod gui;
mod rendering_utils;
mod input;

use std::{borrow::Cow, collections::HashMap, num::NonZero, sync::Arc, time::{Duration, Instant}};

use glam::{uvec2, vec2, UVec2, Vec2};
use gui::{GuiRenderer, UiState};
use input::{HybridInputState, InputSource};
use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue};
use rendering_utils::{Profiler, SimpleUniformHelper};
use wgpu::{naga::valid::Capabilities, CommandEncoder, ShaderStages, TextureView};
use winit::{
    application::ApplicationHandler, dpi::PhysicalSize, event::{ElementState, KeyEvent, TouchPhase, WindowEvent}, event_loop::EventLoop, keyboard::{KeyCode, PhysicalKey}, window::Window
};

struct AppState {
    renderer: RendererContext<'static>,
    game_of_life: GameOfLifeState,
    gui: GuiRenderer,
    ui_state: UiState,
    window: Arc<Window>, // FIXME: I really dislike the use of an `Arc` here but I can't find a way around it
}

pub struct App {
    last_update: Instant,
    state: Option<AppState>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            last_update: Instant::now(),
            state: None,
        }
    }
}

pub struct RendererContext<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,   
    limits: wgpu::Limits,
}

struct GameOfLifeState {
    render_pipeline: wgpu::RenderPipeline,
    fragment_bind_groups: [wgpu::BindGroup; 2],
    compute_pipeline: wgpu::ComputePipeline,
    init_pipeline: wgpu::ComputePipeline,
    compute_bind_groups: [wgpu::BindGroup; 2],
    frame_polarity: bool,
    camera: SimpleUniformHelper<CameraUniform>,
    input: HybridInputState<InputIdent>,
    profiler: Profiler
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputIdent {
    Up,
    Down,
    Left,
    Right
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
            window,
            ui_state: UiState::default(),
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

        if state.gui.handle_input(&state.window, &event) || state.game_of_life.handle_input(&event){
            // Event has been consumed
            return;
        }

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
            WindowEvent::KeyboardInput { 
                event,
                ..
            } => match event {
                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::F3),
                    ..
                } => state.gui.enabled = !state.gui.enabled,
                _ => {}
            }
            WindowEvent::Resized(size) => {
                state.renderer.resize(size);
                state.game_of_life.resize(size);
            }
            WindowEvent::RedrawRequested => {
                state.window.request_redraw();
                state.game_of_life.step_camera(self.last_update.elapsed());

                self.last_update = Instant::now();
                match state.render() {
                    Ok(_) => {
                        state.ui_state.update_time_per_frame = state.game_of_life.profiler.average_time(0);
                    }

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
    }
}

const BITS_PER_PIXEL: u32 = 32;

const WORLD_WIDTH: u32 = 1 << 16;
const WORLD_SIZE: UVec2 = uvec2(WORLD_WIDTH, WORLD_WIDTH);
const TEXTURE_SIZE: UVec2 = uvec2(WORLD_WIDTH / BITS_PER_PIXEL, WORLD_WIDTH);
const WORKGROUP_DIMS: UVec2 = TEXTURE_SIZE.saturating_div(uvec2(8, 8));

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Default, Debug)]
struct CameraUniform {
    screen_resolution: Vec2,
    centre: Vec2,
    zoom: f32,
    _padding: u32,
}

impl GameOfLifeState {
    pub fn handle_input(&mut self, event: &WindowEvent) -> bool {
        return match event {
            WindowEvent::MouseWheel { delta, phase: TouchPhase::Moved, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, amount) => self.camera.update_inner(|camera| {
                        camera.zoom *= 1. - *amount * 0.1;
                    }),
                    winit::event::MouseScrollDelta::PixelDelta(physical_position) => self.camera.update_inner(|camera| {
                        camera.zoom *= 1. - physical_position.y as f32 * 0.1;
                    }),
                };
                true
            },
            _ => {
                self.input.update_gamepad();
                self.input.handle_winit(event)
            }
        }
    }

    pub fn step_camera(&mut self, delta_time: Duration) {
        let delta_time = delta_time.as_secs_f32();
        
        self.camera.update_inner(|camera| {
            camera.centre.y -= camera.zoom * 150. * delta_time * self.input.pressed_amount(InputIdent::Up);
            camera.centre.y += camera.zoom * 150. * delta_time * self.input.pressed_amount(InputIdent::Down);
            camera.centre.x -= camera.zoom * 150. * delta_time * self.input.pressed_amount(InputIdent::Left);
            camera.centre.x += camera.zoom * 150. * delta_time * self.input.pressed_amount(InputIdent::Right);
        });
    }

    pub fn new(renderer: &RendererContext) -> Self {
        let mut composer = Composer::default()
            .with_capabilities(Capabilities::UNIFORM_BUFFER_AND_STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING);

        composer.add_composable_module(ComposableModuleDescriptor {
            source: include_str!("wgsl/conway_common.wgsl"),
            file_path: "wgsl/conway_common.wgsl",
            ..Default::default()
        }).unwrap();

        let world = World::new(renderer.limits.max_texture_dimension_2d, &renderer.device);
            
        let fragment_bind_group_layout = world.fragment_bind_group_layout(&renderer.device);
        let fragment_bind_groups = world.fragment_bind_groups(&fragment_bind_group_layout, &renderer.device);

        let camera = SimpleUniformHelper::from_inner(CameraUniform {
            centre: (WORLD_SIZE / 2).as_vec2(),
            // centre: vec2(32., 0.),
            zoom: 1.,
            screen_resolution: vec2(renderer.config.width as f32, renderer.config.height as f32),
            ..Default::default()
        }, &renderer.device, ShaderStages::FRAGMENT);
            

        let render_pipeline_layout =
            renderer.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render Pipeline Layout"),
                bind_group_layouts: &[&fragment_bind_group_layout, camera.layout()],
                push_constant_ranges: &[],
            });
        

        let graphics_shader = renderer.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Renderer"),
            source: wgpu::ShaderSource::Naga(Cow::Owned(composer.make_naga_module(NagaModuleDescriptor {
                source: include_str!("wgsl/conway_renderer.wgsl"),
                file_path: "wgsl/conway_renderer.wgsl",
                shader_defs: world.shader_defs(),
                ..Default::default()
            }).unwrap()))
        });

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
        
        println!("{:?}", world.shader_defs());

        let compute_shader = renderer.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute"),
            source: wgpu::ShaderSource::Naga(Cow::Owned(composer.make_naga_module(NagaModuleDescriptor {
                source: include_str!("wgsl/conway_compute.wgsl"),
                file_path: "wgsl/conway_compute.wgsl",
                shader_defs: world.shader_defs(),
                ..Default::default()
            }).unwrap()))
        });

        let compute_bind_group_layout = world.compute_bind_group_layout(&renderer.device);

        let compute_bind_groups = world.compute_bind_groups(&compute_bind_group_layout, &renderer.device);
        
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

        let init_pipeline = renderer.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            cache: None,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            entry_point: Some("init"),
            label: Some("init_pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader
        });

        const DEAD_ZONE: f32 = 0.2;

        let input = HybridInputState::new(
            &[
                (&[InputSource::key(KeyCode::KeyW), InputSource::key(KeyCode::ArrowUp), InputSource::axis(gilrs::Axis::LeftStickY, |a|    (10f32).powf(( a - DEAD_ZONE).max(0.)) - 1.)], InputIdent::Up),
                (&[InputSource::key(KeyCode::KeyS), InputSource::key(KeyCode::ArrowDown), InputSource::axis(gilrs::Axis::LeftStickY, |a|  (10f32).powf((-a - DEAD_ZONE).max(0.)) - 1.)], InputIdent::Down),
                (&[InputSource::key(KeyCode::KeyA), InputSource::key(KeyCode::ArrowLeft), InputSource::axis(gilrs::Axis::LeftStickX, |a|  (10f32).powf((-a - DEAD_ZONE).max(0.)) - 1.)], InputIdent::Left),
                (&[InputSource::key(KeyCode::KeyD), InputSource::key(KeyCode::ArrowRight), InputSource::axis(gilrs::Axis::LeftStickX, |a| (10f32).powf(( a - DEAD_ZONE).max(0.)) - 1.)], InputIdent::Right)
            ],
            &[]
        );
    
        let profiler = Profiler::new(1, 100, &renderer.device, renderer.queue.get_timestamp_period());

        let state = Self {
            compute_bind_groups,
            compute_pipeline,
            init_pipeline,
            fragment_bind_groups,
            frame_polarity: false,
            render_pipeline,
            camera,
            input,
            profiler
        };

        state.init(renderer);

        state
    }

    pub fn init(&self, renderer: &RendererContext) {
        let mut encoder = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Init Encoder")
        });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Init Pass"),
                timestamp_writes: None
            });

            compute_pass.set_pipeline(&self.init_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_groups[1], &[]);
            
            compute_pass.dispatch_workgroups(WORKGROUP_DIMS.x, WORKGROUP_DIMS.y, 1);
        }
        renderer.queue.submit(std::iter::once(encoder.finish()));
        println!("Initialise");
    }

    pub fn render(&mut self, renderer: &RendererContext, view: &TextureView, render_encoder: &mut CommandEncoder) {
        self.camera.update_buffer(&renderer.queue);

        let mut encoder = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder")
        });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: Some(self.profiler.compute_pass_timestamp_writes(0))
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_groups[self.frame_polarity as usize], &[]);
            
            compute_pass.dispatch_workgroups(WORKGROUP_DIMS.x, WORKGROUP_DIMS.y, 1);
        }
        self.profiler.resolve(&mut encoder);
        renderer.queue.submit(std::iter::once(encoder.finish()));
        self.profiler.process_results(&renderer.device);
        
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
            render_pass.set_bind_group(0, &self.fragment_bind_groups[0], &[]);
            render_pass.set_bind_group(1, self.camera.bind_group(), &[]);
            render_pass.draw(0..3, 0..1);
        }
        
        self.frame_polarity = !self.frame_polarity;
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.camera.update_inner(|camera| {
            camera.screen_resolution.x = size.width as f32;
            camera.screen_resolution.y = size.height as f32;
        });
    }
}

struct World {
    textures: [Vec<wgpu::Texture>; 2],
}

impl World {
    fn new(max_texture_size: u32, device: &wgpu::Device) -> Self {
        let grid_dims = uvec2(TEXTURE_SIZE.x.div_ceil(max_texture_size), TEXTURE_SIZE.y.div_ceil(max_texture_size));
        let tiles = grid_dims.element_product();

        let gen_textures = || {
            let mut textures = Vec::with_capacity(tiles as usize);

            for y in 0..grid_dims.y {
                for x in 0..grid_dims.x {
                    let desc = wgpu::TextureDescriptor {
                        label: Some("world_tile"),
                        size: wgpu::Extent3d {
                            width: (TEXTURE_SIZE.x - x * max_texture_size).min(max_texture_size),
                            height: (TEXTURE_SIZE.y - y * max_texture_size).min(max_texture_size),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::R32Uint,
                        usage: wgpu::TextureUsages::STORAGE_BINDING,
                        view_formats: &[],
                    };

                    textures.push(device.create_texture(&desc));
                }
            }
            textures
        };

        Self {
            textures: [gen_textures(), gen_textures()]
        }
    }

    fn fragment_bind_group_layout(&self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2
                    },
                    count: NonZero::new(self.textures[0].len() as u32),
                }
            ]
        })
    }

    fn fragment_bind_groups(&self, layout: &wgpu::BindGroupLayout, device: &wgpu::Device) -> [wgpu::BindGroup; 2] {
        let views = self.textures
            .iter()
            .map(|grid| grid.iter()
                .map(|texture| texture.create_view(&Default::default()))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>();

        let view_refs = views
            .iter()
            .map(|grid| grid
                .iter()
                .collect::<Vec<_>>())
            .collect::<Vec<_>>();

        view_refs.iter()    
            .map(|grid| device.create_bind_group({
                &wgpu::BindGroupDescriptor {
                    label: None,
                    layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureViewArray(&grid)
                    }],
                }
            }))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    fn compute_bind_group_layout(&self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        let texture_count= NonZero::new(self.textures[0].len() as u32);

        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    count: texture_count,
                    ty: wgpu::BindingType::StorageTexture { 
                        access: wgpu::StorageTextureAccess::ReadOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2
                    },
                    visibility: wgpu::ShaderStages::COMPUTE
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    count: texture_count,
                    ty: wgpu::BindingType::StorageTexture { 
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2
                    },
                    visibility: wgpu::ShaderStages::COMPUTE
                },
            ]
        })
    }

    fn compute_bind_groups(&self, layout: &wgpu::BindGroupLayout, device: &wgpu::Device) -> [wgpu::BindGroup; 2] {
        let views = self.textures
            .iter()
            .map(|grid| grid.iter()
                .map(|texture| texture.create_view(&Default::default()))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>();

        let view_refs = views
            .iter()
            .map(|grid| grid
                .iter()
                .collect::<Vec<_>>())
            .collect::<Vec<_>>();


        [[&view_refs[0], &view_refs[1]], [&view_refs[1], &view_refs[0]]]
            .into_iter()
            .map(|[grid_a, grid_b]| device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureViewArray(grid_a)
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureViewArray(grid_b)
                    },
                ],
            }))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    fn shader_defs(&self) -> HashMap<String, naga_oil::compose::ShaderDefValue> {
        let tile_width = self.textures[0][0].width();
        let max_tile_dim = self.textures[0][0].height().max(tile_width);
        HashMap::from([
            ("TILE_SIZE".into(), ShaderDefValue::UInt(max_tile_dim)),
            ("GRID_WIDTH".into(), ShaderDefValue::UInt(TEXTURE_SIZE.x.div_ceil(tile_width))),
            ("GRID_LENGTH".into(), ShaderDefValue::UInt(self.textures[0].len() as u32)),
        ])
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
        
        let limits = adapter.limits();

        // Connection to the device
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES |
                        wgpu::Features::TIMESTAMP_QUERY |
                        wgpu::Features::TEXTURE_BINDING_ARRAY |
                        wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY |
                        wgpu::Features::UNIFORM_BUFFER_AND_STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING,
                    required_limits: wgpu::Limits {
                        max_texture_dimension_2d: WORLD_SIZE.y.min(limits.max_texture_dimension_2d),
                        ..Default::default()
                    },
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
        
        println!("{:?}", adapter.get_info());
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
            limits
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
}

impl AppState {
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.renderer.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut render_encoder = self.renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render encoder")
        });

        self.game_of_life.render(&self.renderer, &view, &mut render_encoder);
        self.gui.draw(&self.renderer, &self.window, |ctx| self.ui_state.draw(ctx), &mut render_encoder, &view);

        self.renderer.queue.submit(core::iter::once(render_encoder.finish()));

        output.present();

        Ok(())
    }
}