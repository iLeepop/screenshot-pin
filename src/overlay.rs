use image::GenericImageView;
use softbuffer::{Context, Surface};
use std::{
    num::NonZeroU32,
    rc::Rc,
    sync::{Arc, Mutex},
};
use winit::{
    event::{MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoopProxy, OwnedDisplayHandle},
    window::{Fullscreen, Window},
};

use crate::AppEvent;

pub struct OverlayWindow {
    pub overlay_win: Option<Rc<Window>>,

    pub surface: Option<Surface<OwnedDisplayHandle, Rc<Window>>>,
    pub context: Option<Context<OwnedDisplayHandle>>,

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

            image: None,

            buffer: None,
            dimmed_buffer: None,

            selection: Arc::new(Mutex::new(None)),
            is_selecting: Arc::new(Mutex::new(false)),
            start_pos: Arc::new(Mutex::new(None)),
        }
    }

    pub fn show(
        &mut self,
        event_loop: &ActiveEventLoop,
        image: image::RgbaImage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let monitor = event_loop.primary_monitor().unwrap();
        let size = monitor.size();
        let position = monitor.position();

        let window_attributes = Window::default_attributes()
            .with_decorations(false)
            .with_transparent(false)
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .with_inner_size(size)
            .with_position(position)
            .with_visible(false)
            .with_resizable(false);

        let w = event_loop
            .create_window(window_attributes)
            .expect("create overlay window");

        self.overlay_win = Some(Rc::new(w));

        self.context = Some(Context::new(event_loop.owned_display_handle()).unwrap());
        let mut surface = Surface::new(
            self.context.as_ref().unwrap(),
            self.overlay_win.as_ref().unwrap().clone(),
        )
        .unwrap();

        self.image = Some(image.clone());

        surface
            .resize(
                NonZeroU32::new(image.width()).unwrap(),
                NonZeroU32::new(image.height()).unwrap(),
            )
            .unwrap();

        let mut buffer = surface.buffer_mut().unwrap();

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

    pub fn handle_window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
        proxy_event: Arc<EventLoopProxy<AppEvent>>,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                // 处理重绘请求
                let Some(surface) = &mut self.surface else {
                    return;
                };

                let mut buffer = surface.buffer_mut().unwrap();
                let width = self.image.as_ref().unwrap().width();
                let height = self.image.as_ref().unwrap().height();

                // 绘制暗化缓冲区
                if let Some(buffer_data) = &self.dimmed_buffer {
                    // 缓存数据存在，直接使用
                    buffer.copy_from_slice(buffer_data.as_slice());
                } else {
                    for (x, y, pixel) in self.image.as_ref().unwrap().enumerate_pixels() {
                        let red = pixel.0[0] as u32;
                        let green = pixel.0[1] as u32;
                        let blue = pixel.0[2] as u32;
                        let color = blue | (green << 8) | (red << 16);
                        buffer[(y * width + x) as usize] = color;
                    }
                }

                // 绘制选择区域-正常缓冲区
                if let Ok(selection) = self.selection.lock() {
                    let Some((x1, y1, x2, y2)) = *selection else {
                        // 无选择区域，直接绘制整个缓冲区
                        buffer.present().unwrap();
                        return;
                    };
                    let l = x1.min(x2).max(0) as u32;
                    let r = x1.max(x2).min(width as i32 - 1) as u32;
                    let t = y1.min(y2).max(0) as u32;
                    let b = y1.max(y2).min(height as i32 - 1) as u32;

                    for y in t..=b {
                        let row = (y * width) as usize;
                        let src = &self.buffer.as_ref().unwrap()[row..row + width as usize];
                        let dst = &mut buffer[row..row + width as usize];

                        dst[l as usize..=r as usize].copy_from_slice(&src[l as usize..=r as usize]);
                    }
                }

                // 绘制选择矩形边框
                let mut rect = None;
                if let Ok(selection) = self.selection.lock() {
                    rect = selection.map(|(x1, y1, x2, y2)| {
                        let left = x1.min(x2).max(0);
                        let right = x1.max(x2).max(0);
                        let top = y1.min(y2).max(0);
                        let bottom = y1.max(y2).max(0);
                        (left, top, right, bottom)
                    });
                }

                if let Some((l, t, r, b)) = rect {
                    let color = 0x00FFFFFFu32; // 白色（softbuffer: 0x00RRGGBB）

                    // 上下边
                    for x in l..=r {
                        if x >= 0 && x < width as i32 {
                            if t >= 0 && t < height as i32 {
                                buffer[(t as u32 * width + x as u32) as usize] = color;
                            }
                            if b >= 0 && b < height as i32 {
                                buffer[(b as u32 * width + x as u32) as usize] = color;
                            }
                        }
                    }

                    // 左右边
                    for y in t..=b {
                        if y >= 0 && y < height as i32 {
                            if l >= 0 && l < width as i32 {
                                buffer[(y as u32 * width + l as u32) as usize] = color;
                            }
                            if r >= 0 && r < width as i32 {
                                buffer[(y as u32 * width + r as u32) as usize] = color;
                            }
                        }
                    }
                }

                // 绘制 buffer 内容
                buffer.present().unwrap();
            }

            WindowEvent::Focused(_) => {
                // 处理窗口聚焦事件
                println!("Overlay window focused.");
                if let Some(overlay_win) = &self.overlay_win {
                    overlay_win.request_redraw();
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                // 处理鼠标输入事件
                // 绘制矩形选择区域s
                // 这里可以根据鼠标输入更新选择区域
                // 例如，开始选择、结束选择等
                match button {
                    MouseButton::Left => {
                        match state {
                            winit::event::ElementState::Pressed => {
                                // 开始选择
                                let mut is_selecting = self.is_selecting.lock().unwrap();
                                *is_selecting = true;
                                println!("Started selecting.");
                            }
                            winit::event::ElementState::Released => {
                                // 结束选择
                                let mut is_selecting = self.is_selecting.lock().unwrap();
                                *is_selecting = false;
                                println!("Finished selecting.");

                                // 获取选择区域
                                let selection = self.get_selection();
                                println!("Selected area: {:?}", selection);

                                let pos = if let Some((x1, y1, x2, y2)) = selection {
                                    let left = x1.min(x2);
                                    let top = y1.min(y2);
                                    (left, top)
                                } else {
                                    (100, 100)
                                };

                                // 获取选择区域的图像数据
                                let crop_img = if let Some((x1, y1, x2, y2)) = selection {
                                    let img = self.image.as_ref().unwrap();
                                    let (left, top) = (x1.min(x2) as u32, y1.min(y2) as u32);
                                    let (right, bottom) = (x1.max(x2) as u32, y1.max(y2) as u32);
                                    let width = right - left;
                                    let height = bottom - top;

                                    Some(img.view(left, top, width, height).to_image())
                                } else {
                                    None
                                };

                                if let Err(_) = proxy_event.send_event(AppEvent::Reset) {
                                    eprintln!("Failed to send Reset event");
                                }

                                if let Some(cropped_image) = crop_img {
                                    // 发送显示预览窗口事件
                                    if let Err(_) = proxy_event.send_event(AppEvent::ShowPreview {
                                        cropped_image,
                                        position: pos,
                                    }) {
                                        eprintln!("Failed to send ShowPreview event");
                                    }
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                // 处理鼠标移动事件
                // println!("Mouse moved in overlay window: {:?}, {:?}", device_id, position);
                // 如果正在选择，更新选择区域
                let (x, y) = (position.x as i32, position.y as i32);
                let is_selecting = self.is_selecting.lock().unwrap();
                if *is_selecting {
                    // 更新选择区域逻辑
                    // 这里可以根据当前鼠标位置更新选择矩形
                    if let Ok(mut selection) = self.selection.lock() {
                        // 更新结束坐标
                        if let Ok(start_pos) = self.start_pos.lock() {
                            if let Some((start_x, start_y)) = *start_pos {
                                *selection = Some((start_x, start_y, x, y));
                            }
                        }
                    }
                    self.overlay_win.as_ref().unwrap().request_redraw();
                } else {
                    // 未选择状态
                    if let Ok(mut start_pos) = self.start_pos.lock() {
                        *start_pos = Some((x, y));
                    }
                }
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
            self.buffer.as_mut().unwrap()[idx] = b | (g << 8) | (r << 16);

            // 暗化
            let dr = (r as f32 * factor) as u32;
            let dg = (g as f32 * factor) as u32;
            let db = (b as f32 * factor) as u32;

            self.dimmed_buffer.as_mut().unwrap()[idx] = db | (dg << 8) | (dr << 16);
        }
    }
}
