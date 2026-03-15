#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod control;
mod overlay;
mod preview;

use capture::*;
use overlay::OverlayWindow;
use preview::PreviewWindow;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowId;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// 定义应用程序状态
#[derive(Clone)]
struct AppState {
    // 截图缓存
    screenshot: Arc<Mutex<Option<image::RgbaImage>>>,
    // 热键状态
    hotkey_pressed: Arc<Mutex<bool>>,
    // 热键启用状态
    hotkey_enabled: Arc<Mutex<bool>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            screenshot: Arc::new(Mutex::new(None)),
            hotkey_pressed: Arc::new(Mutex::new(false)),
            hotkey_enabled: Arc::new(Mutex::new(false)),
        }
    }
}

#[derive(Debug, Clone)]
enum AppEvent {
    ShowOverlay {
        screenshot: image::RgbaImage,
    },

    ShowPreview {
        cropped_image: image::RgbaImage,
        position: (i32, i32),
    },

    Reset,

    // tray-icon 菜单事件
    MenuEvent(tray_icon::menu::MenuEvent),

    // tray 菜单操作
    // 切换热键启用状态
    HotKeyEnable(bool),

    // 立即截图
    TakeScreenshot,

    // 退出应用
    Exit,
}

// 应用程序管理器
struct ScreenshotApp {
    state: AppState,

    // overlay 窗口管理器
    overlay_window: Option<OverlayWindow>,
    // overlay 覆盖 窗口ID
    overlay_window_id: Option<winit::window::WindowId>,

    // preview 窗口管理列表
    preview_windows: HashMap<WindowId, PreviewWindow>,

    alt_pressed: Arc<Mutex<bool>>,

    // 事件代理
    event_proxy: Arc<EventLoopProxy<AppEvent>>,

    // tray-icon 控制器
    control: Option<control::Control>,
}

impl ScreenshotApp {
    fn new(event_proxy: EventLoopProxy<AppEvent>) -> Self {
        let tray = control::Control::new(Arc::new(event_proxy.clone()));

        Self {
            state: AppState::new(),

            overlay_window: None,
            overlay_window_id: None,

            preview_windows: HashMap::new(),

            alt_pressed: Arc::new(Mutex::new(false)),

            event_proxy: Arc::new(event_proxy),

            control: Some(tray),
        }
    }

    // 监听热键
    fn start_hotkey_listener(&self) {
        let hotkey_enabled = self.state.hotkey_enabled.clone();
        let hotkey_pressed = self.state.hotkey_pressed.clone();
        let alt_pressed = self.alt_pressed.clone();

        let screenshot_state = self.state.screenshot.clone();

        let proxy = self.event_proxy.clone();

        std::thread::spawn(move || {
            use rdev::{listen, Event, EventType, Key};

            listen(move |event: Event| {
                // 先检查热键是否启用
                let enabled = hotkey_enabled.lock().map(|e| *e).unwrap_or(false);
                if !enabled {
                    return;
                }
                match event.event_type {
                    EventType::KeyPress(key) => {
                        match key {
                            Key::Alt => {
                                if let Ok(mut alt) = alt_pressed.lock() {
                                    *alt = true;
                                }
                            }
                            Key::KeyA => {
                                // 检查是否同时按下了 Ctrl 和 Shift
                                let alt = alt_pressed.lock().map(|c| *c).unwrap_or(false);

                                if alt {
                                    println!("Alt+A pressed!");
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
                                        screenshot_state
                                            .lock()
                                            .unwrap()
                                            .replace(screenshot.clone());
                                        println!("Screenshot stored in state.");

                                        proxy
                                            .send_event(AppEvent::ShowOverlay { screenshot })
                                            .unwrap();
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
                            // Key::ControlLeft => {
                            //     if let Ok(mut ctrl) = ctrl_pressed.lock() {
                            //         *ctrl = false;
                            //     }
                            // }
                            // Key::ShiftLeft | Key::ShiftRight => {
                            //     if let Ok(mut shift) = shift_pressed.lock() {
                            //         *shift = false;
                            //     }
                            // }
                            Key::Alt => {
                                if let Ok(mut alt) = alt_pressed.lock() {
                                    *alt = false;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            })
            .unwrap();
        });
    }
}

// 在 ScreenshotApp 中添加方法
impl ScreenshotApp {
    #[allow(dead_code)]
    fn proxy_send_event(&self, event: AppEvent) {
        self.event_proxy.send_event(event).unwrap();
    }

    fn show_overlay(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        screenshot: image::RgbaImage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut overlay = OverlayWindow::new();
        overlay.show(event_loop, screenshot)?;
        self.overlay_window = Some(overlay);
        self.overlay_window_id = Some(
            self.overlay_window
                .as_ref()
                .unwrap()
                .overlay_win
                .as_ref()
                .unwrap()
                .id(),
        );
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

    fn create_preview(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        image: image::RgbaImage,
        pos: (i32, i32),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut preview = PreviewWindow::new(image);
        preview.show(event_loop, pos)?;

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
    fn new_events(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _cause: winit::event::StartCause,
    ) {
    }

    // 窗口事件分发
    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        // 匹配操作

        let new_event = event.clone();

        // 必须在当前作用域内才能执行的操作 提前判断
        match new_event {
            // 处理窗口销毁事件
            WindowEvent::Destroyed => {
                if Some(window_id) == self.overlay_window_id {
                    self.overlay_window = None;
                    self.overlay_window_id = None;
                }

                if self.preview_windows.contains_key(&window_id) {
                    self.preview_windows.remove(&window_id);
                    println!("Preview window removed from registry: {:?}", window_id);
                }
            }

            // 处理窗口关闭请求
            WindowEvent::CloseRequested => {
                if Some(window_id) == self.overlay_window_id {
                    self.close_overlay();
                }

                if let Some(_) = self.preview_windows.get(&window_id) {
                    self.destory_preview(window_id);
                }
            }

            // 处理键盘输入事件
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
                    }
                    _ => {}
                }
            }

            _ => {}
        }

        // 将事件分发给 overlay 窗口
        if Some(window_id) == self.overlay_window_id {
            if let Some(overlay) = &mut self.overlay_window {
                overlay.handle_window_event(event_loop, window_id, event, self.event_proxy.clone());
                return;
            }
        }

        // 将事件分发给 preview 窗口
        if let Some(preview) = self.preview_windows.get_mut(&window_id) {
            preview.handle_window_event(event_loop, window_id, event, self.event_proxy.clone());
            return;
        }
    }

    // 当应用程序恢复时调用
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: AppEvent) {
        // 处理用户事件
        match event {
            AppEvent::ShowOverlay { screenshot } => {
                println!("Showing overlay...");
                self.show_overlay(event_loop, screenshot).unwrap();
            }

            AppEvent::ShowPreview {
                cropped_image,
                position,
            } => {
                println!("Showing preview...");
                self.create_preview(event_loop, cropped_image, position)
                    .unwrap();
            }

            AppEvent::Reset => {
                println!("Resetting...");
                self.close_overlay();
            }
            
            AppEvent::MenuEvent(event) => {
                println!("Menu event received: {:?}\n", event);
                if let Some(control) = &self.control {
                    control.handle_menu_event(event);
                }
            }

            AppEvent::HotKeyEnable(value) => {
                println!("Hotkey enable set to: {}", value);
                if let Ok(mut enabled) = self.state.hotkey_enabled.lock() {
                    *enabled = value;
                }
            }

            AppEvent::TakeScreenshot => {
                println!("Taking screenshot...");
                self.show_overlay(event_loop, capture_fullscreen().unwrap())
                    .unwrap();
            }

            AppEvent::Exit => {
                println!("Exiting application...");
                event_loop.exit();
            }
        }
    }
}

fn main() {
    use winit::event_loop::{ControlFlow, EventLoop};

    if !ensure_single_instance() {
        eprintln!("application in running");
        return;
    }

    let event_loop = EventLoop::<AppEvent>::with_user_event().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = ScreenshotApp::new(event_loop.create_proxy());

    // 主事件循环
    run_event_loop(&mut app, event_loop);
}

// 修改主事件循环
fn run_event_loop(app: &mut ScreenshotApp, event_loop: EventLoop<AppEvent>) {
    // 启动热键监听
    app.start_hotkey_listener();

    if let Some(control) = &app.control {
        control.set_proxy_event();
    }

    event_loop.run_app(app).unwrap();
}

// 检测windows系统 该应用进程是否已经开启
fn ensure_single_instance() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::{
        Foundation::{GetLastError, ERROR_ALREADY_EXISTS},
        System::Threading::CreateMutexW,
    };

    let name = "Global\\SSPIN_SingleInstance";

    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();

    unsafe {
        let handle = CreateMutexW(None, false, PCWSTR(wide.as_ptr()));

        if handle.is_err() {
            return false;
        }

        GetLastError() != ERROR_ALREADY_EXISTS
    }
}
