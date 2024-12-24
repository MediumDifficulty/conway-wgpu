use egui::{Align2, Context};
use egui_wgpu::ScreenDescriptor;
use egui_winit::{winit::{event::WindowEvent, window::Window}, State};
use wgpu::{CommandEncoder, Device, Queue, SurfaceConfiguration, TextureFormat, TextureView};

// https://github.com/ejb004/egui-wgpu-demo/blob/master/src/gui.rs
pub struct GuiRenderer {
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
            renderer: egui_renderer
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.state.on_window_event(window, event);
    }

    pub fn draw(
        &mut self,
        window: &Window,
        device: &Device,
        queue: &Queue,
        config: &SurfaceConfiguration,
        mut run_ui: impl FnMut(&Context),
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
    ) {
        let raw_input = self.state.take_egui_input(&window);
        let full_output = self.context.run(raw_input, |ui| {
            run_ui(ui);
        });
        
        self.state.handle_platform_output(&window, full_output.platform_output);

        let tris = self.context.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            pixels_per_point: window.scale_factor() as f32,
            size_in_pixels: [config.width, config.height]
        };

        self.renderer.update_buffers(device, queue, encoder, &tris, &screen_descriptor);
        
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

pub fn gui(ui: &Context) {
    egui::Window::new("Conway")
        // .vscroll(true)
        .default_open(true)
        .max_width(1000.0)
        .max_height(800.0)
        .default_width(800.0)
        .resizable(true)
        .anchor(Align2::LEFT_TOP, [0.0, 0.0])
        .show(&ui, |ui| {
            if ui.add(egui::Button::new("Click me")).clicked() {
                println!("PRESSED")
            }
            let mut a = 0;
            ui.label("Slider");
            ui.add(egui::Slider::new(&mut a, 0..=120).text("age"));
            ui.label(format!("{a}"));
            ui.end_row();

            // proto_scene.egui(ui);
        });
}