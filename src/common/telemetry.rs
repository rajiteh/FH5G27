// Telemetry parsing module for multi-game support
// 
// Extends the original DR2G27 architecture by Aely0 with:
// - Trait-based telemetry parsing
// - Forza Horizon 5 support
// - Game-agnostic RPM extraction

use std::convert::TryFrom;

/// Trait for parsing telemetry data from different racing games
pub trait TelemetryParser {
    /// Parse telemetry data and return (current_rpm, max_rpm, idle_rpm, is_race_active)
    fn parse_rpm_data(&self, data: &[u8]) -> (f32, f32, f32, bool);
    
    /// Get the expected packet size for this game's telemetry
    fn expected_packet_size(&self) -> usize;
    
    /// Get the game name for logging
    fn game_name(&self) -> &'static str;
}

/// Helper function to convert bytes to f32
fn f32_from_byte_slice(slice: &[u8]) -> f32 {
    f32::from_le_bytes(<[u8; 4]>::try_from(slice).expect("bytes_to_f32"))
}

/// Helper function to convert bytes to i32
fn i32_from_byte_slice(slice: &[u8]) -> i32 {
    i32::from_le_bytes(<[u8; 4]>::try_from(slice).expect("bytes_to_i32"))
}

/// DiRT Rally 2.0 telemetry parser
pub struct DirtRally2Parser;

impl TelemetryParser for DirtRally2Parser {
    fn parse_rpm_data(&self, data: &[u8]) -> (f32, f32, f32, bool) {
        if data.len() < self.expected_packet_size() {
            return (0.0, 0.0, 0.0, false);
        }
        
        let current_rpm = f32_from_byte_slice(&data[148..152]);
        let max_rpm = f32_from_byte_slice(&data[252..256]);
        let idle_rpm = f32_from_byte_slice(&data[256..260]);
        
        // For DiRT Rally 2.0, assume race is active if we're receiving valid RPM data
        let is_race_active = max_rpm > 0.0 && current_rpm >= 0.0;
        
        (current_rpm, max_rpm, idle_rpm, is_race_active)
    }
    
    fn expected_packet_size(&self) -> usize {
        264 // DiRT Rally 2.0 packet size
    }
    
    fn game_name(&self) -> &'static str {
        "DiRT Rally 2.0"
    }
}

/// Forza Horizon 5 telemetry parser
pub struct ForzaHorizon5Parser;

impl TelemetryParser for ForzaHorizon5Parser {
    fn parse_rpm_data(&self, data: &[u8]) -> (f32, f32, f32, bool) {
        if data.len() < self.expected_packet_size() {
            return (0.0, 0.0, 0.0, false);
        }
        
        // Check if race is active (IsRaceOn flag)
        let is_race_on = i32_from_byte_slice(&data[0..4]) == 1;
        
        if !is_race_on {
            return (0.0, 0.0, 0.0, false);
        }
        
        let max_rpm = f32_from_byte_slice(&data[8..12]);
        let idle_rpm = f32_from_byte_slice(&data[12..16]);
        let current_rpm = f32_from_byte_slice(&data[16..20]);
        
        (current_rpm, max_rpm, idle_rpm, is_race_on)
    }
    
    fn expected_packet_size(&self) -> usize {
        232 // Forza "Sled" format packet size (smaller than "Dash" format)
    }
    
    fn game_name(&self) -> &'static str {
        "Forza Horizon 5"
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GameType {
    DirtRally2,
    ForzaHorizon5,
}

impl GameType {
    pub fn parser(&self) -> Box<dyn TelemetryParser> {
        match self {
            GameType::DirtRally2 => Box::new(DirtRally2Parser),
            GameType::ForzaHorizon5 => Box::new(ForzaHorizon5Parser),
        }
    }
    
    pub fn default_port(&self) -> u16 {
        match self {
            GameType::DirtRally2 => 20777,
            GameType::ForzaHorizon5 => 9999, // Common Forza port
        }
    }

    pub fn from_str(s: &str) -> Option<GameType> {
        match s.to_lowercase().as_str() {
            "dirt-rally-2" | "dr2" | "dirt" => Some(GameType::DirtRally2),
            "forza-horizon-5" | "fh5" | "forza" => Some(GameType::ForzaHorizon5),
            _ => None,
        }
    }
}