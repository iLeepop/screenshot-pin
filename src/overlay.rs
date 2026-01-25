use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use winit::{
    application::ApplicationHandler, dpi::{PhysicalPosition, PhysicalSize}, event::{ElementState, Event, MouseButton, WindowEvent}, event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy, OwnedDisplayHandle}, window::{Window, WindowAttributes, WindowLevel, Fullscreen}
};
use softbuffer::{Context, Surface};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{sync::{Arc, Mutex}, rc::Rc, num::NonZeroU32};

use crate::AppEvent;

pub struct OverlayWindow {
    pub overlay_win: Option<Rc<Window>>,

    pub surface: Option<Surface<OwnedDisplayHandle, Rc<Window>>>,
    pub context: Option<Context<OwnedDisplayHandle>>,

    // pub image: Arc<Mutex<Option<ImageBuffer<Rgba<u8>, Vec<u8>>>>>,
    pub image: Option<image::RgbaImage>,
    pub buffer: Option<Vec<u32>>,
    pub dimmed_buffer: Option<Vec<u32>>,

    pub selection: Arc<Mutex<Option<(i32, i32, i32, i32)>>>, // (x1, y1, x2, y2)
    pub is_selecting: Arc<Mutex<bool>>,
    pub start_pos: Arc<Mutex<Option<(i32, i32)>>>,
}

impl OverlayWindow {
    pub fn new() -> Self {

        Self {
            overlay_win: None,

            surface: None,
            context: None,

            image:None,

            buffer: None,
            dimmed_buffer: None,

            selection: Arc::new(Mutex::new(None)),
            is_selecting: Arc::new(Mutex::new(false)),
            start_pos: Arc::new(Mutex::new(None)),
        }
    }
    
    pub fn show(&mut self, event_loop: &ActiveEventLoop, image: image::RgbaImage) -> Result<(), Box<dyn std::error::Error>> {

        let monitor = event_loop.primary_monitor().unwrap();
        let size = monitor.size();
        let position = monitor.position();

        let window_attributes = Window::default_attributes()
            .with_decorations(false)
            .with_transparent(false)
            // .with_window_level(WindowLevel::AlwaysOnTop)
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .with_inner_size(size)
            .with_position(position)
            .with_visible(false)
            .with_resizable(false);

        let w = event_loop.create_window(window_attributes).expect("create overlay window");

        self.overlay_win = Some(Rc::new(w));

        self.context = Some(Context::new(event_loop.owned_display_handle()).unwrap());
        let mut surface = Surface::new(self.context.as_ref().unwrap(), self.overlay_win.as_ref().unwrap().clone()).unwrap();

        self.image = Some(image.clone());

        surface
            .resize(NonZeroU32::new(image.width()).unwrap(), NonZeroU32::new(image.height()).unwrap())
            .unwrap();

        let mut buffer = surface.buffer_mut().unwrap();
        // let width = image.width();

        // for (x, y, pixel) in image.enumerate_pixels() {
        //     let red = pixel.0[0] as u32;
        //     let green = pixel.0[1] as u32;
        //     let blue = pixel.0[2] as u32;

        //     let color = blue | (green << 8) | (red << 16);
        //     buffer[(y * width + x) as usize] = color;
        // }

        // self.buffer = Some(buffer.to_vec());

        // 构建原始和暗化缓冲区
        self.build_buffers();

        // 先使用暗化缓冲区填充
        buffer.copy_from_slice(self.dimmed_buffer.as_ref().unwrap());

        // 绘制 buffer 内容
        buffer.present().unwrap();

        self.surface = Some(surface);

        self.overlay_win.as_ref().unwrap().set_visible(true);

        println!("overlay window created.");
        
        Ok(())
    }

    // pub fn set_image_buffer(&mut self, buffer: ImageBuffer<Rgba<u8>, Vec<u8>>) {
    //     if let Ok(mut img) = self.image.lock() {
    //         *img = Some(buffer);
    //     }

    //     // 请求重绘窗口
    //     if let Some(win) = &self.overlay_win {
    //         win.request_redraw();
    //     }
        
    // }
    
    pub fn get_selection(&self) -> Option<(i32, i32, i32, i32)> {
        self.selection.lock().unwrap().clone()
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        // 处理窗口事件
        match event {
            AppEvent::Reset => {
                println!("OverlayWindow received Reset event");
            }
            _ => {}
        }
    }

    fn build_buffers(&mut self) {
        let img = self.image.as_ref().unwrap();
        let w = img.width();
        let h = img.height();

        self.buffer = Some(vec![0; (w * h) as usize]);
        self.dimmed_buffer = Some(vec![0; (w * h) as usize]);

        let factor = 0.5f32;

        for (x, y, p) in img.enumerate_pixels() {
            let r = p[0] as u32;
            let g = p[1] as u32;
            let b = p[2] as u32;

            let idx = (y * w + x) as usize;

            // 原始
            self.buffer.as_mut().unwrap()[idx] =
                b | (g << 8) | (r << 16);

            // 暗化
            let dr = (r as f32 * factor) as u32;
            let dg = (g as f32 * factor) as u32;
            let db = (b as f32 * factor) as u32;

            self.dimmed_buffer.as_mut().unwrap()[idx] =
                db | (dg << 8) | (dr << 16);
        }
    }

}
