use crate::AppEvent;
use image::ImageReader;
use std::{io::Cursor, sync::Arc};
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem},
};
use winit::event_loop::EventLoopProxy;

pub struct Control {
    #[allow(dead_code)]
    pub tray_icon: tray_icon::TrayIcon,
    pub toggle_hotkey: Option<CheckMenuItem>,
    pub toggle_hotkey_id: MenuId,
    pub screentshot_id: MenuId,
    pub exit_id: MenuId,
    pub proxy_event: Arc<EventLoopProxy<AppEvent>>,
}

impl Control {
    pub fn new(proxy_event: Arc<EventLoopProxy<AppEvent>>) -> Self {
        // 创建菜单
        let menu = Menu::new();

        // 创建菜单项
        // 启用/禁用热键
        let toggle_hotkey = CheckMenuItem::new("Enable Hotkey", true, false, None);

        // 立即截图
        let screentshot = MenuItem::new("Take Screenshot", true, None);

        // 退出应用
        let exit = MenuItem::new("Exit", true, None);

        // 将菜单项添加到菜单中
        if let Err(_) = menu.append(&toggle_hotkey) {
            panic!("Failed to add toggle hotkey menu item");
        }
        if let Err(_) = menu.append(&screentshot) {
            panic!("Failed to add screenshot menu item");
        }
        if let Err(_) = menu.append(&exit) {
            panic!("Failed to add exit menu item");
        }

        // 创建系统托盘图标
        let tray_icon = TrayIconBuilder::new()
            .with_icon(load_icon())
            .with_menu(Box::new(menu))
            .with_tooltip("Screenshot Tool")
            .build()
            .expect("Failed to create tray icon");

        let toggle_hotkey_id = toggle_hotkey.id().clone();
        // 返回 Control 实例
        Control {
            tray_icon,
            toggle_hotkey: Some(toggle_hotkey),
            toggle_hotkey_id: toggle_hotkey_id,
            screentshot_id: screentshot.id().clone(),
            exit_id: exit.id().clone(),
            proxy_event,
        }
    }

    pub fn set_proxy_event(&self) {
        // 监听菜单事件
        let proxy = self.proxy_event.clone();
        MenuEvent::set_event_handler(Some(move |event| {
            if let Err(_) = proxy.send_event(AppEvent::MenuEvent(event)) {
                eprintln!("Failed to send menu event");
            }
        }));
    }

    pub fn handle_menu_event(&self, event: MenuEvent) {
        if event.id().0 == self.toggle_hotkey_id.0 {
            // 切换热键启用状态
            if let Some(item) = &self.toggle_hotkey {
                let enable = item.is_checked();
                if let Err(_) = self.proxy_event.send_event(AppEvent::HotKeyEnable(enable)) {
                    eprintln!("Failed to send hotkey enable event");
                }
            }
        } else if event.id().0 == self.screentshot_id.0 {
            // 立即截图
            if let Err(_) = self.proxy_event.send_event(AppEvent::TakeScreenshot) {
                eprintln!("Failed to send take screenshot event");
            }
        } else if event.id().0 == self.exit_id.0 {
            // 退出应用
            if let Err(_) = self.proxy_event.send_event(AppEvent::Exit) {
                eprintln!("Failed to send exit event");
            }
        }
    }
}

fn load_icon() -> Icon {
    // 编译期嵌入
    const ICON_BYTES: &[u8] = include_bytes!("../asset/logo.ico");

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
