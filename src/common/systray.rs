// System tray implementation for G27 LED Bridge
// 
// Provides a comprehensive background interface with:
// - Game selection menu (DiRT Rally 2.0, Forza Horizon 5)
// - Settings editor integration (Notepad)
// - Manual settings reload functionality
// - Status display and about dialog
// - Clean exit handling
// 
// Simplified from complex file watching to reliable manual reload approach

use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    TrayIcon, TrayIconBuilder,
};
use winit::{
    event_loop::{EventLoop, EventLoopBuilder},
    platform::windows::EventLoopBuilderExtWindows,
};
use crate::common::{settings::AppSettings, telemetry::GameType};

#[derive(Debug, Clone, Copy)]
enum MenuAction {
    Quit,
    About,
    SelectDirtRally,
    SelectForzaHorizon,
    OpenSettings,
    ReloadSettings,
}

// Global menu ID registry
static MENU_ACTIONS: OnceLock<Mutex<HashMap<String, MenuAction>>> = OnceLock::new();

pub struct SystemTray {
    _tray: TrayIcon,
    pub should_exit: Arc<Mutex<bool>>,
    pub settings_changed: Arc<Mutex<bool>>,
    pub settings: Arc<Mutex<AppSettings>>,
    status_item: MenuItem,
    port_item: MenuItem,
    wheel_status_item: MenuItem,
}

impl SystemTray {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let should_exit = Arc::new(Mutex::new(false));
        let should_exit_clone = should_exit.clone();
        let settings_changed = Arc::new(Mutex::new(false));
        let settings_changed_clone = settings_changed.clone();
        
        // Load settings
        let settings = Arc::new(Mutex::new(AppSettings::load()));
        let settings_clone = settings.clone();
        let current_game = settings.lock().unwrap().game_type;

        // Create game selection menu items
        let dirt_rally_item = MenuItem::new("DiRT Rally 2.0", true, None);
        let forza_horizon_item = MenuItem::new("Forza Horizon 5", true, None);
        
        let games_submenu = Submenu::new("Select Game", true);
        games_submenu.append(&dirt_rally_item)?;
        games_submenu.append(&forza_horizon_item)?;
        
        // Create settings menu items
        let open_settings_item = MenuItem::new("Edit Settings...", true, None);
        let reload_settings_item = MenuItem::new("Reload Settings", true, None);
        
        // Create other menu items  
        let status_item = MenuItem::new(format!("Active: {}", current_game.parser().game_name()), false, None);
        let port_item = MenuItem::new(format!("Port: {}", settings.lock().unwrap().port), false, None);
        let wheel_status_item = MenuItem::new("Wheel: Checking...", false, None);
        let separator1 = PredefinedMenuItem::separator();
        let separator2 = PredefinedMenuItem::separator();
        let about_item = MenuItem::new("About G27 LED Bridge", true, None);
        let quit_item = MenuItem::new("Exit G27 LED Bridge", true, None);

        let menu = Menu::new();
        menu.append(&status_item)?;
        menu.append(&port_item)?;
        menu.append(&wheel_status_item)?;
        menu.append(&separator1)?;
        menu.append(&games_submenu)?;
        menu.append(&open_settings_item)?;
        menu.append(&reload_settings_item)?;
        menu.append(&separator2)?;
        menu.append(&about_item)?;
        menu.append(&quit_item)?;

        // Create tray icon using embedded icon data
        let icon = Self::create_tray_icon()?;
        
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("G27 LED Bridge - Racing Game Telemetry")
            .with_icon(icon)
            .build()?;

        // Store menu IDs for event matching
        let menu_actions = MENU_ACTIONS.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut actions) = menu_actions.lock() {
            actions.insert(format!("{:?}", quit_item.id()), MenuAction::Quit);
            actions.insert(format!("{:?}", about_item.id()), MenuAction::About);
            actions.insert(format!("{:?}", dirt_rally_item.id()), MenuAction::SelectDirtRally);
            actions.insert(format!("{:?}", forza_horizon_item.id()), MenuAction::SelectForzaHorizon);
            actions.insert(format!("{:?}", open_settings_item.id()), MenuAction::OpenSettings);
            actions.insert(format!("{:?}", reload_settings_item.id()), MenuAction::ReloadSettings);
        }

        // Handle menu events
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let event_id = format!("{:?}", event.id);
            
            if let Some(menu_actions) = MENU_ACTIONS.get() {
                if let Ok(actions) = menu_actions.lock() {
                    if let Some(action) = actions.get(&event_id) {
                        match action {
                            MenuAction::Quit => {
                                if let Ok(mut should_exit) = should_exit_clone.lock() {
                                    *should_exit = true;
                                }
                            }
                            MenuAction::About => {
                                Self::show_about_dialog();
                            }
                            MenuAction::SelectDirtRally => {
                                if let Ok(mut settings) = settings_clone.lock() {
                                    settings.set_game_type(GameType::DirtRally2);
                                }
                                if let Ok(mut changed) = settings_changed_clone.lock() {
                                    *changed = true;
                                }
                                // Note: Menu update will happen in main loop
                            }
                            MenuAction::SelectForzaHorizon => {
                                if let Ok(mut settings) = settings_clone.lock() {
                                    settings.set_game_type(GameType::ForzaHorizon5);
                                }
                                if let Ok(mut changed) = settings_changed_clone.lock() {
                                    *changed = true;
                                }
                                // Note: Menu update will happen in main loop
                            }
                            MenuAction::OpenSettings => {
                                Self::open_settings_file();
                            }
                            MenuAction::ReloadSettings => {
                                if let Ok(mut settings) = settings_clone.lock() {
                                    *settings = AppSettings::load();
                                    println!("# Settings reloaded from file");
                                }
                                if let Ok(mut changed) = settings_changed_clone.lock() {
                                    *changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }));

        Ok(SystemTray {
            _tray: tray,
            should_exit,
            settings_changed,
            settings,
            status_item,
            port_item,
            wheel_status_item,
        })
    }

    fn create_tray_icon() -> Result<tray_icon::Icon, Box<dyn std::error::Error>> {
        // Create a simple 16x16 icon with G27 colors (green/orange/red)
        let icon_data = Self::create_icon_data();
        let icon = tray_icon::Icon::from_rgba(icon_data, 16, 16)?;
        Ok(icon)
    }

    fn create_icon_data() -> Vec<u8> {
        let mut data = Vec::with_capacity(16 * 16 * 4); // RGBA
        
        for y in 0..16 {
            for x in 0..16 {
                let (r, g, b, a) = if (2..=13).contains(&x) && (6..=9).contains(&y) {
                    // LED bar area
                    match x {
                        2..=4 => (0, 255, 0, 255),   // Green LEDs
                        5..=7 => (0, 255, 0, 255),   // Green LEDs
                        8..=10 => (255, 165, 0, 255), // Orange LEDs
                        11..=13 => (255, 0, 0, 255),  // Red LED
                        _ => (64, 64, 64, 255),       // Background
                    }
                } else if (1..=14).contains(&x) && (5..=10).contains(&y) {
                    (32, 32, 32, 255) // Border
                } else {
                    (0, 0, 0, 0) // Transparent
                };
                
                data.extend_from_slice(&[r, g, b, a]);
            }
        }
        data
    }

    fn show_about_dialog() {
        #[cfg(windows)]
        {
            use winapi::um::winuser::{MessageBoxA, MB_ICONINFORMATION, MB_OK};
            use std::ffi::CString;
            
            let title = CString::new("About G27 LED Bridge").unwrap();
            let message = CString::new(
                "G27 LED Bridge v2.0.0\n\n\
                Multi-game telemetry bridge for Logitech G27 Racing Wheel\n\n\
                Supported Games:\n\
                - DiRT Rally 2.0\n\
                - Forza Horizon 5\n\n\
                Based on DR2G27 by Aely0\n\
                Extended by Rajitha Perera\n\n\
                MIT License"
            ).unwrap();
            
            unsafe {
                MessageBoxA(
                    std::ptr::null_mut(),
                    message.as_ptr(),
                    title.as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
        }
    }
    
    fn open_settings_file() {
        #[cfg(windows)]
        {
            if let Ok(settings_path) = AppSettings::config_path() {
                // Use Windows ShellExecute API which works reliably in Windows subsystem mode
                use winapi::um::shellapi::ShellExecuteW;
                use winapi::um::winuser::SW_SHOW;
                use std::ffi::OsStr;
                use std::os::windows::ffi::OsStrExt;
                
                let file_path_wide: Vec<u16> = OsStr::new(&settings_path)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                    
                let operation_wide: Vec<u16> = OsStr::new("open")
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                    
                let application_wide: Vec<u16> = OsStr::new("notepad.exe")
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                
                unsafe {
                    ShellExecuteW(
                        std::ptr::null_mut(),
                        operation_wide.as_ptr(),
                        application_wide.as_ptr(),
                        file_path_wide.as_ptr(),
                        std::ptr::null(),
                        SW_SHOW,
                    );
                }
                println!("# Opened settings file in Notepad");
            }
        }
        
        #[cfg(not(windows))]
        {
            if let Ok(settings_path) = AppSettings::config_path() {
                println!("# Settings file location: {}", settings_path.display());
                println!("# Edit the file and use 'Reload Settings' menu to apply changes");
            }
        }
    }

    pub fn should_exit(&self) -> bool {
        *self.should_exit.lock().unwrap()
    }
    
    pub fn settings_changed(&self) -> bool {
        if let Ok(mut changed) = self.settings_changed.lock() {
            let result = *changed;
            *changed = false; // Reset the flag
            result
        } else {
            false
        }
    }
    
    pub fn get_current_settings(&self) -> AppSettings {
        self.settings.lock().unwrap().clone()
    }

    pub fn update_status(&self, status: &str) {
        println!("# Status: {}", status);
    }
    
    pub fn update_menu_display(&self) {
        if let Ok(settings) = self.settings.lock() {
            let game_name = settings.game_type.parser().game_name();
            let port = settings.port;
            
            // Update menu item text
            self.status_item.set_text(format!("Active: {}", game_name));
            self.port_item.set_text(format!("Port: {}", port));
            
            println!("# Menu updated: {} on port {}", game_name, port);
        }
    }
    
    pub fn update_wheel_status(&self, connected: bool, error_msg: Option<&str>) {
        let status_text = if connected {
            "Wheel: Connected ✓"
        } else if let Some(msg) = error_msg {
            &format!("Wheel: Error - {}", msg)
        } else {
            "Wheel: Not Found ✗"
        };
        
        self.wheel_status_item.set_text(status_text);
        
        if !connected {
            println!("# Wheel Status: {}", status_text);
        }
    }
    
    pub fn update_wheel_connecting(&self) {
        self.wheel_status_item.set_text("Wheel: Connecting...");
    }

}

pub fn hide_console_window() {
    #[cfg(windows)]
    {
        unsafe { winapi::um::wincon::FreeConsole() };
    }
}

pub fn create_event_loop() -> EventLoop<()> {
    EventLoopBuilder::new()
        .with_any_thread(true)
        .build()
        .expect("Failed to create event loop")
}