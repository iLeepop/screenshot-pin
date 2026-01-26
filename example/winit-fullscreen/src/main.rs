use std::{num::NonZeroU32, rc::Rc};

use image::GenericImageView;
use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    event::{StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop, OwnedDisplayHandle},
    keyboard::{Key, NamedKey},
    window::{Fullscreen, Window},
};

struct FullscreenApp {
    window: Option<Rc<Window>>,
    image: Option<image::DynamicImage>,
    surface: Option<Surface<OwnedDisplayHandle, Rc<Window>>>,
    context: Option<Context<OwnedDisplayHandle>>,
}

impl FullscreenApp {
    fn new() -> Self {
        Self {
            window: None,
            image: None,
            surface: None,
            context: None,
        }
    }
}

impl ApplicationHandler for FullscreenApp {
    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        match cause {
            StartCause::Init => {
                let window_attributes = Window::default_attributes()
                    .with_decorations(false)
                    .with_resizable(false)
                    .with_fullscreen(Some(Fullscreen::Borderless(None)))
                    .with_visible(false)
                    .with_transparent(false);

                let w = event_loop
                    .create_window(window_attributes)
                    .expect("create fullscreen window");
                self.window = Some(Rc::new(w));

                // 创建 window、surface 和 context
                // link: https://github.com/rust-windowing/softbuffer/blob/master/examples/fruit.rs
                self.context = Some(Context::new(event_loop.owned_display_handle()).unwrap());
                let mut surface = Surface::new(
                    self.context.as_ref().unwrap(),
                    self.window.as_ref().unwrap().clone(),
                )
                .unwrap();

                let img = image::load_from_memory(include_bytes!("../img/overlay.png")).unwrap();

                self.image = Some(img.clone());

                surface
                    .resize(
                        NonZeroU32::new(img.width()).unwrap(),
                        NonZeroU32::new(img.height()).unwrap(),
                    )
                    .unwrap();

                let mut buffer = surface.buffer_mut().unwrap();
                let width = self.image.as_ref().unwrap().width();

                for (x, y, pixel) in self.image.as_ref().unwrap().pixels() {
                    let red = pixel.0[0] as u32;
                    let green = pixel.0[1] as u32;
                    let blue = pixel.0[2] as u32;

                    let color = blue | (green << 8) | (red << 16);
                    buffer[(y * width + x) as usize] = color;
                }

                // 绘制 buffer 内容
                buffer.present().unwrap();

                self.surface = Some(surface);

                // 在 windows 上，窗口初始化会具有白色背景并没有绘制任何内容，必须等待第三方图形库来绘制
                // 然后再显示窗口，才能避免闪烁
                // link: https://github.com/vulkano-rs/vulkano/issues/2263
                self.window.as_ref().unwrap().set_visible(true);

                println!("Fullscreen window created.");
            }

            _ => {}
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(win) = &self.window {
                    if win.id() == window_id {
                        println!("Fullscreen window requested close.");
                        self.window = None;
                        event_loop.exit();
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // 处理键盘输入事件
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        // 按下 Esc 键时关闭全屏窗口
                        println!("Escape pressed, closing fullscreen window.");
                        event_loop.exit();
                    }
                    _ => {}
                }
            }

            WindowEvent::RedrawRequested => {
                let Some(surface) = &mut self.surface else {
                    return;
                };

                let mut buffer = surface.buffer_mut().unwrap();
                let width = self.image.as_ref().unwrap().width();

                for (x, y, pixel) in self.image.as_ref().unwrap().pixels() {
                    let red = pixel.0[0] as u32;
                    let green = pixel.0[1] as u32;
                    let blue = pixel.0[2] as u32;

                    let color = blue | (green << 8) | (red << 16);
                    buffer[(y * width + x) as usize] = color;
                }

                // 绘制 buffer 内容
                buffer.present().unwrap();
            }

            // 解决窗口获得焦点时显示 titlebar 的问题
            // link: https://github.com/rust-windowing/winit/issues/3698
            WindowEvent::Focused(_) => {
                self.window.as_ref().unwrap().request_redraw();
            }

            _ => {}
        }
    }

    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}
}

fn main() {
    let mut app = FullscreenApp::new();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    event_loop.run_app(&mut app).unwrap();
}
