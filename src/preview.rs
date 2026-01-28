use chrono::{Datelike, Timelike};
use image::{ImageReader, RgbaImage};
use softbuffer::{Context, Surface};
use std::{
    io::Cursor, num::NonZeroU32, rc::Rc, sync::{Arc, Mutex}, time::{Duration, Instant}
};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, OwnedDisplayHandle},
    platform::windows::{CornerPreference, WindowAttributesExtWindows},
    window::{Icon, Window, WindowLevel},
};

use crate::AppEvent;

pub struct ViewState {
    pub scale: f32,    // 缩放比例
    pub offset_x: f32, // X偏移
    pub offset_y: f32, // Y偏移

    // 拖拽状态
    pub dragging: bool,    // 是否正在拖拽
    pub last_mouse_x: f32, // 上次鼠标X位置
    pub last_mouse_y: f32, // 上次鼠标Y位置
}

pub struct ClickState {
    pub last_click: Option<Instant>,
}

pub struct PreviewWindow {
    pub preview_win: Option<Rc<Window>>,
    pub image: Option<image::RgbaImage>,
    pub buffer: Option<Vec<u32>>,

    pub surface: Option<Surface<OwnedDisplayHandle, Rc<Window>>>,
    pub context: Option<Context<OwnedDisplayHandle>>,

    pub view_state: ViewState,
    pub click_state: ClickState,

    is_pinned: Arc<Mutex<bool>>,
}

impl PreviewWindow {
    pub fn new(image: RgbaImage) -> Self {
        Self {
            preview_win: None,
            image: Some(image),
            buffer: None,

            surface: None,
            context: None,

            view_state: ViewState {
                scale: 1.0,
                offset_x: 0.0,
                offset_y: 0.0,

                dragging: false,
                last_mouse_x: 0.0,
                last_mouse_y: 0.0,
            },

            click_state: ClickState { last_click: None },

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
        _proxy_event: Arc<winit::event_loop::EventLoopProxy<AppEvent>>,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                // 处理重绘请求

                let Some(surface) = &mut self.surface else {
                    return;
                };

                let w_width = self.image.as_ref().unwrap().width();
                let w_height = self.image.as_ref().unwrap().height();
                let width = self.image.as_ref().unwrap().width();
                let height = self.image.as_ref().unwrap().height();

                let mut buffer = surface.buffer_mut().unwrap();

                if let Some(buffer_data) = &self.buffer {
                    for sy in 0..w_height {
                        for sx in 0..w_width {
                            let ix = (sx as f32 - self.view_state.offset_x) / self.view_state.scale;
                            let iy = (sy as f32 - self.view_state.offset_y) / self.view_state.scale;
                            if ix >= 0.0 && iy >= 0.0 && ix < width as f32 && iy < height as f32 {
                                let px = ix as u32;
                                let py = iy as u32;

                                buffer[(sy * w_width + sx) as usize] =
                                    buffer_data[(py * width + px) as usize];
                            } else {
                                buffer[(sy * w_width + sx) as usize] = 0x00202020; // 背景色
                            }
                        }
                    }

                    // 缓存数据存在，直接使用
                    // buffer.copy_from_slice(buffer_data.as_slice());
                } else {
                    for (x, y, pixel) in self.image.as_ref().unwrap().enumerate_pixels() {
                        let red = pixel.0[0] as u32;
                        let green = pixel.0[1] as u32;
                        let blue = pixel.0[2] as u32;
                        let color = blue | (green << 8) | (red << 16);
                        buffer[(y * width + x) as usize] = color;
                    }
                }

                // 绘制 buffer 内容
                buffer.present().unwrap();
            }

            WindowEvent::Focused(_) => {
                // 处理窗口聚焦事件
            }

            WindowEvent::MouseInput { state, button, .. } => {
                // 处理鼠标输入事件

                // 处理 preview 窗口的鼠标输入事件
                match button {
                    MouseButton::Right => {
                        match state {
                            ElementState::Pressed => {
                                // 右键点击 pin preview 窗口
                                println!("Right mouse button clicked, pin preview.");
                                self.pin_window();
                            }

                            _ => {}
                        }
                    }

                    MouseButton::Left => {
                        match state {
                            ElementState::Pressed => {
                                self.view_state.dragging = true;
                            }
                            ElementState::Released => {
                                self.view_state.dragging = false;

                                let now = Instant::now();
                                let double_click_threshold = Duration::from_millis(300);

                                if let Some(last) = self.click_state.last_click {
                                    if now.duration_since(last) < double_click_threshold {
                                        // 双击
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
                    }
                    _ => {}
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                // 处理鼠标移动事件

                // 处理 preview 窗口的鼠标移动事件
                let x = position.x as f32;
                let y = position.y as f32;

                if self.view_state.dragging {
                    let dx = x - self.view_state.last_mouse_x;
                    let dy = y - self.view_state.last_mouse_y;

                    self.view_state.offset_x += dx;
                    self.view_state.offset_y += dy;
                }

                self.view_state.last_mouse_x = x;
                self.view_state.last_mouse_y = y;

                self.preview_win.as_ref().unwrap().request_redraw();
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

    pub fn pin_window(&self) {
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
            }
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
