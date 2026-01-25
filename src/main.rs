mod capture;
mod overlay;
mod preview;

use capture::*;
use image::{GenericImage, GenericImageView};
use overlay::OverlayWindow;
use preview::PreviewWindow;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{EventLoop, EventLoopProxy};
use winit::keyboard::{Key, KeyCode, NamedKey};
use winit::window::WindowId;

use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// 定义应用程序状态
#[derive(Clone)]
struct AppState {
    // 截图缓存
    screenshot: Arc<Mutex<Option<image::RgbaImage>>>,
    // 热键状态
    hotkey_pressed: Arc<Mutex<bool>>,
    // 矩形选择区域
    selection: Arc<Mutex<Option<(i32, i32, i32, i32)>>>, // (x1, y1, x2, y2)
}

impl AppState {
    fn new() -> Self {
        Self {
            screenshot: Arc::new(Mutex::new(None)),
            hotkey_pressed: Arc::new(Mutex::new(false)),
            selection: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Debug, Clone)]
enum AppEvent {
    StartCapture,

    ShowOverlay {
        screenshot: image::RgbaImage,
    },

    ShowPreview {
        cropped_image: image::RgbaImage,
        position: (i32, i32),
    },

    Reset,
}

// 应用程序管理器
struct ScreenshotApp {
    state: AppState,
    // 窗口管理器
    windows: HashMap<String, winit::window::Window>,

    // overlay 窗口管理器
    overlay_window: Option<OverlayWindow>,
    // overlay 覆盖 窗口ID
    overlay_window_id: Option<winit::window::WindowId>,

    // preview 预览 窗口管理器
    preview_window: Option<PreviewWindow>,
    // preview 预览 窗口ID
    preview_window_id: Option<winit::window::WindowId>,

    preview_windows: HashMap<WindowId, PreviewWindow>,

    ctrl_pressed: Arc<Mutex<bool>>,
    shift_pressed: Arc<Mutex<bool>>,

    // 事件代理
    event_proxy: Arc<EventLoopProxy<AppEvent>>,
}

impl ScreenshotApp {
    fn new(event_proxy: EventLoopProxy<AppEvent>) -> Self {
        Self {
            state: AppState::new(),
            windows: HashMap::new(),

            overlay_window: None,
            preview_window: None,

            overlay_window_id: None,
            preview_window_id: None,

            preview_windows: HashMap::new(),

            ctrl_pressed: Arc::new(Mutex::new(false)),
            shift_pressed: Arc::new(Mutex::new(false)),

            event_proxy: Arc::new(event_proxy),
        }
    }

    fn run(&mut self) {
        // 启动事件循环
    }
    
    // 监听热键
     fn start_hotkey_listener(&self) {
        let hotkey_pressed = self.state.hotkey_pressed.clone();
        let ctrl_pressed = self.ctrl_pressed.clone();
        let shift_pressed = self.shift_pressed.clone();

        let screenshot_state = self.state.screenshot.clone();

        let proxy = self.event_proxy.clone();
        
        std::thread::spawn(move || {
            use rdev::{listen, Event, EventType, Key};
            
            listen(move |event: Event| {
                match event.event_type {
                    EventType::KeyPress(key) => {
                        match key {
                            Key::Alt => {
                                if let Ok(mut ctrl) = ctrl_pressed.lock() {
                                    *ctrl = true;
                                }
                            }
                            Key::ShiftLeft | Key::ShiftRight => {
                                if let Ok(mut shift) = shift_pressed.lock() {
                                    *shift = true;
                                }
                            }
                            Key::KeyM => {
                                // 检查是否同时按下了 Ctrl 和 Shift
                                let ctrl = ctrl_pressed.lock().map(|c| *c).unwrap_or(false);
                                // let shift = shift_pressed.lock().map(|s| *s).unwrap_or(false);
                                
                                if ctrl {
                                    println!("Ctrl+Shift+S pressed!");
                                    if let Ok(mut pressed) = hotkey_pressed.lock() {
                                        *pressed = true;
                                    }

                                    {
                                        println!("Capturing screenshot...");
        
                                        // 1. 捕获所有显示器（合并截图）
                                        let screenshot = capture_fullscreen().unwrap();
                                        println!("Screenshot captured.");
                                        
                                        // 将截图保存到状态
                                        // 截图数据正常
                                        screenshot_state.lock().unwrap().replace(screenshot.clone());
                                        println!("Screenshot stored in state.");

                                        // screenshot_state.lock().unwrap().as_ref().unwrap().save("./new.png").unwrap();
                                        // println!("Screenshot saved.");

                                        proxy.send_event(AppEvent::ShowOverlay { screenshot }).unwrap();
                                    }
                                }
                            }
                            
                            Key::Escape => {
                                // 按下 Esc 键时关闭overlay窗口
                                println!("Escape pressed, closing overlay if exists.");
                                proxy.send_event(AppEvent::Reset).unwrap();
                            }
                            _ => {}
                        }
                    }
                    EventType::KeyRelease(key) => {
                        match key {
                            Key::Alt => {
                                if let Ok(mut ctrl) = ctrl_pressed.lock() {
                                    *ctrl = false;
                                }
                            }
                            Key::ShiftLeft | Key::ShiftRight => {
                                if let Ok(mut shift) = shift_pressed.lock() {
                                    *shift = false;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }).unwrap();
        });
    }
    
    // 检查热键是否被按下
    fn check_hotkey(&self) -> bool {
        if let Ok(mut pressed) = self.state.hotkey_pressed.lock() {
            let was_pressed = *pressed;
            if was_pressed {
                *pressed = false;
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

// 在 ScreenshotApp 中添加方法
impl ScreenshotApp {

    fn proxy_send_event(&self, event: AppEvent) {
        self.event_proxy.send_event(event).unwrap();
    }

    fn show_overlay(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, screenshot: image::RgbaImage) -> Result<(), Box<dyn std::error::Error>> {
        let mut overlay = OverlayWindow::new();
        // overlay.set_image_buffer(screenshot);
        overlay.show(event_loop, screenshot)?;
        self.overlay_window = Some(overlay);
        self.overlay_window_id = Some(self.overlay_window.as_ref().unwrap().overlay_win.as_ref().unwrap().id());
        Ok(())
    }

    fn close_overlay(&mut self) {
        if let Some(overlay) = &mut self.overlay_window {
            overlay.handle_event(AppEvent::Reset);
        }
        self.overlay_window = None;
        self.overlay_window_id = None;
        println!("Overlay window closed.");
    }

    fn create_preview(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, image: image::RgbaImage, pos: (i32, i32)) -> Result<(), Box<dyn std::error::Error>> {
        let mut preview = PreviewWindow::new(image);
        preview.show(event_loop, pos)?;
        // self.preview_window = Some(preview);
        // self.preview_window_id = Some(self.preview_window.as_ref().unwrap().preview_win.as_ref().unwrap().id());

        let window_id = preview.preview_win.as_ref().unwrap().id();
        self.preview_windows.insert(window_id, preview);

        Ok(())
    }

    fn destory_preview(&mut self, wid: WindowId) {

        println!("Destroying preview window: {:?}", wid);

        self.preview_windows.remove(&wid);

        println!("Preview window closed.");
    }

}

// 添加winit ApplicationHandler
impl ApplicationHandler<AppEvent> for ScreenshotApp {
    // 新事件循环开始
    fn new_events(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, cause: winit::event::StartCause) {
        
    }

    // 窗口事件分发
    fn window_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: winit::event::WindowEvent,
        ) {
        // 匹配操作
        match event {
            WindowEvent::CloseRequested => {
                if Some(window_id) == self.overlay_window_id {
                    self.close_overlay();
                }

                if let Some(_) = self.preview_windows.get(&window_id) {
                    self.destory_preview(window_id);
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // 处理键盘输入事件
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        // 发送重置事件以关闭 overlay 窗口
                        // 这里假设有一个事件代理 proxy 可用
                        // proxy.send_event(AppEvent::Reset).unwrap();

                        if Some(window_id) == self.overlay_window_id {
                            println!("Escape key pressed, closing overlay if exists.");
                            // 在overlay窗口中按下Esc键，关闭overlay
                            self.close_overlay();
                        }

                        if let Some(_) = self.preview_windows.get(&window_id) {
                            self.destory_preview(window_id);
                        }
                    }
                    _ => {}
                }
            }

            WindowEvent::RedrawRequested => {
                // 处理重绘请求
                if Some(window_id) == self.overlay_window_id {
                    if let Some(overlay) = &mut self.overlay_window {
                        let Some(surface) = &mut overlay.surface else {
                            return;
                        };

                        let mut buffer = surface.buffer_mut().unwrap();
                        let width = overlay.image.as_ref().unwrap().width();
                        let height = overlay.image.as_ref().unwrap().height();

                        // 绘制暗化缓冲区
                        if let Some(buffer_data) = &overlay.dimmed_buffer {
                            // 缓存数据存在，直接使用
                            buffer.copy_from_slice(buffer_data.as_slice());
                        } else {
                            for (x, y, pixel) in overlay.image.as_ref().unwrap().enumerate_pixels() {
                                let red = pixel.0[0] as u32;
                                let green = pixel.0[1] as u32;
                                let blue = pixel.0[2] as u32;
                                let color = blue | (green << 8) | (red << 16);
                                buffer[(y * width + x) as usize] = color;
                            }
                        }

                        // 绘制选择区域-正常缓冲区
                        if let Ok(selection) = overlay.selection.lock() {
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
                                let src = &overlay.buffer.as_ref().unwrap()[row..row + width as usize];
                                let dst = &mut buffer[row..row + width as usize];

                                dst[l as usize..=r as usize]
                                    .copy_from_slice(&src[l as usize..=r as usize]);
                            }
                        }

                        // 绘制选择矩形边框
                        let mut rect = None;
                        if let Ok(selection) = overlay.selection.lock() {
                            rect = selection.map(|(x1, y1, x2, y2)| {
                                let left   = x1.min(x2).max(0);
                                let right  = x1.max(x2).max(0);
                                let top    = y1.min(y2).max(0);
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
                }

                if let Some(preview) = self.preview_windows.get_mut(&window_id) {
                    let Some(surface) = &mut preview.surface else {
                        return;
                    };

                    let w_width = preview.image.as_ref().unwrap().width();
                    let w_height = preview.image.as_ref().unwrap().height();
                    let width = preview.image.as_ref().unwrap().width();
                    let height = preview.image.as_ref().unwrap().height();

                    let mut buffer = surface.buffer_mut().unwrap();

                    if let Some(buffer_data) = &preview.buffer {

                        for sy in 0..w_height {
                            for sx in 0..w_width {
                                let ix = (sx as f32 - preview.view_state.offset_x) / preview.view_state.scale;
                                let iy = (sy as f32 - preview.view_state.offset_y) / preview.view_state.scale;

                                if ix >= 0.0 && iy >= 0.0 &&
                                ix < width as f32 && iy < height as f32 {

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
                        for (x, y, pixel) in preview.image.as_ref().unwrap().enumerate_pixels() {
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
            }

            WindowEvent::Focused(_) => {
                // 处理窗口聚焦事件
                if Some(window_id) == self.overlay_window_id {
                    println!("Overlay window focused.");
                    println!("{:?},{:?}", window_id , self.overlay_window_id.unwrap());
                    self.overlay_window.as_ref().unwrap().overlay_win.as_ref().unwrap().request_redraw();
                }
            }

            WindowEvent::MouseInput { device_id, state, button } => {
                // 处理鼠标输入事件
                if Some(window_id) == self.overlay_window_id {
                    // 绘制矩形选择区域s
                    // 这里可以根据鼠标输入更新选择区域
                    // 例如，开始选择、结束选择等
                    match button {
                        MouseButton::Left => {
                            match state {
                                winit::event::ElementState::Pressed => {
                                    if let Some(overlay) = &mut self.overlay_window {
                                        // 开始选择
                                        let mut is_selecting = overlay.is_selecting.lock().unwrap();
                                        *is_selecting = true;
                                        println!("Started selecting.");
                                    }
                                }
                                winit::event::ElementState::Released => {
                                    let mut crop_img = None;
                                    let mut pos = (100, 100);
                                    if let Some(overlay) = &mut self.overlay_window {
                                        // 结束选择
                                        let mut is_selecting = overlay.is_selecting.lock().unwrap();
                                        *is_selecting = false;
                                        println!("Finished selecting.");

                                        // 获取选择区域
                                        let selection = overlay.get_selection();
                                        println!("Selected area: {:?}", selection);

                                        pos = if let Some((x1, y1, x2, y2)) = selection {
                                            let left = x1.min(x2);
                                            let top = y1.min(y2);
                                            (left, top)
                                        } else {
                                            (100, 100)
                                        };

                                        // 获取选择区域的图像数据
                                        crop_img = if let Some((x1, y1, x2, y2)) = selection {
                                            let img = overlay.image.as_ref().unwrap();
                                            let (left, top) = (x1.min(x2) as u32, y1.min(y2) as u32);
                                            let (right, bottom) = (x1.max(x2) as u32, y1.max(y2) as u32);
                                            let width = right - left;
                                            let height = bottom - top;

                                            Some(img.view(left, top, width, height).to_image())
                                        } else {
                                            None
                                        };
                                    }

                                    // 关闭 overlay 窗口
                                    // if Some(window_id) == self.overlay_window_id {
                                    //     if let Some(overlay) = &mut self.overlay_window {
                                    //         overlay.handle_event(AppEvent::Reset);
                                    //     }
                                    //     println!("Overlay window close.");
                                    //     self.overlay_window = None;
                                    //     self.overlay_window_id = None;
                                    // }

                                    self.proxy_send_event(AppEvent::Reset);

                                    if let Some(cropped_image) = crop_img {
                                        // 发送显示预览窗口事件
                                        self.proxy_send_event(AppEvent::ShowPreview { cropped_image, position: pos});
                                    }
                                }
                            }
                        }

                        _ => {}
                    }
                }

                if let Some(preview) = self.preview_windows.get_mut(&window_id) {
                    match button {
                        MouseButton::Right => {
                            match state {
                                ElementState::Pressed => {
                                    // 右键点击 pin preview 窗口
                                    println!("Right mouse button clicked, pin preview.");
                                    preview.pin_window();
                                }

                                _ => {}
                            }
                        }
                        
                        MouseButton::Left => {
                            match state {
                                ElementState::Pressed => {
                                    preview.view_state.dragging = true;
                                }
                                ElementState::Released => {
                                    preview.view_state.dragging = false;

                                    let now = Instant::now();
                                    let double_click_threshold = Duration::from_millis(300);

                                    if let Some(last) = preview.click_state.last_click {
                                        if now.duration_since(last) < double_click_threshold {
                                            // 双击
                                            preview.reset_view();
                                            preview.click_state.last_click = None;
                                        } else {
                                            preview.click_state.last_click = Some(now);
                                        }
                                    } else {
                                        preview.click_state.last_click = Some(now);
                                    }

                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            WindowEvent::CursorMoved { device_id, position } => {
                // 处理鼠标移动事件
                if Some(window_id) == self.overlay_window_id {
                    // println!("Mouse moved in overlay window: {:?}, {:?}", device_id, position);
                    // 如果正在选择，更新选择区域
                    if let Some(overlay) = &mut self.overlay_window {
                        let (x, y) = (position.x as i32, position.y as i32);
                        let is_selecting = overlay.is_selecting.lock().unwrap();
                        if *is_selecting {
                            // 更新选择区域逻辑
                            // 这里可以根据当前鼠标位置更新选择矩形
                            if let Ok(mut selection) = overlay.selection.lock() {
                                // 更新结束坐标
                                if let Ok(start_pos) = overlay.start_pos.lock() {
                                    if let Some((start_x, start_y)) = *start_pos {
                                        *selection = Some((start_x, start_y, x, y));
                                    }
                                }
                            }
                            overlay.overlay_win.as_ref().unwrap().request_redraw();
                        } else {
                            // 未选择状态
                            if let Ok(mut start_pos) = overlay.start_pos.lock() {
                                *start_pos = Some((x, y));
                            }
                        }
                    }
                }
                
                if let Some(preview) = self.preview_windows.get_mut(&window_id) {
                    let x = position.x as f32;
                    let y = position.y as f32;

                    if preview.view_state.dragging {
                        let dx = x - preview.view_state.last_mouse_x;
                        let dy = y - preview.view_state.last_mouse_y;

                        preview.view_state.offset_x += dx;
                        preview.view_state.offset_y += dy;
                    }

                    preview.view_state.last_mouse_x = x;
                    preview.view_state.last_mouse_y = y;

                    preview.preview_win.as_ref().unwrap().request_redraw();

                }
            }

            WindowEvent::MouseWheel { device_id, delta, phase } => {
                // 处理鼠标滚轮事件
                if let Some(preview) = self.preview_windows.get_mut(&window_id) {
                    match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => {
                            preview.zoom_at_center(y, 
                                preview.preview_win.as_ref().unwrap().inner_size().width as f32,
                                preview.preview_win.as_ref().unwrap().inner_size().height as f32);
                            preview.preview_win.as_ref().unwrap().request_redraw();
                        }
                        winit::event::MouseScrollDelta::PixelDelta(pos) => {
                            let y = pos.y as f32;
                            preview.zoom_at_center(y, 
                                preview.preview_win.as_ref().unwrap().inner_size().width as f32,
                                preview.preview_win.as_ref().unwrap().inner_size().height as f32);
                            preview.preview_win.as_ref().unwrap().request_redraw();
                        }
                    }
                }
            }

            WindowEvent::Resized(size) => {
                // 处理窗口大小调整事件
                if let Some(preview) = self.preview_windows.get_mut(&window_id) {
                    preview.handle_resize(size);
                    preview.preview_win.as_ref().unwrap().request_redraw();
                }
            }

            _ => {}
        }
    }

    // 当应用程序恢复时调用
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: AppEvent) {
        // 处理用户事件
        match event {
            AppEvent::StartCapture => {
                println!("Starting capture...");
            }
            AppEvent::ShowOverlay { screenshot } => {
                println!("Showing overlay...");

                // let mut overlay = OverlayWindow::new();
                // // overlay.set_image_buffer(screenshot);
                // overlay.show(event_loop, screenshot).unwrap();
                // self.overlay_window = Some(overlay);
                // self.overlay_window_id = Some(self.overlay_window.as_ref().unwrap().overlay_win.as_ref().unwrap().id());
                self.show_overlay(event_loop, screenshot).unwrap();
            }
            AppEvent::ShowPreview { cropped_image, position } => {
                println!("Showing preview...");
                self.create_preview(event_loop, cropped_image, position).unwrap();
            }
            AppEvent::Reset => {
                println!("Resetting...");
                self.close_overlay();
            }
        }
    }
}

fn main() {
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::{EventLoop, ControlFlow};

    let event_loop = EventLoop::<AppEvent>::with_user_event().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = ScreenshotApp::new(event_loop.create_proxy());
    
    // 主事件循环
    run_event_loop(&mut app, event_loop);
}

// 修改主事件循环
fn run_event_loop(app: &mut ScreenshotApp, event_loop: EventLoop<AppEvent>) {

    // let proxy = Arc::new(event_loop.create_proxy());

    // app.start_hotkey_listener(proxy);
    
    // 启动热键监听
    app.start_hotkey_listener();

    event_loop.run_app(app).unwrap();
}