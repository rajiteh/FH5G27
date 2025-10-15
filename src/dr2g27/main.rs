// G27 LED Bridge - Multi-game telemetry to Logitech G27 LED bridge
// 
// Based on DR2G27 by Aely0: https://github.com/Aely0/DR2G27
// Extended with Forza Horizon 5 support and enhanced architecture
//
// MIT License - see LICENSE file for details

use clap::{Parser, Subcommand};
use g27_led_bridge::common::{
    leds::LEDS,
    settings::AppSettings,
    systray::{SystemTray, hide_console_window, create_event_loop},
    telemetry::GameType,
    util::{DR2G27Error, DR2G27Result, G27_PID, G27_VID},
};
use hidapi::{HidApi, HidDevice};
use std::{net::UdpSocket, thread::{self, sleep}, time::Duration, sync::Arc};
use winit::event::WindowEvent;

// Telemetry config "hardware_settings_config.xml"
// <udp enabled="true" extradata="3" ip="127.0.0.1" port="20777" delay="1" />

#[derive(Parser)]
#[command(name = "g27-led-bridge")]
#[command(about = "Racing game telemetry to Logitech G27 LED bridge")]
struct Cli {
    /// Game to bridge telemetry from (overrides saved setting)
    #[arg(short, long)]
    game: Option<String>,
    
    /// UDP port to listen on (overrides saved setting)
    #[arg(short, long)]
    port: Option<u16>,
    
    /// Run in console mode instead of system tray
    #[arg(long)]
    console: bool,
    
    /// Exit immediately if G27 wheel is not found during startup
    #[arg(long)]
    require_wheel: bool,
    
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Test LED functionality without game running
    Test {
        /// Run a continuous LED test pattern
        #[arg(short, long)]
        continuous: bool,
    },
}

fn read_telemetry_and_update(device: HidDevice, game_type: GameType, port: u16) -> DR2G27Result {
    let bind_addr = format!("127.0.0.1:{}", port);
    println!("# Attempting to bind UDP listener to {}", bind_addr);
    
    let socket = match UdpSocket::bind(&bind_addr) {
        Ok(socket) => {
            println!("# Successfully bound to {}", bind_addr);
            socket
        }
        Err(e) => {
            println!("# Failed to bind to port {}: {}", port, e);
            println!("# Port may already be in use. Try a different port with --port <PORT>");
            return Err(e.into());
        }
    };
    
    let mut leds = LEDS::new(device);
    let parser = game_type.parser();
    let expected_size = parser.expected_packet_size();
    let mut data = vec![0u8; expected_size.max(512)]; // Ensure buffer is large enough
    
    println!("# Listening for {} telemetry on port {} (expecting {} byte packets)", 
             parser.game_name(), port, expected_size);
    println!("# Waiting for telemetry data from the game...");

    loop {
        match socket.recv(&mut data) {
            Ok(received_size) => {
                if received_size >= expected_size {
                    leds.update(&data[..received_size], parser.as_ref())?;
                } else {
                    println!("# Received packet too small: {} bytes (expected {})", received_size, expected_size);
                }
            }
            Err(e) => {
                println!("# UDP receive error: {}", e);
                return Err(e.into());
            }
        }
    }
}

fn device_connected(hid: &HidApi) -> bool {
    for device in hid.device_list() {
        if device.product_id() == G27_PID && device.vendor_id() == G27_VID {
            return true;
        }
    }

    false
}

fn connect_and_bridge(
    game_type: GameType, 
    port: u16,
    wheel_status_tx: Option<&std::sync::mpsc::Sender<(bool, Option<String>)>>,
    require_wheel: bool,
) -> DR2G27Result {
    println!("# Looking for G27");
    
    if let Some(tx) = wheel_status_tx {
        let _ = tx.send((false, Some("Searching...".to_string())));
    }
    
    let mut hid = HidApi::new()?;
    let mut found = device_connected(&hid);
    
    if !found {
        println!("# G27 not found...");
        if let Some(tx) = wheel_status_tx {
            let _ = tx.send((false, Some("Not found".to_string())));
        }
        
        if require_wheel {
            println!("# Exiting: G27 wheel required but not found");
            std::process::exit(1);
        }
    }
    
    loop {
        if found {
            if let Ok(device) = hid.open(G27_VID, G27_PID) {
                println!("# G27 connected");
                if let Some(tx) = wheel_status_tx {
                    let _ = tx.send((true, None));
                }
                return read_telemetry_and_update(device, game_type, port);
            } else {
                println!("# Found G27 but failed to open connection");
                if let Some(tx) = wheel_status_tx {
                    let _ = tx.send((false, Some("Connection failed".to_string())));
                }
            }
        } 

        sleep(Duration::from_secs(5));
        hid.refresh_devices()?;
        found = device_connected(&hid);
    }
}


fn test_led_functionality(continuous: bool) -> DR2G27Result {
    println!("# Looking for G27 for LED test");
    let hid = HidApi::new()?;
    
    if !device_connected(&hid) {
        println!("# Error: G27 not found. Please connect your G27 racing wheel.");
        return Ok(());
    }
    
    let device = hid.open(G27_VID, G27_PID)?;
    println!("# G27 connected - Starting LED test");
    
    if continuous {
        println!("# Running continuous LED test (Press Ctrl+C to stop)");
        loop {
            run_led_test_cycle(&device)?;
        }
    } else {
        println!("# Running single LED test cycle");
        run_led_test_cycle(&device)?;
        // Turn off all LEDs at the end
        device.write(&[0x00, 0xF8, 0x12, 0, 0x00, 0x00, 0x00, 0x01])?;
        println!("# LED test completed");
    }
    
    Ok(())
}

fn run_led_test_cycle(device: &HidDevice) -> DR2G27Result {
    // LED states: 0=off, 1=green1, 3=green1+2, 7=green1+2+orange1, 15=green1+2+orange1+2, 31=all
    println!("# Testing LED progression: Off -> Green -> Orange -> Red");
    
    // Progressive LED activation
    let led_states = vec![0, 1, 3, 7, 15, 31];
    for state in &led_states {
        device.write(&[0x00, 0xF8, 0x12, *state, 0x00, 0x00, 0x00, 0x01])?;
        sleep(Duration::from_millis(500));
    }
    
    println!("# Testing reverse LED progression: Red -> Orange -> Green -> Off");
    
    // Reverse LED deactivation
    for state in led_states.iter().rev() {
        device.write(&[0x00, 0xF8, 0x12, *state, 0x00, 0x00, 0x00, 0x01])?;
        sleep(Duration::from_millis(500));
    }
    
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    
    // Handle subcommands first
    match cli.command {
        Some(Commands::Test { continuous }) => {
            match test_led_functionality(continuous) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("# LED test failed: {:?}", e);
                    std::process::exit(1);
                }
            }
            return;
        }
        None => {}
    }
    
    // Load settings
    let mut settings = AppSettings::load();
    
    // Override settings with CLI arguments if provided
    if let Some(ref game_str) = cli.game {
        match GameType::parse_game_name(game_str) {
            Some(game) => {
                settings.set_game_type(game);
            }
            None => {
                println!("# Error: Unknown game '{}'. Supported games: dirt-rally-2, forza-horizon-5", game_str);
                println!("# Use --help for more information");
                return;
            }
        }
    }
    
    let port = settings.get_effective_port(cli.port);
    
    run(settings.game_type, port, cli.console, cli.require_wheel);
}

fn run(initial_game_type: GameType, initial_port: u16, _keep_console: bool, require_wheel: bool) {
    use std::sync::mpsc;
    use std::sync::atomic::{AtomicBool, Ordering};
    
    if !_keep_console {
        hide_console_window();
    }
    
    println!("# Starting G27 LED Bridge in system tray mode");
    println!("# Right-click system tray icon to change games or exit");
    
    // Create system tray
    let tray = match SystemTray::new() {
        Ok(tray) => tray,
        Err(e) => {
            eprintln!("Failed to create system tray: {}", e);
            println!("# Falling back to console mode");
            run(initial_game_type, initial_port, false, require_wheel);
            return;
        }
    };
    
    // Create shared flags and channels
    let exit_flag = Arc::new(AtomicBool::new(false));
    let (status_tx, status_rx) = mpsc::channel::<String>();
    let (wheel_status_tx, wheel_status_rx) = mpsc::channel::<(bool, Option<String>)>();
    
    // Start the bridge in a background thread with dynamic settings
    let exit_flag_clone = Arc::clone(&exit_flag);
    let tray_settings_clone = tray.settings.clone();
    let _bridge_handle = thread::spawn(move || {
        let mut current_game_type = initial_game_type;
        let mut current_port = initial_port;
        
        loop {
            if exit_flag_clone.load(Ordering::Relaxed) {
                break;
            }
            
            // Check for settings changes
            if let Ok(settings) = tray_settings_clone.lock() {
                let new_game_type = settings.game_type;
                let new_port = settings.port;
                
                if new_game_type != current_game_type || new_port != current_port {
                    current_game_type = new_game_type;
                    current_port = new_port;
                    let parser = new_game_type.parser();
                    let _ = status_tx.send(format!("Switched to {} on port {}", parser.game_name(), new_port));
                }
            }
            
            match connect_and_bridge(current_game_type, current_port, Some(&wheel_status_tx), require_wheel) {
                Err(error) => {
                    let msg = match error {
                        DR2G27Error::DR2UdpSocketError => {
                            let _ = wheel_status_tx.send((false, Some("UDP Error".to_string())));
                            "UDP Socket Error - retrying in 5 seconds...".to_string()
                        }
                        DR2G27Error::G27ConnectionLostError => {
                            let _ = wheel_status_tx.send((false, Some("Disconnected".to_string())));
                            "G27 connection lost - retrying in 2 seconds...".to_string()
                        }
                    };
                    let _ = status_tx.send(msg);
                    
                    // Sleep with periodic exit checks
                    for _ in 0..50 { // Check every 100ms for 5 seconds max
                        if exit_flag_clone.load(Ordering::Relaxed) {
                            return;
                        }
                        sleep(Duration::from_millis(100));
                    }
                }
                Ok(()) => {
                    let _ = status_tx.send("Bridge stopped unexpectedly, restarting...".to_string());
                    sleep(Duration::from_secs(1));
                }
            }
        }
    });
    
    // Run the event loop for system tray
    let event_loop = create_event_loop();
    let _ = event_loop.run(move |event, elwt| {
        elwt.set_control_flow(winit::event_loop::ControlFlow::Wait);
        
        if let winit::event::Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = event {
            exit_flag.store(true, Ordering::Relaxed);
            elwt.exit();
        }
        
        // Check for status messages
        while let Ok(status) = status_rx.try_recv() {
            println!("# {}", status);
        }
        
        // Check for wheel status updates
        while let Ok((connected, error_msg)) = wheel_status_rx.try_recv() {
            tray.update_wheel_status(connected, error_msg.as_deref());
        }
        
        // Check for settings changes (menu)
        if tray.settings_changed() {
            println!("# Settings changed - bridge will update automatically");
            tray.update_menu_display();
        }
        
        // Check if we should exit
        if tray.should_exit() {
            exit_flag.store(true, Ordering::Relaxed);
            elwt.exit();
        }
    });
}

#[test]
fn test_device_leds() -> DR2G27Result {
    let device = HidApi::new()?.open(G27_VID, G27_PID)?;

    for state in vec![0, 1, 3, 7, 15, 31] {
        device.write(&[0x00, 0xF8, 0x12, state, 0x00, 0x00, 0x00, 0x01])?;
        sleep(Duration::from_millis(200));
    }

    sleep(Duration::from_secs(1));

    for state in vec![31, 15, 7, 3, 1, 0] {
        device.write(&[0x00, 0xF8, 0x12, state, 0x00, 0x00, 0x00, 0x01])?;
        sleep(Duration::from_millis(200));
    }

    Ok(())
}
