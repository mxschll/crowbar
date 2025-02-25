//! Module for scanning and parsing desktop entry files on Unix-like systems.
//!
//! This module provides functionality to find and parse `.desktop` files from
//! standard system locations, extracting application information such as name,
//! executable path, and icon location.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use crate::common::expand_tilde;
use super::desktop_entry_categories::Category;

const DESKTOP_ENTRIES_UNIX_PATHS: &[&'static str] = &[
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

// https://specifications.freedesktop.org/desktop-entry-spec/latest/exec-variables.html
const DESKTOP_ENTRY_FIELD_CODES: &[&'static str] = &[
    "%f", // Single file name
    "%F", // A list of files
    "%u", // A single URL
    "%U", // A list of URLs
    "%d", // Deprecated
    "%D", // Deprecated
    "%n", // Deprecated
    "%N", // Deprecated
    "%i", // The Icon key of the desktop entry expanded as two arguments, first --icon and then the value of the Icon key.
    "%c", // The translated name of the application as listed in the appropriate Name key in the desktop entry
    "%k", // The location of the desktop file
    "%v", // Deprecated
    "%m", // Deprecated
];

pub const ARGUMENT_FIELD_CODES: &[&str] = &["%f", "%F", "%u", "%U"];

/// Represents information about a desktop application
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub filename: String,
    pub takes_args: bool,
    pub categories: Vec<Category>,
}

/// Scan system directories for desktop entries and return a list of valid applications
pub fn scan_desktopentries() -> Vec<DesktopEntry> {
    DESKTOP_ENTRIES_UNIX_PATHS
        .iter()
        .flat_map(|path| {
            let expanded_path = expand_tilde(path);
            let mut apps = Vec::new();
            scan_directory(&expanded_path, &mut apps);
            apps
        })
        .collect()
}

fn scan_directory(dir: &PathBuf, apps: &mut Vec<DesktopEntry>) {
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

/// Parse a desktop entry file and return application information if valid
fn parse_desktop_file(path: &PathBuf) -> Option<DesktopEntry> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let filename = path.file_name()?.to_string_lossy().into_owned();

    let mut name = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut type_entry = String::new();
    let mut categories = Vec::new();
    let mut in_desktop_entry = false;

    for line in reader.lines().flatten() {
        let line = line.trim();

        match line {
            "[Desktop Entry]" => in_desktop_entry = true,
            line if line.starts_with('[') => in_desktop_entry = false,
            line if in_desktop_entry => {
                if let Some((key, value)) = line.split_once('=') {
                    match key.trim() {
                        "Name" => name = value.trim().to_string(),
                        "Exec" => exec = value.trim().to_string(),
                        "Icon" => icon = value.trim().to_string(),
                        "Type" => type_entry = value.trim().to_string(),
                        "Categories" => {
                            categories = value
                                .split(';')
                                .filter(|s| !s.is_empty())
                                .filter_map(|s| Category::from_str(s.trim()))
                                .collect();
                        }
                        _ => {}
                    }
                }
            }
            _ => continue,
        }
    }

    if type_entry != "Application" || name.is_empty() || exec.is_empty() {
        return None;
    }

    // Only enable takes_args for web browsers
    let takes_args = categories
        .iter()
        .any(|cat| matches!(cat, Category::WebBrowser))
        && ARGUMENT_FIELD_CODES.iter().any(|&code| {
            // Split exec by whitespace and check if any part exactly matches the field code
            exec.split_whitespace().any(|part| part == code)
        });
    let exec = DESKTOP_ENTRY_FIELD_CODES
        .iter()
        .fold(exec, |acc, &code| acc.replace(code, ""))
        .trim()
        .to_string();

    Some(DesktopEntry {
        name,
        exec,
        icon,
        filename,
        takes_args,
        categories,
    })
}
