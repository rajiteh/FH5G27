// G27 LED Bridge - Multi-game telemetry to Logitech G27 LED bridge
// 
// Based on DR2G27 by Aely0: https://github.com/Aely0/DR2G27
// Extended with Forza Horizon 5 support and enhanced architecture
//
// MIT License - see LICENSE file for details

use clap::{Parser, Subcommand};
use g27_led_bridge::common::{
    leds::LEDS,
    telemetry::GameType,
    util::{DR2G27Error, DR2G27Result, G27_PID, G27_VID},
};
use hidapi::{HidApi, HidDevice};
use std::{net::UdpSocket, thread::sleep, time::Duration};

// Telemetry config "hardware_settings_config.xml"
// <udp enabled="true" extradata="3" ip="127.0.0.1" port="20777" delay="1" />

#[derive(Parser)]
#[command(name = "g27-led-bridge")]
#[command(about = "Racing game telemetry to Logitech G27 LED bridge")]
struct Cli {
    /// Game to bridge telemetry from
    #[arg(short, long, default_value = "dirt-rally-2")]
    game: String,
    
    /// UDP port to listen on (overrides default for selected game)
    #[arg(short, long)]
    port: Option<u16>,
    
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

fn connect_and_bridge(game_type: GameType, port: u16) -> DR2G27Result {
    println!("# Looking for G27");
    let mut hid = HidApi::new()?;

    loop {
        if device_connected(&hid) {
            if let Ok(device) = hid.open(G27_VID, G27_PID) {
                println!("# G27 connected");
                return read_telemetry_and_update(device, game_type, port);
            } else {
                println!("# Found G27 but failed to open connection");
            }
        } else {
            println!("# G27 not found, retrying in 1 second...");
        }

        sleep(Duration::from_secs(1));
        hid.refresh_devices()?;
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
    
    // Parse game type
    let game_type = match GameType::from_str(&cli.game) {
        Some(game) => game,
        None => {
            println!("# Error: Unknown game '{}'. Supported games: dirt-rally-2, forza-horizon-5", cli.game);
            println!("# Use --help for more information");
            return;
        }
    };
    
    // Determine port
    let port = cli.port.unwrap_or_else(|| game_type.default_port());
    
    match cli.command {
        Some(Commands::Test { continuous }) => {
            if let Err(error) = test_led_functionality(continuous) {
                println!("# LED test failed: {:?}", error);
            }
        }
        None => {
            // Default behavior - bridge game to LEDs
            let parser = game_type.parser();
            println!("# Starting {} to G27 LED bridge", parser.game_name());
            println!("# Use 'g27-led-bridge test' to test LED functionality without the game");
            println!("# Supported games: dirt-rally-2 (dr2), forza-horizon-5 (fh5)");
            
            loop {
                match connect_and_bridge(game_type, port) {
                    Err(error) => {
                        match error {
                            DR2G27Error::DR2UdpSocketError => {
                                println!("# UDP Socket Error - This usually means:");
                                println!("#   1. Port {} is already in use by another application", port);
                                println!("#   2. The game is not sending telemetry data");
                                println!("#   3. Firewall is blocking the connection");
                                println!("# Retrying in 5 seconds...");
                                sleep(Duration::from_secs(5));
                            }
                            DR2G27Error::G27ConnectionLostError => {
                                println!("# G27 connection lost - device may have been disconnected");
                                println!("# Retrying in 2 seconds...");
                                sleep(Duration::from_secs(2));
                            }
                        }
                    }
                    Ok(()) => {
                        println!("# Bridge stopped unexpectedly, restarting...");
                        sleep(Duration::from_secs(1));
                    }
                }
            }
        }
    }
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
