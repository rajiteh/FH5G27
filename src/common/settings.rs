// Settings persistence module for G27 LED Bridge
// 
// Handles automatic loading/saving of user preferences including:
// - Game type selection (DiRT Rally 2.0, Forza Horizon 5)
// - UDP port configuration
// - Persistent storage to %APPDATA%\G27-LED-Bridge\settings.toml
// - CLI argument override support

use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::common::telemetry::GameType;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub game_type: GameType,
    pub port: u16,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            game_type: GameType::DirtRally2,
            port: GameType::DirtRally2.default_port(),
        }
    }
}

impl AppSettings {
    /// Get the config file path in the user's AppData directory
    pub fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mut path = dirs::config_dir()
            .ok_or("Could not find config directory")?;
        path.push("G27-LED-Bridge");
        
        // Create directory if it doesn't exist
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }
        
        path.push("settings.toml");
        Ok(path)
    }
    
    /// Load settings from config file, or return defaults if file doesn't exist
    pub fn load() -> Self {
        match Self::config_path() {
            Ok(path) => {
                if path.exists() {
                    match fs::read_to_string(&path) {
                        Ok(contents) => {
                            match toml::from_str(&contents) {
                                Ok(settings) => {
                                    println!("# Loaded settings from {:?}", path);
                                    return settings;
                                }
                                Err(e) => {
                                    eprintln!("# Error parsing settings file: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("# Error reading settings file: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("# Error accessing config directory: {}", e);
            }
        }
        
        println!("# Using default settings");
        Self::default()
    }
    
    /// Save settings to config file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path()?;
        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        println!("# Settings saved to {:?}", path);
        Ok(())
    }
    
    /// Update game type and save
    pub fn set_game_type(&mut self, game_type: GameType) {
        self.game_type = game_type;
        // Update port to default for the new game if current port matches old game's default
        if self.port == self.game_type.default_port() {
            self.port = game_type.default_port();
        }
        if let Err(e) = self.save() {
            eprintln!("# Failed to save settings: {}", e);
        }
    }
    
    /// Update port and save
    pub fn set_port(&mut self, port: u16) {
        self.port = port;
        if let Err(e) = self.save() {
            eprintln!("# Failed to save settings: {}", e);
        }
    }
    
    /// Get the effective port (command line override or saved setting)
    pub fn get_effective_port(&self, cli_port: Option<u16>) -> u16 {
        cli_port.unwrap_or(self.port)
    }
}