use std::iter;
use std::sync::Arc;
use std::time::Instant;
use sdl2::{Sdl, VideoSubsystem};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::Window;
use wgpu::{Backend, Device, Queue, Surface, SurfaceConfiguration};

// use chrono::Timelike;
use egui::{Context, FontDefinitions, FullOutput, Modifiers, Pos2, RawInput, Rect, Rgba};
use egui::CursorIcon::Default;
use egui::mutex::RwLock;
use egui_wgpu::renderer;
use egui_wgpu::renderer::RenderPass;

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
    // surface: &Surface,
    // surface_configuration: &SurfaceConfiguration,
    pixels_per_point: f32,
    clear_color: egui::Rgba,
    clipped_primitives: &[egui::ClippedPrimitive],
    textures_delta: &egui::TexturesDelta,
) {
    //let render_state = match self.render_state.as_mut() {
    //    Some(rs) => rs,
    //    None => return,
    //};
    //let surface_state = match self.surface_state.as_ref() {
    //    Some(rs) => rs,
    //    None => return,
    //};

    let output_frame = match wgpu_sdl2_app.surface.get_current_texture() {
        Ok(frame) => frame,
        Err(wgpu::SurfaceError::Outdated) => {
            // This error occurs when the app is minimized on Windows.
            // Silently return here to prevent spamming the console with:
            // "The underlying surface has changed, and therefore the swap chain must be updated"
            return;
        }
        Err(e) => {
            // tracing::warn!("Dropped frame with error: {e}");
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

/// A simple egui + wgpu + winit based example.
fn main() {

    let sys = init_sdl(INITIAL_WIDTH, INITIAL_HEIGHT);
    let mut event_pump = sys.sdl_context.event_pump().expect("Cannot create SDL2 event pump");

    let mut egui_ctx = egui::Context::default();

    let mut egui_rpass = Arc::new(RwLock::new(RenderPass::new(&sys.device, sys.surface_config.format, 1)));


    //let scale = match scale {
    //    DpiScaling::Default => 96.0 / window.subsystem().display_dpi(0).unwrap().0,
    //    DpiScaling::Custom(custom) => {
    //        (96.0 / window.subsystem().display_dpi(0).unwrap().0) * custom
    //    }
    //};
    let scale = 96.0 / sys.sdl_window.subsystem().display_dpi(0).unwrap().0; // ddpi
    // println!("scale: {}", scale);


    let rect = egui::vec2(INITIAL_WIDTH as f32, INITIAL_HEIGHT as f32) / scale;
    // let rect = egui::vec2(INITIAL_WIDTH as f32, INITIAL_HEIGHT as f32);
    let screen_rect = Rect::from_min_size(Pos2::new(0f32, 0f32), rect);
    let raw = RawInput {
        screen_rect: Some(screen_rect),
        pixels_per_point: Some(1.0),
        ..RawInput::default()
    };

    'running: loop {
        for event in event_pump.poll_iter() {
            match &event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                e => { // dbg!(e); }
                }
            }
        }

        egui_ctx.begin_frame(raw.clone());

        egui::Window::new("Settings").resizable(true).show(&egui_ctx, |ui| {
            ui.label("Welcome!");
        });

        let full_output: FullOutput = egui_ctx.end_frame();

        let tris = egui_ctx.tessellate(full_output.shapes);
        if (full_output.needs_repaint) {
            paint_and_update_textures(&sys, egui_rpass.clone(), raw.pixels_per_point.unwrap(), Rgba::from_rgb(0.0,0.0,0.0), &tris, &full_output.textures_delta)
            //let mut rpass = egui_rpass.write();
            //for (id, image_delta) in &full_output.textures_delta.set {
            //    rpass.update_texture(&wgpu_sdl2_app.device, &wgpu_sdl2_app.queue, *id, image_delta);
            //}
        }



    }

    // EGUI_CONTEXT --> FullOutput -->  egui_rpass.update_texture



    // paint_and_update_textures(&sys, egui_rpass)

    // Display the demo application that ships with egui.
    // let mut demo_app = egui_demo_lib::WrapApp::default();
    //egui_rpass.update_texture(&device, &queue, &platform.context().font_image());
    //egui_rpass.update_user_textures(&device, &queue);
    //egui_rpass.update_buffers(&device, &queue, &paint_jobs, &screen_descriptor);



    /*    let event_loop = winit::event_loop::EventLoop::with_user_event();
        let window = winit::window::WindowBuilder::new()
            .with_decorations(true)
            .with_resizable(true)
            .with_transparent(false)
            .with_title("egui-wgpu_winit example")
            .with_inner_size(winit::dpi::PhysicalSize {
                width: INITIAL_WIDTH,
                height: INITIAL_HEIGHT,
            })
            .build(&event_loop)
            .unwrap();

        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let surface = unsafe { instance.create_surface(&window) };

        // WGPU 0.11+ support force fallback (if HW implementation not supported), set it to true or false (optional).
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
            .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::default(),
                limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ))
            .unwrap();

        let size = window.inner_size();
        let surface_format = surface.get_preferred_format(&adapter).unwrap();
        let mut surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width as u32,
            height: size.height as u32,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &surface_config);

        let repaint_signal = std::sync::Arc::new(ExampleRepaintSignal(std::sync::Mutex::new(
            event_loop.create_proxy(),
        )));

        // We use the egui_winit_platform crate as the platform.
        let mut platform = Platform::new(PlatformDescriptor {
            physical_width: size.width as u32,
            physical_height: size.height as u32,
            scale_factor: window.scale_factor(),
            font_definitions: FontDefinitions::default(),
            style: Default::default(),
        });

        // We use the egui_wgpu_backend crate as the render backend.
        let mut egui_rpass = RenderPass::new(&device, surface_format, 1);

        // Display the demo application that ships with egui.
        let mut demo_app = egui_demo_lib::WrapApp::default();

        let start_time = Instant::now();
        let mut previous_frame_time = None;
        event_loop.run(move |event, _, control_flow| {
            // Pass the winit events to the platform integration.
            platform.handle_event(&event);

            match event {
                RedrawRequested(..) => {
                    platform.update_time(start_time.elapsed().as_secs_f64());

                    let output_frame = match surface.get_current_texture() {
                        Ok(frame) => frame,
                        Err(wgpu::SurfaceError::Outdated) => {
                            // This error occurs when the app is minimized on Windows.
                            // Silently return here to prevent spamming the console with:
                            // "The underlying surface has changed, and therefore the swap chain must be updated"
                            return;
                        }
                        Err(e) => {
                            eprintln!("Dropped frame with error: {}", e);
                            return;
                        }
                    };
                    let output_view = output_frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    // Begin to draw the UI frame.
                    let egui_start = Instant::now();
                    platform.begin_frame();
                    let app_output = epi::backend::AppOutput::default();

                    let mut frame =  epi::Frame::new(epi::backend::FrameData {
                        info: epi::IntegrationInfo {
                            name: "egui_example",
                            web_info: None,
                            cpu_usage: previous_frame_time,
                            native_pixels_per_point: Some(window.scale_factor() as _),
                            prefer_dark_mode: None,
                        },
                        output: app_output,
                        repaint_signal: repaint_signal.clone(),
                    });

                    // Draw the demo application.
                    demo_app.update(&platform.context(), &mut frame);

                    // End the UI frame. We could now handle the output and draw the UI with the backend.
                    let (_output, paint_commands) = platform.end_frame(Some(&window));
                    let paint_jobs = platform.context().tessellate(paint_commands);

                    let frame_time = (Instant::now() - egui_start).as_secs_f64() as f32;
                    previous_frame_time = Some(frame_time);

                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("encoder"),
                    });

                    // Upload all resources for the GPU.
                    let screen_descriptor = ScreenDescriptor {
                        physical_width: surface_config.width,
                        physical_height: surface_config.height,
                        scale_factor: window.scale_factor() as f32,
                    };
                    egui_rpass.update_texture(&device, &queue, &platform.context().font_image());
                    egui_rpass.update_user_textures(&device, &queue);
                    egui_rpass.update_buffers(&device, &queue, &paint_jobs, &screen_descriptor);

                    // Record all render passes.
                    egui_rpass
                        .execute(
                            &mut encoder,
                            &output_view,
                            &paint_jobs,
                            &screen_descriptor,
                            Some(wgpu::Color::BLACK),
                        )
                        .unwrap();
                    // Submit the commands.
                    queue.submit(iter::once(encoder.finish()));

                    // Redraw egui
                    output_frame.present();

                    // Suppport reactive on windows only, but not on linux.
                    // if _output.needs_repaint {
                    //     *control_flow = ControlFlow::Poll;
                    // } else {
                    //     *control_flow = ControlFlow::Wait;
                    // }
                }
                MainEventsCleared | UserEvent(Event::RequestRedraw) => {
                    window.request_redraw();
                }
                WindowEvent { event, .. } => match event {
                    winit::event::WindowEvent::Resized(size) => {
                        // Resize with 0 width and height is used by winit to signal a minimize event on Windows.
                        // See: https://github.com/rust-windowing/winit/issues/208
                        // This solves an issue where the app would panic when minimizing on Windows.
                        if size.width > 0 && size.height > 0 {
                            surface_config.width = size.width;
                            surface_config.height = size.height;
                            surface.configure(&device, &surface_config);
                        }
                    }
                    winit::event::WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => {}
                },
                _ => (),
            }
        });
    */
    }
















// egui_sdl2_gl:


/*

pub struct EguiStateHandler {
    pub fused_cursor: FusedCursor,
    pub pointer_pos: Pos2,
    pub input: RawInput,
    pub modifiers: Modifiers,
    pub native_pixels_per_point: f32,
}

pub fn with_sdl2(
    window: &sdl2::video::Window,
    shader_ver: ShaderVersion,
    scale: DpiScaling,
) -> (EguiStateHandler) {
    let scale = match scale {
        DpiScaling::Default => 96.0 / window.subsystem().display_dpi(0).unwrap().0,
        DpiScaling::Custom(custom) => {
            (96.0 / window.subsystem().display_dpi(0).unwrap().0) * custom
        }
    };
    let painter = painter::Painter::new(window, scale, shader_ver);
    EguiStateHandler::new(painter)
}

impl EguiStateHandler {
    pub fn new(painter: Painter) -> (Painter, EguiStateHandler) {
        let native_pixels_per_point = painter.pixels_per_point;
        let _self = EguiStateHandler {
            fused_cursor: FusedCursor::default(),
            pointer_pos: Pos2::new(0f32, 0f32),
            input: egui::RawInput {
                screen_rect: Some(painter.screen_rect),
                pixels_per_point: Some(native_pixels_per_point),
                ..Default::default()
            },
            modifiers: Modifiers::default(),
            native_pixels_per_point,
        };
        (painter, _self)
    }

    pub fn process_input(
        &mut self,
        window: &sdl2::video::Window,
        event: sdl2::event::Event,
        painter: &mut Painter,
    ) {
        input_to_egui(window, event, painter, self);
    }

    pub fn process_output(&mut self, window: &sdl2::video::Window, egui_output: &egui::Output) {
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
        translate_cursor(&mut self.fused_cursor, egui_output.cursor_icon);
    }
}

pub fn input_to_egui(
    window: &sdl2::video::Window,
    event: sdl2::event::Event,
    painter: &mut Painter,
    state: &mut EguiStateHandler,
) {
    use sdl2::event::Event::*;

    let pixels_per_point = painter.pixels_per_point;
    if event.get_window_id() != Some(window.id()) {
        return;
    }
    match event {
        // handle when window Resized and SizeChanged.
        Window { win_event, .. } => match win_event {
            WindowEvent::Resized(_, _) | sdl2::event::WindowEvent::SizeChanged(_, _) => {
                painter.update_screen_rect(window.drawable_size());
                state.input.screen_rect = Some(painter.screen_rect);
            }
            _ => (),
        },

        //MouseButonLeft pressed is the only one needed by egui
        MouseButtonDown { mouse_btn, .. } => {
            let mouse_btn = match mouse_btn {
                MouseButton::Left => Some(egui::PointerButton::Primary),
                MouseButton::Middle => Some(egui::PointerButton::Middle),
                MouseButton::Right => Some(egui::PointerButton::Secondary),
                _ => None,
            };
            if let Some(pressed) = mouse_btn {
                state.input.events.push(egui::Event::PointerButton {
                    pos: state.pointer_pos,
                    button: pressed,
                    pressed: true,
                    modifiers: state.modifiers,
                });
            }
        }

        //MouseButonLeft pressed is the only one needed by egui
        MouseButtonUp { mouse_btn, .. } => {
            let mouse_btn = match mouse_btn {
                MouseButton::Left => Some(egui::PointerButton::Primary),
                MouseButton::Middle => Some(egui::PointerButton::Middle),
                MouseButton::Right => Some(egui::PointerButton::Secondary),
                _ => None,
            };
            if let Some(released) = mouse_btn {
                state.input.events.push(egui::Event::PointerButton {
                    pos: state.pointer_pos,
                    button: released,
                    pressed: false,
                    modifiers: state.modifiers,
                });
            }
        }

        MouseMotion { x, y, .. } => {
            state.pointer_pos = pos2(x as f32 / pixels_per_point, y as f32 / pixels_per_point);
            state
                .input
                .events
                .push(egui::Event::PointerMoved(state.pointer_pos));
        }

        KeyUp {
            keycode, keymod, ..
        } => {
            let key_code = match keycode {
                Some(key_code) => key_code,
                _ => return,
            };
            let key = match translate_virtual_key_code(key_code) {
                Some(key) => key,
                _ => return,
            };
            state.modifiers = Modifiers {
                alt: (keymod & Mod::LALTMOD == Mod::LALTMOD)
                    || (keymod & Mod::RALTMOD == Mod::RALTMOD),
                ctrl: (keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                    || (keymod & Mod::RCTRLMOD == Mod::RCTRLMOD),
                shift: (keymod & Mod::LSHIFTMOD == Mod::LSHIFTMOD)
                    || (keymod & Mod::RSHIFTMOD == Mod::RSHIFTMOD),
                mac_cmd: keymod & Mod::LGUIMOD == Mod::LGUIMOD,

                //TOD: Test on both windows and mac
                command: (keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                    || (keymod & Mod::LGUIMOD == Mod::LGUIMOD),
            };

            state.input.events.push(Event::Key {
                key,
                pressed: false,
                modifiers: state.modifiers,
            });
        }

        KeyDown {
            keycode, keymod, ..
        } => {
            let key_code = match keycode {
                Some(key_code) => key_code,
                _ => return,
            };

            let key = match translate_virtual_key_code(key_code) {
                Some(key) => key,
                _ => return,
            };
            state.modifiers = Modifiers {
                alt: (keymod & Mod::LALTMOD == Mod::LALTMOD)
                    || (keymod & Mod::RALTMOD == Mod::RALTMOD),
                ctrl: (keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                    || (keymod & Mod::RCTRLMOD == Mod::RCTRLMOD),
                shift: (keymod & Mod::LSHIFTMOD == Mod::LSHIFTMOD)
                    || (keymod & Mod::RSHIFTMOD == Mod::RSHIFTMOD),
                mac_cmd: keymod & Mod::LGUIMOD == Mod::LGUIMOD,

                //TOD: Test on both windows and mac
                command: (keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                    || (keymod & Mod::LGUIMOD == Mod::LGUIMOD),
            };

            state.input.events.push(Event::Key {
                key,
                pressed: true,
                modifiers: state.modifiers,
            });

            if state.modifiers.command && key == Key::C {
                // println!("copy event");
                state.input.events.push(Event::Copy);
            } else if state.modifiers.command && key == Key::X {
                // println!("cut event");
                state.input.events.push(Event::Cut);
            } else if state.modifiers.command && key == Key::V {
                // println!("paste");
                if let Ok(contents) = window.subsystem().clipboard().clipboard_text() {
                    state.input.events.push(Event::Text(contents));
                }
            }
        }

        TextInput { text, .. } => {
            state.input.events.push(Event::Text(text));
        }

        MouseWheel { x, y, .. } => {
            let delta = vec2(x as f32 * 8.0, y as f32 * 8.0);
            let sdl = window.subsystem().sdl();
            if sdl.keyboard().mod_state() & Mod::LCTRLMOD == Mod::LCTRLMOD
                || sdl.keyboard().mod_state() & Mod::RCTRLMOD == Mod::RCTRLMOD
            {
                state.input.zoom_delta *= (delta.y / 125.0).exp();
            } else {
                state.input.scroll_delta = delta;
            }
        }

        _ => {
            //dbg!(event);
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

pub fn translate_cursor(fused: &mut FusedCursor, cursor_icon: egui::CursorIcon) {
    let tmp_icon = match cursor_icon {
        CursorIcon::Crosshair => SystemCursor::Crosshair,
        CursorIcon::Default => SystemCursor::Arrow,
        CursorIcon::Grab => SystemCursor::Hand,
        CursorIcon::Grabbing => SystemCursor::SizeAll,
        CursorIcon::Move => SystemCursor::SizeAll,
        CursorIcon::PointingHand => SystemCursor::Hand,
        CursorIcon::ResizeHorizontal => SystemCursor::SizeWE,
        CursorIcon::ResizeNeSw => SystemCursor::SizeNESW,
        CursorIcon::ResizeNwSe => SystemCursor::SizeNWSE,
        CursorIcon::ResizeVertical => SystemCursor::SizeNS,
        CursorIcon::Text => SystemCursor::IBeam,
        CursorIcon::NotAllowed | CursorIcon::NoDrop => SystemCursor::No,
        CursorIcon::Wait => SystemCursor::Wait,
        //There doesn't seem to be a suitable SDL equivalent...
        _ => SystemCursor::Arrow,
    };

    if tmp_icon != fused.icon {
        fused.cursor = Cursor::from_system(tmp_icon).unwrap();
        fused.icon = tmp_icon;
        fused.cursor.set();
    }
}
*/