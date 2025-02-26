use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result};
use gpui::{App, Global, Rgba};
use log;
use serde::{Deserialize, Serialize};
use toml;

/// A color in RGB format
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(from = "String", into = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_rgba(&self) -> Rgba {
        Rgba {
            r: self.r as f32 / 255.0,
            g: self.g as f32 / 255.0,
            b: self.b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub fn from_rgba(rgba: &Rgba) -> Self {
        Self {
            r: (rgba.r * 255.0) as u8,
            g: (rgba.g * 255.0) as u8,
            b: (rgba.b * 255.0) as u8,
        }
    }

    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn from_hex(hex: &str) -> Result<Self, anyhow::Error> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return Err(anyhow::anyhow!("Invalid hex color format: {}", hex));
        }

        Ok(Self {
            r: u8::from_str_radix(&hex[0..2], 16)
                .with_context(|| format!("Invalid red component in hex color: {}", hex))?,
            g: u8::from_str_radix(&hex[2..4], 16)
                .with_context(|| format!("Invalid green component in hex color: {}", hex))?,
            b: u8::from_str_radix(&hex[4..6], 16)
                .with_context(|| format!("Invalid blue component in hex color: {}", hex))?,
        })
    }
}

impl From<String> for Color {
    fn from(hex: String) -> Self {
        Self::from_hex(&hex).unwrap_or_else(|e| {
            log::warn!("Failed to parse color '{}': {}", hex, e);
            Self::new(0, 0, 0)
        })
    }
}

impl From<Color> for String {
    fn from(color: Color) -> Self {
        color.to_hex()
    }
}

/// Status bar item types
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StatusItem {
    Text { content: String },
    DateTime { format: String },
}

impl Default for StatusItem {
    fn default() -> Self {
        StatusItem::Text {
            content: String::new(),
        }
    }
}

/// Application configuration
pub struct Config {
    pub text_primary_color: Rgba,
    pub text_secondary_color: Rgba,
    pub text_selected_primary_color: Rgba,
    pub text_selected_secondary_color: Rgba,
    pub background_color: Rgba,
    pub border_color: Rgba,
    pub selected_background_color: Rgba,
    pub font_family: String,
    pub font_size: f32,
    pub window_width: f32,
    pub window_height: f32,
    pub status_bar_left: Vec<StatusItem>,
    pub status_bar_center: Vec<StatusItem>,
    pub status_bar_right: Vec<StatusItem>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            text_primary_color: Rgba {
                r: 205.0 / 255.0,
                g: 214.0 / 255.0,
                b: 244.0 / 255.0,
                a: 1.0,
            },
            text_secondary_color: Rgba {
                r: 166.0 / 255.0,
                g: 173.0 / 255.0,
                b: 200.0 / 255.0,
                a: 1.0,
            },
            text_selected_primary_color: Rgba {
                r: 205.0 / 255.0,
                g: 214.0 / 255.0,
                b: 244.0 / 255.0,
                a: 1.0,
            },
            text_selected_secondary_color: Rgba {
                r: 166.0 / 255.0,
                g: 173.0 / 255.0,
                b: 200.0 / 255.0,
                a: 1.0,
            },
            background_color: Rgba {
                r: 30.0 / 255.0,
                g: 31.0 / 255.0,
                b: 47.0 / 255.0,
                a: 1.0,
            },
            border_color: Rgba {
                r: 186.0 / 255.0,
                g: 194.0 / 255.0,
                b: 222.0 / 255.0,
                a: 1.0,
            },
            selected_background_color: Rgba {
                r: 69.0 / 255.0,
                g: 71.0 / 255.0,
                b: 90.0 / 255.0,
                a: 1.0,
            },
            font_family: String::from("Liberation Mono"),
            font_size: 16.0,
            window_width: 800.0,
            window_height: 400.0,
            status_bar_left: vec![],
            status_bar_center: vec![StatusItem::DateTime {
                format: "%I:%M:%S %p".to_string(),
            }],
            status_bar_right: vec![StatusItem::DateTime {
                format: "%Y-%m-%d".to_string(),
            }],
        }
    }
}

/// Intermediate struct for TOML serialization/deserialization
#[derive(Serialize, Deserialize)]
struct ConfigToml {
    text_primary_color: String,
    text_secondary_color: String,
    text_selected_primary_color: String,
    text_selected_secondary_color: String,
    background_color: String,
    border_color: String,
    selected_background_color: String,
    font_family: String,
    font_size: f32,
    window_width: f32,
    window_height: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_bar_left: Option<Vec<StatusItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_bar_center: Option<Vec<StatusItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_bar_right: Option<Vec<StatusItem>>,
}

impl From<&Config> for ConfigToml {
    fn from(config: &Config) -> Self {
        // Helper function to convert Rgba to hex string
        fn rgba_to_hex(rgba: &Rgba) -> String {
            Color::from_rgba(rgba).to_hex()
        }

        Self {
            text_primary_color: rgba_to_hex(&config.text_primary_color),
            text_secondary_color: rgba_to_hex(&config.text_secondary_color),
            text_selected_primary_color: rgba_to_hex(&config.text_selected_primary_color),
            text_selected_secondary_color: rgba_to_hex(&config.text_selected_secondary_color),
            background_color: rgba_to_hex(&config.background_color),
            border_color: rgba_to_hex(&config.border_color),
            selected_background_color: rgba_to_hex(&config.selected_background_color),
            font_family: config.font_family.clone(),
            font_size: config.font_size,
            window_width: config.window_width,
            window_height: config.window_height,
            // Convert empty vectors to None for cleaner serialization
            status_bar_left: (!config.status_bar_left.is_empty())
                .then(|| config.status_bar_left.clone()),
            status_bar_center: (!config.status_bar_center.is_empty())
                .then(|| config.status_bar_center.clone()),
            status_bar_right: (!config.status_bar_right.is_empty())
                .then(|| config.status_bar_right.clone()),
        }
    }
}

impl TryFrom<ConfigToml> for Config {
    type Error = anyhow::Error;

    fn try_from(toml: ConfigToml) -> Result<Self, Self::Error> {
        // Helper function to convert hex string to Rgba
        fn hex_to_rgba(hex: String) -> Result<Rgba, anyhow::Error> {
            Ok(Color::from_hex(&hex)?.to_rgba())
        }

        Ok(Self {
            text_primary_color: hex_to_rgba(toml.text_primary_color)?,
            text_secondary_color: hex_to_rgba(toml.text_secondary_color)?,
            text_selected_primary_color: hex_to_rgba(toml.text_selected_primary_color)?,
            text_selected_secondary_color: hex_to_rgba(toml.text_selected_secondary_color)?,
            background_color: hex_to_rgba(toml.background_color)?,
            border_color: hex_to_rgba(toml.border_color)?,
            selected_background_color: hex_to_rgba(toml.selected_background_color)?,
            font_family: toml.font_family,
            font_size: toml.font_size,
            window_width: toml.window_width,
            window_height: toml.window_height,
            status_bar_left: toml.status_bar_left.unwrap_or_default(),
            status_bar_center: toml.status_bar_center.unwrap_or_default(),
            status_bar_right: toml.status_bar_right.unwrap_or_default(),
        })
    }
}

impl Serialize for Config {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ConfigToml::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let toml = ConfigToml::deserialize(deserializer)?;
        toml.try_into().map_err(serde::de::Error::custom)
    }
}

impl Config {
    pub fn init(cx: &mut App) {
        let config = Self::load().unwrap_or_else(|e| {
            log::error!("Failed to load config: {}", e);
            Config::default()
        });
        cx.set_global(config);
    }

    /// Load configuration from disk, creating a default if none exists
    fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        let config = if !config_path.exists() {
            log::info!(
                "No config file found at {:?}, creating default config",
                config_path
            );
            Config::default()
        } else {
            log::info!("Loading config from {:?}", config_path);
            let config_str = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file at {:?}", config_path))?;

            match toml::from_str(&config_str) {
                Ok(config) => {
                    log::info!("Successfully loaded config file");
                    config
                }
                Err(e) => {
                    log::warn!("Failed to parse config file: {}", e);
                    // Backup invalid config file
                    let backup_path = config_path.with_extension("toml.bak");
                    if let Err(e) = fs::rename(&config_path, &backup_path) {
                        log::error!("Failed to backup invalid config: {}", e);
                    } else {
                        log::info!("Backed up invalid config to {:?}", backup_path);
                    }
                    Config::default()
                }
            }
        };

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory at {:?}", parent))?;
        }

        // Write config (ensures a valid config always exists)
        fs::write(&config_path, toml::to_string_pretty(&config)?)
            .with_context(|| format!("Failed to write config to {:?}", config_path))?;

        log::info!("Wrote config to {:?}", config_path);

        Ok(config)
    }

    fn config_path() -> Result<PathBuf> {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .context("Could not determine home directory")?;

        Ok(PathBuf::from(home).join(".config/crowbar/crowbar.toml"))
    }
}

impl Global for Config {}
