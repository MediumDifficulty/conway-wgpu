use egui::{Align2, Context};
use egui_wgpu::ScreenDescriptor;
use egui_winit::{winit::{event::WindowEvent, window::Window}, State};
use wgpu::{CommandEncoder, Device, TextureFormat, TextureView};

use crate::{RendererContext, WORLD_SIZE};

// https://github.com/ejb004/egui-wgpu-demo/blob/master/src/gui.rs
pub struct GuiRenderer {
    pub enabled: bool,
    pub context: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer
}

impl GuiRenderer {
    pub fn new(
        device: &Device,
        output_color_format: TextureFormat,
        output_depth_format: Option<TextureFormat>,
        msaa_samples: u32,
        window: &Window
    ) -> Self {
        let egui_context = Context::default();
        let id = egui_context.viewport_id();

        egui_context.set_visuals(egui::Visuals {
            ..Default::default()
        });

        let egui_state = State::new(egui_context.clone(), id, &window, None, None, None);

        let egui_renderer = egui_wgpu::Renderer::new(
            device,
            output_color_format,
            output_depth_format,
            msaa_samples,
            false
        );
        
        Self {
            context: egui_context,
            state: egui_state,
            renderer: egui_renderer,
            enabled: true
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    pub fn draw(
        &mut self,
        renderer: &RendererContext,
        window: &Window,
        mut run_ui: impl FnMut(&Context),
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
    ) {
        if !self.enabled {
            return;
        }

        let raw_input = self.state.take_egui_input(&window);
        let full_output = self.context.run(raw_input, |ui| {
            run_ui(ui);
        });
        
        self.state.handle_platform_output(&window, full_output.platform_output);

        let tris = self.context.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer.update_texture(&renderer.device, &renderer.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            pixels_per_point: window.scale_factor() as f32,
            size_in_pixels: [renderer.config.width, renderer.config.height]
        };

        self.renderer.update_buffers(&renderer.device, &renderer.queue, encoder, &tris, &screen_descriptor);
        
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: egui_wgpu::wgpu::Operations {
                    load: egui_wgpu::wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            label: Some("egui main render pass"),
            occlusion_query_set: None,
        });

        self.renderer.render(&mut render_pass.forget_lifetime(), &tris, &screen_descriptor);
        
        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x);
        }
    }
}

#[derive(Default)]
pub struct UiState {
    pub elapsed_frame_time: f32,
    pub frames: usize
}

impl UiState {
    pub fn draw(&mut self, ctx: &Context) {
        if self.elapsed_frame_time > 1. {
            self.frames = 0;
            self.elapsed_frame_time = 0.;
        }

        egui::Window::new("Settings")
        // .vscroll(true)
        .default_open(true)
        // .min_width(1000.0)
        // .min_height(800.0)
        .default_width(800.0)
        .resizable(true)
        .anchor(Align2::LEFT_TOP, [0.0, 0.0])
        .show(&ctx, |ui| {
            let secs_per_frame = self.elapsed_frame_time / self.frames as f32;
            ui.label(format!("ms / frame: {}ms", ((secs_per_frame * 1000. * 100.).round() / 100.)));
            ui.label(format!("Gc / s: {}", (WORLD_SIZE.element_product() as f64 / secs_per_frame as f64 / 1e9 * 100.).round() / 100.));
            ui.label(format!("ps / cell: {}", (secs_per_frame as f64 / WORLD_SIZE.element_product() as f64 / 1e-12 * 100.).round() / 100.));
            ui.end_row();
        });
    }
}
