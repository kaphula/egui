mod frame_timer;

use std::iter;
use std::sync::Arc;
use std::time::Instant;
use sdl2::{Sdl, VideoSubsystem};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::{Cursor, MouseButton, SystemCursor};
use sdl2::video::Window;
use wgpu::{Backend, Device, Queue, Surface, SurfaceConfiguration};
use core::default::Default;

// use chrono::Timelike;
use egui::{Context, FontDefinitions, FullOutput, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Rgba};
// use egui::CursorIcon::Default;
use egui::mutex::RwLock;
use egui_wgpu::renderer;
use egui_wgpu::renderer::RenderPass;
use crate::frame_timer::FrameTimer;

// use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
const INITIAL_WIDTH: u32 = 800;
const INITIAL_HEIGHT: u32 = 600;

/// A custom event type for the winit app.
// enum Event {
//     RequestRedraw,
// }

/// This is the repaint signal type that egui needs for requesting a repaint from another thread.
/// It sends the custom RequestRedraw event to the winit event loop.
// struct ExampleRepaintSignal(std::sync::Mutex<winit::event_loop::EventLoopProxy<Event>>);
//
// impl epi::backend::RepaintSignal for ExampleRepaintSignal {
//     fn request_repaint(&self) {
//         self.0.lock().unwrap().send_event(Event::RequestRedraw).ok();
//     }
// }

pub struct FusedCursor {
    pub cursor: sdl2::mouse::Cursor,
    pub icon: sdl2::mouse::SystemCursor,
}

impl FusedCursor {
    pub fn new() -> Self {
        Self {
            cursor: sdl2::mouse::Cursor::from_system(sdl2::mouse::SystemCursor::Arrow).unwrap(),
            icon: sdl2::mouse::SystemCursor::Arrow,
        }
    }
}

impl Default for FusedCursor {
    fn default() -> Self {
        Self::new()
    }
}

struct WGPUSDL2 {
    sdl_window: Window,
    surface: Surface,
    device: Device,
    queue: Queue,
    sdl_context: Sdl,
    sdl_video_subsystem: VideoSubsystem,
    surface_config: SurfaceConfiguration,
}

fn init_sdl(width: u32, height: u32) -> WGPUSDL2 {
    let sdl_context = sdl2::init().expect("Cannot initialize SDL2!");
    let video_subsystem = sdl_context.video().expect("Cannot get SDL2 context!");

    //let gl_attr = video_subsystem.gl_attr();
    //gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
    //gl_attr.set_context_version(4, 3);

    let window = video_subsystem
        .window("asd", width, height)
        .position_centered()
        .resizable()
        // .vulkan()
        // .opengl()
        .build()
        .map_err(|e| e.to_string()).expect("Cannot create SDL2 window!");

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
    // let instance = wgpu::Instance::new(wgpu::Backends::GL);
    #[allow(unsafe_code)]
    let surface = unsafe { instance.create_surface(&window) };
    let adapter_opt = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }));
    let adapter = match adapter_opt {
        Some(a) => {
            //let info = &a.get_info();
            //let be = match info.backend {
            //    wgpu::Backend::Empty => { "empty" }
            //    Backend::Vulkan => { "vulkan"}
            //    Backend::Metal => { "metal"}
            //    Backend::Dx12 => { "dx12"}
            //    Backend::Dx11 => { "dx11"}
            //    Backend::Gl => { "gl"}
            //    Backend::BrowserWebGpu => { "browser webgpu"}
            //};
            a
        },
        None => panic!("Failed to find wgpu adapter!") ,
    };

    let (device, queue) = match pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            limits: wgpu::Limits::default(),
            label: Some("device"),
            features: wgpu::Features::empty(),
        },
        None,
    )) {
        Ok(a) => a,
        Err(e) => panic!("{}", e.to_string()),
    };

    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_preferred_format(&adapter).unwrap(),
        width,
        height,
        present_mode: wgpu::PresentMode::Mailbox,
    };
    surface.configure(&device, &config);

    WGPUSDL2 {
        sdl_context: sdl_context,
        sdl_video_subsystem: video_subsystem,
        sdl_window: window,
        surface: surface,
        surface_config: config,
        device: device,
        queue: queue
    }
}

//#[derive(Clone)]
//pub struct RenderState {
//    pub target_format: TextureFormat,
//    pub egui_rpass: Arc<RwLock<renderer::RenderPass>>,
//}

fn paint_and_update_textures(
    wgpu_sdl2_app: &WGPUSDL2,
    egui_rpass: Arc<RwLock<RenderPass>>,
    pixels_per_point: f32,
    clear_color: egui::Rgba,
    clipped_primitives: &[egui::ClippedPrimitive],
    textures_delta: &egui::TexturesDelta,
) {
    let output_frame = match wgpu_sdl2_app.surface.get_current_texture() {
        Ok(frame) => frame,
        Err(wgpu::SurfaceError::Outdated) => {
            // This error occurs when the app is minimized on Windows.
            // Silently return here to prevent spamming the console with:
            // "The underlying surface has changed, and therefore the swap chain must be updated"
            return;
        }
        Err(e) => {
            return;
        }
    };
    let output_view = output_frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = wgpu_sdl2_app.device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });

    // Upload all resources for the GPU.
    let screen_descriptor = renderer::ScreenDescriptor {
        size_in_pixels: [wgpu_sdl2_app.surface_config.width, wgpu_sdl2_app.surface_config.height],
        pixels_per_point,
    };

    {
        let mut rpass = egui_rpass.write();
        for (id, image_delta) in &textures_delta.set {
            rpass.update_texture(&wgpu_sdl2_app.device, &wgpu_sdl2_app.queue, *id, image_delta);
        }

        rpass.update_buffers(
            &wgpu_sdl2_app.device,
            &wgpu_sdl2_app.queue,
            clipped_primitives,
            &screen_descriptor,
        );
    }

    // Record all render passes.
    egui_rpass.read().execute(
        &mut encoder,
        &output_view,
        clipped_primitives,
        &screen_descriptor,
        Some(wgpu::Color {
            r: clear_color.r() as f64,
            g: clear_color.g() as f64,
            b: clear_color.b() as f64,
            a: clear_color.a() as f64,
        }),
    );

    {
        let mut rpass = egui_rpass.write();
        for id in &textures_delta.free {
            rpass.free_texture(id);
        }
    }

    // Submit the commands.
    wgpu_sdl2_app.queue.submit(std::iter::once(encoder.finish()));

    // Redraw egui
    output_frame.present();
}


pub fn input_to_egui(
    window: &sdl2::video::Window,
    event: &sdl2::event::Event,
    egui_sdl2_state: &mut EguiSDL2State,
) {


    fn sdl_button_to_egui(btn: &MouseButton) -> Option<PointerButton> {
        match btn {
            MouseButton::Left => Some(egui::PointerButton::Primary),
            MouseButton::Middle => Some(egui::PointerButton::Middle),
            MouseButton::Right => Some(egui::PointerButton::Secondary),
            _ => None,
        }
    }

    use sdl2::event::Event::*;
    let pixels_per_point = egui_sdl2_state.dpi_scaling;
    if event.get_window_id() != Some(window.id()) {
        return;
    }
    match event {
        // handle when window Resized and SizeChanged.
        Window { win_event, .. } => match win_event {
            WindowEvent::Resized(x, y) | sdl2::event::WindowEvent::SizeChanged(x, y) => {
                // let drs = window.drawable_size(); // ??
                // egui_sdl2_state.update_screen_rect(drs.0, drs.1, &window);
                egui_sdl2_state.update_screen_rect(*x as u32, *y as u32, &window);
            }
            _ => (),
        },
        MouseButtonDown { mouse_btn, .. } => {
            if let Some(pressed) = sdl_button_to_egui(mouse_btn) {
                println!("press event!");
                egui_sdl2_state.raw_input.events.push(egui::Event::PointerButton {
                    pos: egui_sdl2_state.mouse_pointer_position,
                    button: pressed,
                    pressed: true,
                    modifiers: egui_sdl2_state.modifiers,
                });
            }
        }
        MouseButtonUp { mouse_btn, .. } => {
            if let Some(released) = sdl_button_to_egui(mouse_btn) {
                egui_sdl2_state.raw_input.events.push(egui::Event::PointerButton {
                    pos: egui_sdl2_state.mouse_pointer_position,
                    button: released,
                    pressed: false,
                    modifiers: egui_sdl2_state.modifiers,
                });
            }
        }

        MouseMotion { x, y, .. } => {
            egui_sdl2_state.mouse_pointer_position = egui::pos2(*x as f32 / pixels_per_point, *y as f32 / pixels_per_point);
            egui_sdl2_state.raw_input.events.push(egui::Event::PointerMoved(egui_sdl2_state.mouse_pointer_position));
        }

        KeyUp {
            keycode, keymod, ..
        } => {
            let key_code = match keycode {
                Some(key_code) => key_code,
                _ => return,
            };
            let key = match translate_virtual_key_code(*key_code) {
                Some(key) => key,
                _ => return,
            };
            egui_sdl2_state.modifiers = Modifiers {
                alt: (*keymod & Mod::LALTMOD == Mod::LALTMOD)
                    || (*keymod & Mod::RALTMOD == Mod::RALTMOD),
                ctrl: (*keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                    || (*keymod & Mod::RCTRLMOD == Mod::RCTRLMOD),
                shift: (*keymod & Mod::LSHIFTMOD == Mod::LSHIFTMOD)
                    || (*keymod & Mod::RSHIFTMOD == Mod::RSHIFTMOD),
                mac_cmd: *keymod & Mod::LGUIMOD == Mod::LGUIMOD,

                //TOD: Test on both windows and mac
                command: (*keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                    || (*keymod & Mod::LGUIMOD == Mod::LGUIMOD),
            };

            egui_sdl2_state.raw_input.events.push(egui::Event::Key {
                key,
                pressed: false,
                modifiers: egui_sdl2_state.modifiers,
            });
        }

        KeyDown {
            keycode, keymod, ..
        } => {
            let key_code = match keycode {
                Some(key_code) => key_code,
                _ => return,
            };

            let key = match translate_virtual_key_code(*key_code) {
                Some(key) => key,
                _ => return,
            };
            egui_sdl2_state.modifiers = Modifiers {
                alt: (*keymod & Mod::LALTMOD == Mod::LALTMOD)
                    || (*keymod & Mod::RALTMOD == Mod::RALTMOD),
                ctrl: (*keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                    || (*keymod & Mod::RCTRLMOD == Mod::RCTRLMOD),
                shift: (*keymod & Mod::LSHIFTMOD == Mod::LSHIFTMOD)
                    || (*keymod & Mod::RSHIFTMOD == Mod::RSHIFTMOD),
                mac_cmd: *keymod & Mod::LGUIMOD == Mod::LGUIMOD,

                //TOD: Test on both windows and mac
                command: (*keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                    || (*keymod & Mod::LGUIMOD == Mod::LGUIMOD),
            };

            egui_sdl2_state.raw_input.events.push(egui::Event::Key {
                key,
                pressed: true,
                modifiers: egui_sdl2_state.modifiers,
            });

            if egui_sdl2_state.modifiers.command && key == Key::C {
                // println!("copy event");
                egui_sdl2_state.raw_input.events.push(egui::Event::Copy);
            } else if egui_sdl2_state.modifiers.command && key == Key::X {
                // println!("cut event");
                egui_sdl2_state.raw_input.events.push(egui::Event::Cut);
            } else if egui_sdl2_state.modifiers.command && key == Key::V {
                // println!("paste");
                if let Ok(contents) = window.subsystem().clipboard().clipboard_text() {
                    egui_sdl2_state.raw_input.events.push(egui::Event::Text(contents));
                }
            }
        }

        TextInput { text, .. } => {
            egui_sdl2_state.raw_input.events.push(egui::Event::Text(text.clone()));
        }
        MouseWheel { x, y, .. } => {
            let delta = egui::vec2(*x as f32 * 8.0, *y as f32 * 8.0);
            let sdl = window.subsystem().sdl();
            // zoom:
            if sdl.keyboard().mod_state() & Mod::LCTRLMOD == Mod::LCTRLMOD
                || sdl.keyboard().mod_state() & Mod::RCTRLMOD == Mod::RCTRLMOD
            {
                let zoom_delta = (delta.y / 125.0).exp();
                egui_sdl2_state.raw_input.events.push(egui::Event::Zoom(zoom_delta));
            }
            // horizontal scroll:
            else if sdl.keyboard().mod_state() & Mod::LSHIFTMOD == Mod::LSHIFTMOD
                || sdl.keyboard().mod_state() & Mod::RSHIFTMOD == Mod::RSHIFTMOD
            {
                egui_sdl2_state.raw_input.events
                    .push(egui::Event::Scroll(egui::vec2(delta.x + delta.y, 0.0)));
            // regular scroll:
            } else {
                egui_sdl2_state.raw_input.events.push(egui::Event::Scroll(egui::vec2(delta.x, delta.y)));
            }
        }
        _ => {
        }
    }
}

pub fn translate_virtual_key_code(key: sdl2::keyboard::Keycode) -> Option<egui::Key> {
    use Keycode::*;

    Some(match key {
        Left => Key::ArrowLeft,
        Up => Key::ArrowUp,
        Right => Key::ArrowRight,
        Down => Key::ArrowDown,

        Escape => Key::Escape,
        Tab => Key::Tab,
        Backspace => Key::Backspace,
        Space => Key::Space,
        Return => Key::Enter,

        Insert => Key::Insert,
        Home => Key::Home,
        Delete => Key::Delete,
        End => Key::End,
        PageDown => Key::PageDown,
        PageUp => Key::PageUp,

        Kp0 | Num0 => Key::Num0,
        Kp1 | Num1 => Key::Num1,
        Kp2 | Num2 => Key::Num2,
        Kp3 | Num3 => Key::Num3,
        Kp4 | Num4 => Key::Num4,
        Kp5 | Num5 => Key::Num5,
        Kp6 | Num6 => Key::Num6,
        Kp7 | Num7 => Key::Num7,
        Kp8 | Num8 => Key::Num8,
        Kp9 | Num9 => Key::Num9,

        A => Key::A,
        B => Key::B,
        C => Key::C,
        D => Key::D,
        E => Key::E,
        F => Key::F,
        G => Key::G,
        H => Key::H,
        I => Key::I,
        J => Key::J,
        K => Key::K,
        L => Key::L,
        M => Key::M,
        N => Key::N,
        O => Key::O,
        P => Key::P,
        Q => Key::Q,
        R => Key::R,
        S => Key::S,
        T => Key::T,
        U => Key::U,
        V => Key::V,
        W => Key::W,
        X => Key::X,
        Y => Key::Y,
        Z => Key::Z,

        _ => {
            return None;
        }
    })
}

pub struct EguiSDL2State {
    raw_input: RawInput,
    modifiers: Modifiers,
    dpi_scaling: f32,
    default_dpi: f32,
    mouse_pointer_position: egui::Pos2,
    pub fused_cursor: FusedCursor,
}

impl EguiSDL2State {
    fn update_screen_rect(&mut self, width: u32, height: u32, window: &Window) {
        let ddpi = window.subsystem().display_dpi(0).unwrap().0;
        let scale = self.default_dpi / ddpi;
        let rect = (egui::vec2(width as f32, height as f32) / scale) * self.dpi_scaling;
        self.raw_input.screen_rect = Some(Rect::from_min_size(Pos2::new(0f32, 0f32), rect));
    }

    fn update_time(&mut self, running_time: Option<f64>, delta: f32) {
        self.raw_input.time = running_time;
        self.raw_input.predicted_dt = delta;
    }

    fn new(width: u32, height: u32, default_dpi: f32, display_diagonal_dpi: f32, dpi_scaling: f32) -> Self {
        let scale = default_dpi / display_diagonal_dpi;
        let rect = (egui::vec2(width as f32, height as f32) / scale) * dpi_scaling;
        let screen_rect = Rect::from_min_size(Pos2::new(0f32, 0f32), rect);
        let raw_input = RawInput {
            screen_rect: Some(screen_rect),
            pixels_per_point: Some(dpi_scaling),
            ..RawInput::default()
        };

        let modifiers = Modifiers::default();
        EguiSDL2State {
            raw_input: raw_input,
            modifiers: modifiers,
            dpi_scaling: dpi_scaling,
            default_dpi: default_dpi,
            mouse_pointer_position: egui::Pos2::new(0.0,0.0),
            fused_cursor: FusedCursor::new()
        }
    }

    pub fn process_output(&mut self, window: &Window, egui_output: &egui::PlatformOutput) {
        if !egui_output.copied_text.is_empty() {
            let copied_text = egui_output.copied_text.clone();
            {
                let result = window
                    .subsystem()
                    .clipboard()
                    .set_clipboard_text(&copied_text);
                if result.is_err() {
                    dbg!("Unable to set clipboard content to SDL clipboard.");
                }
            }
        }
        EguiSDL2State::translate_cursor(&mut self.fused_cursor, egui_output.cursor_icon);
    }

    fn translate_cursor(fused: &mut FusedCursor, cursor_icon: egui::CursorIcon) {
        let tmp_icon = match cursor_icon {
            egui::CursorIcon::Crosshair => SystemCursor::Crosshair,
            egui::CursorIcon::Default => SystemCursor::Arrow,
            egui::CursorIcon::Grab => SystemCursor::Hand,
            egui::CursorIcon::Grabbing => SystemCursor::SizeAll,
            egui::CursorIcon::Move => SystemCursor::SizeAll,
            egui::CursorIcon::PointingHand => SystemCursor::Hand,
            egui::CursorIcon::ResizeHorizontal => SystemCursor::SizeWE,
            egui::CursorIcon::ResizeNeSw => SystemCursor::SizeNESW,
            egui::CursorIcon::ResizeNwSe => SystemCursor::SizeNWSE,
            egui::CursorIcon::ResizeVertical => SystemCursor::SizeNS,
            egui::CursorIcon::Text => SystemCursor::IBeam,
            egui::CursorIcon::NotAllowed | egui::CursorIcon::NoDrop => SystemCursor::No,
            egui::CursorIcon::Wait => SystemCursor::Wait,
            //There doesn't seem to be a suitable SDL equivalent...
            _ => SystemCursor::Arrow,
        };

        if tmp_icon != fused.icon {
            fused.cursor = Cursor::from_system(tmp_icon).unwrap();
            fused.icon = tmp_icon;
            fused.cursor.set();
        }
    }
}

fn main() {

    let mut sys = init_sdl(INITIAL_WIDTH, INITIAL_HEIGHT);
    let mut event_pump = sys.sdl_context.event_pump().expect("Cannot create SDL2 event pump");

    let mut egui_ctx = egui::Context::default();
    let mut egui_rpass = Arc::new(RwLock::new(RenderPass::new(&sys.device, sys.surface_config.format, 1)));

    let mut frame_timer = FrameTimer::new();

    let ddpi = sys.sdl_window.subsystem().display_dpi(0).unwrap().0;
    let mut egui_sdl2_state = EguiSDL2State::new(INITIAL_WIDTH, INITIAL_HEIGHT, 96.0, ddpi, 1.0);

    let mut running_time: f64 = 0.0;
    let mut checkbox1_checked = false;
    'running: loop {
        frame_timer.time_start();
        let delta = frame_timer.delta();
        running_time += delta as f64;

        egui_sdl2_state.update_time(Some(running_time), delta);

        for event in event_pump.poll_iter() {
            match &event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                Event::Window {
                    window_id,
                    win_event: WindowEvent::SizeChanged(width, height) | WindowEvent::Resized(width, height),
                    ..
                } => {
                    if window_id.clone() == sys.sdl_window.id() {
                        let config = &mut sys.surface_config;
                        config.width = *width as u32;
                        config.height = *height as u32;
                        sys.surface.configure(&sys.device, &config);
                    }
                }


                e => { // dbg!(e); }
                }
            }
            input_to_egui(&sys.sdl_window, &event, &mut egui_sdl2_state)
        }

        // egui_ctx.input().
        let full_output = egui_ctx.run(egui_sdl2_state.raw_input.take(), |ctx| {
            egui::Window::new("Settings").resizable(true).vscroll(true).show(&ctx, |ui| {
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");
                ui.label("Welcome!");

                if ui.button("Press me").clicked() {
                    println!("you pressed me!")
                }
                ui.checkbox(&mut checkbox1_checked, "checkbox1");
                ui.end_row();
            });

        });

        egui_sdl2_state.process_output(&sys.sdl_window, &full_output.platform_output);
        let tris = egui_ctx.tessellate(full_output.shapes);
        if (full_output.needs_repaint) {
            paint_and_update_textures(&sys, egui_rpass.clone(), egui_sdl2_state.dpi_scaling, Rgba::from_rgb(0.0,0.0,0.0), &tris, &full_output.textures_delta)
        }
        frame_timer.time_stop()
    }
}