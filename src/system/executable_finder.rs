//! Finds and analyzes executable files in PATH
//!
//! Scans PATH for executables and identifies their type (ELF, Mach-O, scripts) using magic numbers.
//!
//! ```no_run
//! let executables = scan_path_executables().unwrap();
//! for exe in executables {
//!     println!("{} at {:?}: {:?}", exe.name, exe.path, exe.file_type);
//! }
//! ```

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Instant;
use log::info;

use crate::common::expand_tilde;

/// Common Unix user-specific executable paths that might not be in PATH
const ADDITIONAL_UNIX_PATHS: &[&str] = &["~/.local/bin", "~/bin", "/snap/bin/"];

const MAGIC_NUMBERS: &[(FileType, &[u8])] = &[
    (FileType::Elf, &[0x7f, 0x45, 0x4c, 0x46]),
    (FileType::MachO, &[0xfe, 0xed, 0xfa, 0xce]),
    (FileType::MachO, &[0xfe, 0xed, 0xfa, 0xcf]),
    (FileType::Script, b"#!"),
];

/// Details of an executable file including name, path, and type
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub path: PathBuf,
    pub file_type: FileType,
}

/// Executable types identified by magic numbers
#[derive(Debug, Clone)]
pub enum FileType {
    /// ELF format (Linux)
    Elf,
    /// Mach-O format (macOS)
    MachO,
    /// Shell or other script
    Script,
    /// Other executable format
    Other,
}

/// Scans PATH for executables and identifies their types
///
/// # Returns
/// - `Ok(Vec<FileInfo>)`: Sorted list of executables
/// - `Err(io::Error)`: If reading fails
///
/// # TODO
/// Track all symlink names pointing to each executable
pub fn scan_path_executables() -> io::Result<Vec<FileInfo>> {
    let start = Instant::now();
    info!("Starting PATH executable scan");
    
    let mut executables = Vec::new();
    let mut seen_paths = HashSet::new();

    // Scan PATH
    if let Some(path) = std::env::var_os("PATH") {
        let path_start = Instant::now();
        for dir in std::env::split_paths(&path) {
            let dir_start = Instant::now();
            if let Err(e) = scan_directory(&dir, &mut executables, &mut seen_paths) {
                info!("Error scanning directory {:?}: {}", dir, e);
            }
            info!("Scanning directory {:?} took {:?}", dir, dir_start.elapsed());
        }
        info!("Scanning PATH directories took {:?}", path_start.elapsed());
    }

    // Scan additional Unix paths
    let additional_start = Instant::now();
    for path in get_additional_paths() {
        let path_start = Instant::now();
        if let Err(e) = scan_directory(&path, &mut executables, &mut seen_paths) {
            info!("Error scanning additional path {:?}: {}", path, e);
        }
        info!("Scanning additional path {:?} took {:?}", path, path_start.elapsed());
    }
    info!("Scanning additional paths took {:?}", additional_start.elapsed());

    info!("Total executable scan took {:?}, found {} executables", start.elapsed(), executables.len());
    Ok(executables)
}

/// Gets a list of additional directories to scan, including user-specific paths
fn get_additional_paths() -> Vec<PathBuf> {
    ADDITIONAL_UNIX_PATHS
        .iter()
        .map(|&path| expand_tilde(path))
        .collect()
}

/// Scans one directory for executables, avoiding duplicates
fn scan_directory(
    dir: &Path,
    executables: &mut Vec<FileInfo>,
    seen_paths: &mut HashSet<PathBuf>,
) -> io::Result<()> {
    let start = Instant::now();
    
    if !dir.is_dir() {
        return Ok(());
    }

    let read_start = Instant::now();
    let entries = fs::read_dir(dir)?;
    info!("Reading directory {:?} took {:?}", dir, read_start.elapsed());

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if seen_paths.contains(&path) {
            continue;
        }
        seen_paths.insert(path.clone());

        if let Ok(Some(info)) = get_executable_info(&path) {
            executables.push(info);
        }
    }

    info!("Scanning directory {:?} completed in {:?}", dir, start.elapsed());
    Ok(())
}

/// Checks if file is executable (has execute bits set and is readable)
fn is_executable(path: &PathBuf) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;

    // Check if it's a regular file or symlink
    if !metadata.is_file() && !metadata.file_type().is_symlink() {
        return Ok(false);
    }

    let mode = metadata.permissions().mode();

    // Check if any execute bit is set (user, group, or others)
    // Also check if we have read permission to actually read the file
    Ok((mode & 0o111 != 0) && (mode & 0o444 != 0))
}

/// Gets executable type by reading magic numbers and creates FileInfo
fn get_executable_info(path: &PathBuf) -> io::Result<Option<FileInfo>> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 4];

    if file.read_exact(&mut buffer).is_err() {
        return Ok(None);
    }

    let file_type = MAGIC_NUMBERS
        .iter()
        .find(|(_, magic)| buffer.starts_with(magic))
        .map(|(ft, _)| ft.clone())
        .unwrap_or(FileType::Other);

    let canonical = fs::canonicalize(path)?;
    Ok(Some(FileInfo {
        name: canonical
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        path: canonical,
        file_type,
    }))
}
