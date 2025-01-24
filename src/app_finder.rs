use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

const DESKTOPENTRIES_UNIX_PATHS: &[&str] = &[
    "~/.local/share/applications",         // User-specific applications
    "/usr/share/applications",             // System-wide applications
    "/usr/local/share/applications",       // Locally installed applications
    "/var/lib/snapd/desktop/applications", // Snap applications
    "/var/lib/flatpak/exports/share/applications", // Flatpak applications
    "~/.var/app/*/desktop",                // Per-user Flatpak applications
    "/opt/*/share/applications",           // Applications installed in /opt
    "/usr/share/gnome/applications",       // GNOME-specific applications
    "/usr/share/kde4/applications",        // KDE4 applications
    "/usr/share/kde/applications",         // KDE applications
];

#[derive(Debug, Clone)]
struct AppInfo {
    name: String,
    exec: String,
    icon: String,
    filename: String,
}

/// Expands the tilde (~) in paths to the user's home directory
fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(path.replacen('~', &home, 1));
        }
    }
    PathBuf::from(path)
}

fn scan_desktopentries() -> Vec<AppInfo> {
    let mut apps = Vec::new();

    for path in DESKTOPENTRIES_UNIX_PATHS {
        let expanded_path = expand_tilde(path);
        scan_directory(&expanded_path, &mut apps);
    }

    apps
}

fn scan_directory(dir: &PathBuf, apps: &mut Vec<AppInfo>) {
    if !dir.exists() {
        return;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                if let Some(app_info) = parse_desktop_file(&path) {
                    apps.push(app_info);
                }
            }
        }
    }
}

fn parse_desktop_file(path: &PathBuf) -> Option<AppInfo> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut name = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let filename = path.file_name()?.to_string_lossy().into_owned();

    let mut in_desktop_entry = false;

    for line in reader.lines().flatten() {
        let line = line.trim();

        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        } else if line.starts_with('[') {
            in_desktop_entry = false;
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Name" => name = value.trim().to_string(),
                "Exec" => exec = value.trim().to_string(),
                "Icon" => icon = value.trim().to_string(),
                _ => {}
            }
        }
    }

    if name.is_empty() || exec.is_empty() {
        return None;
    }

    Some(AppInfo {
        name,
        exec,
        icon,
        filename,
    })
}
