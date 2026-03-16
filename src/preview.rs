use chrono::{Datelike, Timelike};
use image::{ImageReader, RgbaImage};
use softbuffer::{Context, Surface};
use std::{
    io::Cursor,
    num::NonZeroU32,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, OwnedDisplayHandle},
    keyboard::{Key, NamedKey},
    platform::windows::{CornerPreference, WindowAttributesExtWindows},
    window::{Icon, Window, WindowLevel},
};

use crate::AppEvent;

pub struct ViewState {
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,

    pub dragging: bool,
    pub last_mouse_x: f32,
    pub last_mouse_y: f32,

    pub render_mode: RenderMode,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RenderMode {
    Point,
    Fill,
}

pub struct ClickState {
    pub last_click: Option<Instant>,
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

pub struct DrawState {
    pub is_drawing: bool,
    pub is_dragging: bool,
    pub start_x: i32,
    pub start_y: i32,
    pub current_x: i32,
    pub current_y: i32,
}

pub struct PreviewWindow {
    pub preview_win: Option<Rc<Window>>,
    pub image: Option<image::RgbaImage>,
    pub buffer: Option<Vec<u32>>,
    base_title: String,

    pub surface: Option<Surface<OwnedDisplayHandle, Rc<Window>>>,
    pub context: Option<Context<OwnedDisplayHandle>>,

    pub view_state: ViewState,
    pub click_state: ClickState,
    pub draw_state: DrawState,

    is_pinned: Arc<Mutex<bool>>,
}

impl PreviewWindow {
    pub fn new(image: RgbaImage) -> Self {
        let base_title = timestamp_title();
        Self {
            preview_win: None,
            image: Some(image),
            buffer: None,
            base_title,

            surface: None,
            context: None,

            view_state: ViewState {
                scale: 1.0,
                offset_x: 0.0,
                offset_y: 0.0,

                dragging: false,
                last_mouse_x: 0.0,
                last_mouse_y: 0.0,

                render_mode: RenderMode::Fill,
            },

            click_state: ClickState { last_click: None },

            draw_state: DrawState {
                is_drawing: false,
                is_dragging: false,
                start_x: 0,
                start_y: 0,
                current_x: 0,
                current_y: 0,
            },

            is_pinned: Arc::new(Mutex::new(false)),
        }
    }

    pub fn show(
        &mut self,
        event_loop: &ActiveEventLoop,
        pos: (i32, i32),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let time_title = timestamp_title();

        let width = self.image.as_ref().unwrap().width();
        let height = self.image.as_ref().unwrap().height();

        let window_attributes = Window::default_attributes()
            .with_decorations(true)
            .with_corner_preference(CornerPreference::RoundSmall)
            .with_title(time_title)
            .with_window_icon(Some(load_window_icon()))
            .with_taskbar_icon(Some(load_window_icon()))
            .with_transparent(false)
            .with_inner_size(PhysicalSize::new(width, height))
            .with_position(PhysicalPosition::new(pos.0, pos.1))
            .with_visible(false)
            .with_resizable(true);

        let w = event_loop
            .create_window(window_attributes)
            .expect("create preview window");

        // 设置图片视图属性
        let w_width = w.inner_size().width as f32;
        let w_height = w.inner_size().height as f32;

        self.view_state.scale = 1.0;
        self.view_state.offset_x = (w_width - width as f32) * 0.5;
        self.view_state.offset_y = (w_height - height as f32) * 0.5;

        self.preview_win = Some(Rc::new(w));

        self.context = Some(Context::new(event_loop.owned_display_handle()).unwrap());
        let mut surface = Surface::new(
            self.context.as_ref().unwrap(),
            self.preview_win.as_ref().unwrap().clone(),
        )
        .unwrap();

        surface
            .resize(
                NonZeroU32::new(self.image.as_ref().unwrap().width()).unwrap(),
                NonZeroU32::new(self.image.as_ref().unwrap().height()).unwrap(),
            )
            .unwrap();

        let mut buffer = surface.buffer_mut().unwrap();
        for (x, y, pixel) in self.image.as_ref().unwrap().enumerate_pixels() {
            let red = pixel.0[0] as u32;
            let green = pixel.0[1] as u32;
            let blue = pixel.0[2] as u32;

            let color = blue | (green << 8) | (red << 16);
            buffer[(y * width + x) as usize] = color;
        }

        self.buffer = Some(buffer.to_vec());

        // 绘制 buffer 内容
        buffer.present().unwrap();

        self.surface = Some(surface);

        self.preview_win.as_ref().unwrap().set_visible(true);

        println!("preview window created.");

        Ok(())
    }

    pub fn handle_window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
        _: Arc<winit::event_loop::EventLoopProxy<AppEvent>>,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                // 处理重绘请求

                let Some(surface) = &mut self.surface else {
                    return;
                };

                let Some(preview_win) = &self.preview_win else {
                    return;
                };

                let win_width = preview_win.inner_size().width;
                let win_height = preview_win.inner_size().height;

                let img_width = self.image.as_ref().unwrap().width();
                let img_height = self.image.as_ref().unwrap().height();

                // 窗口最小化时尺寸为0，跳过处理
                if win_width == 0 || win_height == 0 {
                    return;
                }

                if let Err(e) = surface.resize(
                    NonZeroU32::new(win_width).unwrap(),
                    NonZeroU32::new(win_height).unwrap(),
                ) {
                    eprintln!("Failed to resize surface: {}", e);
                    return;
                }

                let mut buffer = surface.buffer_mut().unwrap();

                let scale = self.view_state.scale;
                let offset_x = self.view_state.offset_x;
                let offset_y = self.view_state.offset_y;

                // 计算窗口对应的图像可见范围（视口裁剪优化）
                let view_x1 = (0.0 - offset_x) / scale;
                let view_y1 = (0.0 - offset_y) / scale;
                let view_x2 = (win_width as f32 - offset_x) / scale;
                let view_y2 = (win_height as f32 - offset_y) / scale;

                // 裁剪到图像边界
                let start_px = (view_x1.max(0.0) as u32).min(img_width);
                let start_py = (view_y1.max(0.0) as u32).min(img_height);
                let end_px = (view_x2.ceil() as u32).min(img_width);
                let end_py = (view_y2.ceil() as u32).min(img_height);

                if let Some(buffer_data) = &self.buffer {
                    let bg_color: u32 = 0x00202020;
                    buffer.fill(bg_color);

                    let render_mode = self.view_state.render_mode;

                    if render_mode == RenderMode::Point {
                        for py in start_py..end_py {
                            for px in start_px..end_px {
                                let sx = (px as f32 * scale + offset_x) as u32;
                                let sy = (py as f32 * scale + offset_y) as u32;

                                if sx < win_width && sy < win_height {
                                    buffer[(sy * win_width + sx) as usize] =
                                        buffer_data[(py * img_width + px) as usize];
                                }
                            }
                        }
                    } else {
                        for dest_y in 0..win_height {
                            for dest_x in 0..win_width {
                                let src_x = (dest_x as f32 - offset_x) / scale;
                                let src_y = (dest_y as f32 - offset_y) / scale;

                                if src_x >= 0.0
                                    && src_x < img_width as f32
                                    && src_y >= 0.0
                                    && src_y < img_height as f32
                                {
                                    let px = src_x as u32;
                                    let py = src_y as u32;
                                    buffer[(dest_y * win_width + dest_x) as usize] =
                                        buffer_data[(py * img_width + px) as usize];
                                }
                            }
                        }
                    }

                    // 渲染正在绘制的矩形（仅预览）
                    if self.draw_state.is_drawing && self.draw_state.is_dragging {
                        let current_rect = Rect {
                            x1: self.draw_state.start_x.min(self.draw_state.current_x),
                            y1: self.draw_state.start_y.min(self.draw_state.current_y),
                            x2: self.draw_state.start_x.max(self.draw_state.current_x),
                            y2: self.draw_state.start_y.max(self.draw_state.current_y),
                        };
                        draw_rectangle(
                            &mut buffer,
                            win_width,
                            win_height,
                            &current_rect,
                            0x0000FF00,
                        );
                    }
                }

                // 绘制 buffer 内容
                buffer.present().unwrap();
            }

            WindowEvent::Focused(_) => {
                // 处理窗口聚焦事件
            }

            // 处理键盘输入事件
            WindowEvent::KeyboardInput { event, .. } => {
                // 只处理按键按下事件，忽略重复事件
                if event.state != ElementState::Pressed || event.repeat {
                    return;
                }
                // 处理键盘输入事件
                match event.logical_key.as_ref() {
                    // 关闭预览窗口
                    Key::Named(NamedKey::Escape) => {
                        let wid = _window_id;
                        println!(
                            "[Preview {:?}] ESC pressed (repeat: {}), will send ClosePreview",
                            wid, event.repeat
                        );
                        // if let Err(e) =
                        //     proxy_event.send_event(AppEvent::ClosePreview { window_id: wid })
                        // {
                        //     eprintln!("Failed to send ClosePreview event: {}", e);
                        // }
                    }
                    // 关闭预览窗口
                    Key::Character("q") => {
                        let wid = _window_id;
                        println!(
                            "[Preview {:?}] q pressed (repeat: {}), will send ClosePreview",
                            wid, event.repeat
                        );
                        // if let Err(e) =
                        //     proxy_event.send_event(AppEvent::ClosePreview { window_id: wid })
                        // {
                        //     eprintln!("Failed to send ClosePreview event: {}", e);
                        // }
                    }
                    // 保存到剪切板
                    Key::Character("s") => {
                        println!("save to clipboard");
                        if let Some(image) = &self.image {
                            if let Err(_) = copy_image_to_clipboard(
                                image.to_vec(),
                                image.width(),
                                image.height(),
                            ) {
                                println!("save to clipboard failed.");
                            }
                        }
                    }
                    // 切换绘制模式
                    Key::Character("r") => {
                        self.draw_state.is_drawing = !self.draw_state.is_drawing;
                        println!(
                            "Drawing mode: {}",
                            if self.draw_state.is_drawing {
                                "ON"
                            } else {
                                "OFF"
                            }
                        );
                        self.preview_win.as_ref().unwrap().request_redraw();
                        self.update_title();
                    }
                    _ => {}
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                // 在绘制模式下，左键用于绘制矩形
                if self.draw_state.is_drawing {
                    match button {
                        MouseButton::Left => {
                            if state == ElementState::Pressed {
                                self.draw_state.is_dragging = true;
                                self.draw_state.start_x = self.draw_state.current_x;
                                self.draw_state.start_y = self.draw_state.current_y;
                            } else if state == ElementState::Released && self.draw_state.is_dragging
                            {
                                self.draw_state.is_dragging = false;

                                let x1 = self.draw_state.start_x.min(self.draw_state.current_x);
                                let y1 = self.draw_state.start_y.min(self.draw_state.current_y);
                                let x2 = self.draw_state.start_x.max(self.draw_state.current_x);
                                let y2 = self.draw_state.start_y.max(self.draw_state.current_y);

                                if x2 > x1 && y2 > y1 {
                                    if let Some(image) = &mut self.image {
                                        let scale = self.view_state.scale;
                                        let offset_x = self.view_state.offset_x;
                                        let offset_y = self.view_state.offset_y;

                                        let img_x1 = ((x1 as f32 - offset_x) / scale) as i32;
                                        let img_y1 = ((y1 as f32 - offset_y) / scale) as i32;
                                        let img_x2 = ((x2 as f32 - offset_x) / scale) as i32;
                                        let img_y2 = ((y2 as f32 - offset_y) / scale) as i32;

                                        let img_width = image.width() as i32;
                                        let img_height = image.height() as i32;

                                        if img_x1 >= img_width
                                            || img_y1 >= img_height
                                            || img_x2 <= 0
                                            || img_y2 <= 0
                                        {
                                            println!("Rectangle is outside image bounds, skipping");
                                        } else {
                                            let cx1 = img_x1.max(0);
                                            let cy1 = img_y1.max(0);
                                            let cx2 = img_x2.min(img_width);
                                            let cy2 = img_y2.min(img_height);

                                            for px in cx1..cx2 {
                                                if cy1 >= 0 && cy1 < img_height {
                                                    image.put_pixel(
                                                        px as u32,
                                                        cy1 as u32,
                                                        image::Rgba([255, 0, 0, 255]),
                                                    );
                                                }
                                                if cy2 > 0 && cy2 - 1 < img_height {
                                                    image.put_pixel(
                                                        px as u32,
                                                        (cy2 - 1) as u32,
                                                        image::Rgba([255, 0, 0, 255]),
                                                    );
                                                }
                                            }
                                            for py in cy1..cy2 {
                                                if cx1 >= 0 && cx1 < img_width {
                                                    image.put_pixel(
                                                        cx1 as u32,
                                                        py as u32,
                                                        image::Rgba([255, 0, 0, 255]),
                                                    );
                                                }
                                                if cx2 > 0 && cx2 - 1 < img_width {
                                                    image.put_pixel(
                                                        (cx2 - 1) as u32,
                                                        py as u32,
                                                        image::Rgba([255, 0, 0, 255]),
                                                    );
                                                }
                                            }
                                            println!(
                                                "Rectangle written to image: ({}, {}) - ({}, {})",
                                                cx1, cy1, cx2, cy2
                                            );
                                        }
                                    }
                                    self.regenerate_buffer();
                                }
                                self.preview_win.as_ref().unwrap().request_redraw();
                            }
                        }

                        MouseButton::Middle => {
                            if state == ElementState::Pressed {
                                self.view_state.dragging = true;
                                self.view_state.render_mode = RenderMode::Point;
                            } else if state == ElementState::Released {
                                self.view_state.dragging = false;
                                self.view_state.render_mode = RenderMode::Fill;
                            }
                        }
                        _ => {}
                    }
                } else {
                    // 非绘制模式下的鼠标处理
                    match button {
                        MouseButton::Right => {
                            if state == ElementState::Pressed {
                                println!("Right mouse button clicked, pin preview.");
                                self.pin_window();
                            }
                        }

                        MouseButton::Left => {
                            if state == ElementState::Pressed {
                                // 左键拖动窗口
                                if let Some(win) = &self.preview_win {
                                    let _ = win.drag_window();
                                }
                            } else if state == ElementState::Released {
                                let now = Instant::now();
                                let double_click_threshold = Duration::from_millis(300);

                                if let Some(last) = self.click_state.last_click {
                                    if now.duration_since(last) < double_click_threshold {
                                        self.reset_view();
                                        self.click_state.last_click = None;
                                    } else {
                                        self.click_state.last_click = Some(now);
                                    }
                                } else {
                                    self.click_state.last_click = Some(now);
                                }
                            }
                        }

                        MouseButton::Middle => {
                            if state == ElementState::Pressed {
                                self.view_state.dragging = true;
                                self.view_state.render_mode = RenderMode::Point;
                            } else if state == ElementState::Released {
                                self.view_state.dragging = false;
                                self.view_state.render_mode = RenderMode::Fill;
                            }
                        }
                        _ => {}
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                // 处理鼠标移动事件
                let x = position.x as f32;
                let y = position.y as f32;

                if self.view_state.dragging {
                    let dx = x - self.view_state.last_mouse_x;
                    let dy = y - self.view_state.last_mouse_y;

                    self.view_state.offset_x += dx;
                    self.view_state.offset_y += dy;
                }

                // 更新绘制模式下的鼠标位置
                if self.draw_state.is_drawing {
                    self.draw_state.current_x = x as i32;
                    self.draw_state.current_y = y as i32;
                    // 绘制模式下：中键拖拽视图 或 左键拖拽矩形时都重绘
                    if self.draw_state.is_dragging || self.view_state.dragging {
                        self.preview_win.as_ref().unwrap().request_redraw();
                    }
                }

                self.view_state.last_mouse_x = x;
                self.view_state.last_mouse_y = y;

                // 非绘制模式时始终重绘
                if !self.draw_state.is_drawing {
                    self.preview_win.as_ref().unwrap().request_redraw();
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                // 处理鼠标滚轮事件
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        self.zoom_at_center(
                            y,
                            self.preview_win.as_ref().unwrap().inner_size().width as f32,
                            self.preview_win.as_ref().unwrap().inner_size().height as f32,
                        );
                        self.preview_win.as_ref().unwrap().request_redraw();
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        let y = pos.y as f32;
                        self.zoom_at_center(
                            y,
                            self.preview_win.as_ref().unwrap().inner_size().width as f32,
                            self.preview_win.as_ref().unwrap().inner_size().height as f32,
                        );
                        self.preview_win.as_ref().unwrap().request_redraw();
                    }
                }
            }

            WindowEvent::Resized(size) => {
                // 处理窗口大小调整事件
                self.handle_resize(size);
                if let Some(preview_win) = &self.preview_win {
                    preview_win.request_redraw();
                }
            }

            _ => {}
        }
    }

    // 保存图像
    #[allow(dead_code)]
    fn save_image(image: &Arc<Mutex<RgbaImage>>) -> Result<(), Box<dyn std::error::Error>> {
        use std::time::SystemTime;

        let image = image.lock().unwrap();

        // 生成文件名
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();

        let filename = format!("screenshot_{}.png", timestamp);
        let path = std::env::current_dir()?.join(filename);

        // 保存图像
        image.save(&path)?;

        println!("Image saved to: {:?}", path);

        // 尝试打开文件管理器
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            if let Err(e) = Command::new("explorer")
                .args(["/select,", &path.to_string_lossy()])
                .spawn()
            {
                println!("Failed to open explorer: {}", e);
            }
        }

        Ok(())
    }

    pub fn pin_window(&mut self) {
        if let Some(win) = &self.preview_win {
            if let Ok(mut is_pinned) = self.is_pinned.lock() {
                if *is_pinned {
                    win.set_window_level(WindowLevel::Normal);
                    *is_pinned = false;
                    println!("preview unpinned");
                } else {
                    win.set_window_level(WindowLevel::AlwaysOnTop);
                    *is_pinned = true;
                    println!("preview pinned on top");
                }
            } else {
                return;
            };

            // 更新标题
            self.update_title();
        }
    }

    fn update_title(&mut self) {
        if let Some(win) = &self.preview_win {
            let mut title = self.base_title.clone();
            if self.draw_state.is_drawing {
                title.push_str(" [draw-mode]");
            }
            if let Ok(is_pinned) = self.is_pinned.lock() {
                if *is_pinned {
                    title.push_str(" [PIN]");
                }
            }
            let _ = win.set_title(&title);
        }
    }

    pub fn zoom_at_center(&mut self, zoom_delta: f32, win_w: f32, win_h: f32) {
        let old_scale = self.view_state.scale;

        // 缩放速度
        let factor = 1.1f32;
        let new_scale = if zoom_delta > 0.0 {
            old_scale * factor
        } else {
            old_scale / factor
        }
        .clamp(0.1, 10.0);

        // 窗口中心
        let cx = win_w * 0.5;
        let cy = win_h * 0.5;

        // 保持中心不动的关键公式
        self.view_state.offset_x = cx - (cx - self.view_state.offset_x) * (new_scale / old_scale);
        self.view_state.offset_y = cy - (cy - self.view_state.offset_y) * (new_scale / old_scale);

        self.view_state.scale = new_scale;
    }

    pub fn reset_view(&mut self) {
        let win_w = self.preview_win.as_ref().unwrap().inner_size().width as f32;
        let win_h = self.preview_win.as_ref().unwrap().inner_size().height as f32;
        let img_w = self.image.as_ref().unwrap().width() as f32;
        let img_h = self.image.as_ref().unwrap().height() as f32;

        self.view_state.scale = 1.0;
        self.view_state.offset_x = (win_w - img_w) * 0.5;
        self.view_state.offset_y = (win_h - img_h) * 0.5;
    }

    pub fn handle_resize(&mut self, new_size: PhysicalSize<u32>) {
        let img_w = self.image.as_ref().unwrap().width() as f32;
        let img_h = self.image.as_ref().unwrap().height() as f32;

        self.view_state.offset_x = (new_size.width as f32 - img_w * self.view_state.scale) * 0.5;
        self.view_state.offset_y = (new_size.height as f32 - img_h * self.view_state.scale) * 0.5;
    }

    fn regenerate_buffer(&mut self) {
        let img = self.image.as_ref().unwrap();
        let img_width = img.width();
        let img_height = img.height();

        let mut new_buffer = vec![0u32; (img_width * img_height) as usize];
        for (x, y, pixel) in img.enumerate_pixels() {
            let red = pixel.0[0] as u32;
            let green = pixel.0[1] as u32;
            let blue = pixel.0[2] as u32;
            let color = blue | (green << 8) | (red << 16);
            new_buffer[(y * img_width + x) as usize] = color;
        }
        self.buffer = Some(new_buffer);
    }
}

pub fn timestamp_title() -> String {
    use chrono::Local;
    let now = Local::now();

    let timestamp = format!(
        "{}/{}/{}/{:02}:{:02}:{:02}",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );

    timestamp
}

fn load_window_icon() -> Icon {
    // 编译期嵌入
    const ICON_BYTES: &[u8] = include_bytes!("../asset/logo_s.ico");

    // 使用 image 解码 ico
    let image = ImageReader::new(Cursor::new(ICON_BYTES))
        .with_guessed_format()
        .expect("guess image format failed")
        .decode()
        .expect("decode ico failed")
        .to_rgba8();

    let (width, height) = image.dimensions();
    let rgba = image.into_raw();

    Icon::from_rgba(rgba, width, height).expect("create tray icon failed")
}

// 拷贝图片到剪贴板
fn copy_image_to_clipboard(
    rgba: Vec<u8>,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    use arboard::{Clipboard, ImageData};
    use std::borrow::Cow;

    let mut clipboard = Clipboard::new()?;

    let image = ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(rgba),
    };

    clipboard.set_image(image)?;
    Ok(())
}

fn draw_rectangle(buffer: &mut [u32], win_width: u32, win_height: u32, rect: &Rect, color: u32) {
    let x1 = rect.x1.max(0) as u32;
    let y1 = rect.y1.max(0) as u32;
    let x2 = rect.x2.min(win_width as i32) as u32;
    let y2 = rect.y2.min(win_height as i32) as u32;

    // 绘制水平线（上下边）
    for x in x1..x2 {
        if y1 < win_height {
            buffer[(y1 * win_width + x) as usize] = color;
        }
        if y2 > 0 && y2 - 1 < win_height {
            buffer[((y2 - 1) * win_width + x) as usize] = color;
        }
    }

    // 绘制垂直线（左右边）
    for y in y1..y2 {
        if x1 < win_width {
            buffer[(y * win_width + x1) as usize] = color;
        }
        if x2 > 0 && x2 - 1 < win_width {
            buffer[(y * win_width + (x2 - 1)) as usize] = color;
        }
    }
}
