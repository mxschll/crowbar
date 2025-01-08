use core::panic;
use std::fs;

use crate::copilot::Provider;

// Top level config params
#[derive(Debug, Clone)]
pub struct CrowbarConfig {
    pub copilot_options: CopilotOptions,
}

#[derive(Debug, Clone)]
pub struct CopilotOptions {
    pub provider: Option<Provider>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

fn parse_config(content: &str) -> CopilotOptions {
    let mut provider = None;
    let mut api_key = None;
    let mut model = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim();
            match key.trim() {
                "COPILOT_PROVIDER" => {
                    provider = match Provider::from_string(value) {
                        Ok(provider) => Some(provider),
                        Err(_) => None,
                    }
                }
                "COPILOT_API_KEY" => api_key = Some(value.to_string()),
                "COPILOT_MODEL" => model = Some(value.to_string()),
                _ => {}
            }
        }
    }

    CopilotOptions {
        provider,
        api_key,
        model,
    }
}

pub fn load() -> CrowbarConfig {
    let home_dir = match std::env::var("HOME") {
        Ok(path) => path,
        Err(error) => panic!("Error reading home dir {error:?}"),
    };

    let config_dir = std::path::Path::new(&home_dir)
        .join(".config")
        .join("crowbar");
    let config_file = config_dir.join("crowbar.config");

    // Create config directory if it doesn't exist
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
    }

    // Read or create config file
    let config = if config_file.exists() {
        fs::read_to_string(&config_file).expect("Failed to read config file")
    } else {
        // Create default config
        let default_config = "";
        fs::write(&config_file, default_config).expect("Failed to create default config file");
        default_config.to_string()
    };

    let copilot_options = parse_config(&config);

    CrowbarConfig { copilot_options }
}
