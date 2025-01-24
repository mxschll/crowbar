use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::f32;
use std::fs;
use std::path::PathBuf;
use toml;

const BACKGROUND_COLOR: u32 = 0x141D21;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    // Window size
    pub window_width: f32,
    pub window_heigth: f32,
    // Font
    pub font_family: String,
    pub font_size: f32,
    // Colors
    foreground: String,
    background: String,
    selection_foreground: String,
    selection_background: String,
    border_color: String,
    active_border_color: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_width: 800.,
            window_heigth: 400.,
            font_family: String::from("Liberation Mono"),
            font_size: 12.0,
            background: String::from("#c5c8c6"),
            foreground: String::from("#1d1f21"),
            selection_foreground: String::from("#ffffff"),
            selection_background: String::from("#373b41"),
            border_color: String::from("#81a2be"),
            active_border_color: String::from("#373b41"),
        }
    }
}

impl Config {
    /// Convert a hex color string (e.g. "#ffffff") to u32
    fn hex_to_u32(hex: &str) -> Result<u32> {
        let hex = hex.trim_start_matches('#');
        if hex.chars().count() != 6 {
            return Err(anyhow::anyhow!(
                "Invalid hex color length - must be 6 characters"
            ));
        }

        u32::from_str_radix(hex, 16).context("Failed to parse hex color")
    }

    /// Get background color as u32
    pub fn background_color(&self) -> u32 {
        Self::hex_to_u32(&self.background).unwrap_or(BACKGROUND_COLOR)
    }

    /// Get foreground color as u32
    pub fn foreground_color(&self) -> Result<u32> {
        Self::hex_to_u32(&self.foreground)
    }

    /// Get selection background color as u32
    pub fn selection_background_color(&self) -> Result<u32> {
        Self::hex_to_u32(&self.selection_background)
    }

    /// Get selection foreground color as u32
    pub fn selection_foreground_color(&self) -> Result<u32> {
        Self::hex_to_u32(&self.selection_foreground)
    }

    /// Get border color as u32
    pub fn border_color_value(&self) -> Result<u32> {
        Self::hex_to_u32(&self.border_color)
    }

    /// Get active border color as u32
    pub fn active_border_color_value(&self) -> Result<u32> {
        Self::hex_to_u32(&self.active_border_color)
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if !config_path.exists() {
            return Self::create_default_config();
        }

        match Self::read_config(&config_path) {
            Ok(config) => Ok(config),
            Err(_) => {
                // If reading fails, delete the existing file and create a new one
                fs::remove_file(&config_path).context("Failed to remove invalid config file")?;
                Self::create_default_config()
            }
        }
    }

    fn get_config_path() -> Result<PathBuf> {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .context("Failed to determine home directory")?;

        let config_dir = PathBuf::from(home).join(".config").join("crowbar");
        fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        Ok(config_dir.join("crowbar.config"))
    }

    fn read_config(path: &PathBuf) -> Result<Self> {
        let contents = fs::read_to_string(path).context("Failed to read config file")?;

        toml::from_str(&contents).context("Failed to parse config file")
    }

    fn create_default_config() -> Result<Self> {
        let config = Self::default();
        let config_path = Self::get_config_path()?;

        let toml = toml::to_string_pretty(&config).context("Failed to serialize config")?;

        fs::write(&config_path, toml).context("Failed to write config file")?;

        Ok(config)
    }
}
