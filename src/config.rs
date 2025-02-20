use std::{env, fs, path::PathBuf};

use anyhow::{Result};
use gpui::{App, Global, Rgba};
use serde::{Deserialize, Serialize};
use toml;

#[derive(Clone, Copy, Serialize, Deserialize)]
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
}

impl From<String> for Color {
    fn from(hex: String) -> Self {
        let hex = hex.trim_start_matches('#');
        Self {
            r: u8::from_str_radix(&hex[0..2], 16).unwrap_or(0),
            g: u8::from_str_radix(&hex[2..4], 16).unwrap_or(0),
            b: u8::from_str_radix(&hex[4..6], 16).unwrap_or(0),
        }
    }
}

impl From<Color> for String {
    fn from(color: Color) -> Self {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    }
}


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
}

impl Default for Config{
    fn default() -> Self {
        Self {
            text_primary_color: Rgba { r: 186.0/255.0, g: 194.0/255.0, b: 222.0/255.0, a: 1.0 },
            text_secondary_color: Rgba { r: 186.0/255.0, g: 194.0/255.0, b: 222.0/255.0, a: 1.0 },
            text_selected_primary_color: Rgba { r: 186.0/255.0, g: 194.0/255.0, b: 222.0/255.0, a: 1.0 },
            text_selected_secondary_color: Rgba { r: 186.0/255.0, g: 194.0/255.0, b: 222.0/255.0, a: 1.0 },
            background_color: Rgba { r: 30.0/255.0, g: 31.0/255.0, b: 47.0/255.0, a: 1.0 },
            border_color: Rgba { r: 186.0/255.0, g: 194.0/255.0, b: 222.0/255.0, a: 1.0 },
            selected_background_color: Rgba { r: 69.0/255.0, g: 71.0/255.0, b: 90.0/255.0, a: 1.0 },
            font_family: String::from("Liberation Mono"),
            font_size: 16.0,
            window_width: 800.0,
            window_height: 400.0,
        }
    }
}

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
}

impl From<&Config> for ConfigToml {
    fn from(config: &Config) -> Self {
        Self {
            text_primary_color: format!("#{:02x}{:02x}{:02x}", 
                (config.text_primary_color.r * 255.0) as u8,
                (config.text_primary_color.g * 255.0) as u8,
                (config.text_primary_color.b * 255.0) as u8),
            text_secondary_color: format!("#{:02x}{:02x}{:02x}",
                (config.text_secondary_color.r * 255.0) as u8,
                (config.text_secondary_color.g * 255.0) as u8,
                (config.text_secondary_color.b * 255.0) as u8),
            text_selected_primary_color: format!("#{:02x}{:02x}{:02x}",
                (config.text_selected_primary_color.r * 255.0) as u8,
                (config.text_selected_primary_color.g * 255.0) as u8,
                (config.text_selected_primary_color.b * 255.0) as u8),
            text_selected_secondary_color: format!("#{:02x}{:02x}{:02x}",
                (config.text_selected_secondary_color.r * 255.0) as u8,
                (config.text_selected_secondary_color.g * 255.0) as u8,
                (config.text_selected_secondary_color.b * 255.0) as u8),
            background_color: format!("#{:02x}{:02x}{:02x}",
                (config.background_color.r * 255.0) as u8,
                (config.background_color.g * 255.0) as u8,
                (config.background_color.b * 255.0) as u8),
            border_color: format!("#{:02x}{:02x}{:02x}",
                (config.border_color.r * 255.0) as u8,
                (config.border_color.g * 255.0) as u8,
                (config.border_color.b * 255.0) as u8),
            selected_background_color: format!("#{:02x}{:02x}{:02x}",
                (config.selected_background_color.r * 255.0) as u8,
                (config.selected_background_color.g * 255.0) as u8,
                (config.selected_background_color.b * 255.0) as u8),
            font_family: config.font_family.clone(),
            font_size: config.font_size,
            window_width: config.window_width,
            window_height: config.window_height,
        }
    }
}

impl TryFrom<ConfigToml> for Config {
    type Error = anyhow::Error;

    fn try_from(toml: ConfigToml) -> Result<Self, Self::Error> {
        Ok(Self {
            text_primary_color: Color::from(toml.text_primary_color).to_rgba(),
            text_secondary_color: Color::from(toml.text_secondary_color).to_rgba(),
            text_selected_primary_color: Color::from(toml.text_selected_primary_color).to_rgba(),
            text_selected_secondary_color: Color::from(toml.text_selected_secondary_color).to_rgba(),
            background_color: Color::from(toml.background_color).to_rgba(),
            border_color: Color::from(toml.border_color).to_rgba(),
            selected_background_color: Color::from(toml.selected_background_color).to_rgba(),
            font_family: toml.font_family,
            font_size: toml.font_size,
            window_width: toml.window_width,
            window_height: toml.window_height,
        })
    }
}

impl Serialize for Config {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        ConfigToml::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>
    {
        let toml = ConfigToml::deserialize(deserializer)?;
        toml.try_into().map_err(serde::de::Error::custom)
    }
}

impl Config {
    pub fn init(cx: &mut App) -> Result<()> {
        let config = Self::load()?;
        cx.set_global(config);
        Ok(())
    }

    fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        let config = if !config_path.exists() {
            Config::default()
        } else {
            match toml::from_str(&fs::read_to_string(&config_path)?) {
                Ok(config) => config,
                Err(_) => Config::default(),
            }
        };
        
        fs::create_dir_all(config_path.parent().unwrap())?;
        fs::write(&config_path, toml::to_string_pretty(&config)?)?;
        
        Ok(config)
    }

    fn config_path() -> Result<PathBuf> {
        let home = env::var("HOME").or_else(|_| env::var("USERPROFILE"))?;
        Ok(PathBuf::from(home).join(".config/crowbar/crowbar.config"))
    }
}

impl Global for Config {}
