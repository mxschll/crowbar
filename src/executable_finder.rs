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
use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Common Unix user-specific executable paths that might not be in PATH
const ADDITIONAL_UNIX_PATHS: &[&str] = &["~/.local/bin", "~/bin"];

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
    let paths = std::env::var("PATH").unwrap_or_default();
    let mut executables = Vec::new();
    let mut seen_paths = HashSet::new();

    // Scan PATH directories
    for dir in paths.split(':') {
        let _ = scan_directory(Path::new(dir), &mut executables, &mut seen_paths);
    }

    // Scan additional user-specific directories
    for dir in get_additional_paths() {
        let _ = scan_directory(&dir, &mut executables, &mut seen_paths);
    }

    executables.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(executables)
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
    let entries = fs::read_dir(dir)?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        let canonical_path = fs::canonicalize(&path)?;
        if seen_paths.contains(&canonical_path) {
            continue;
        }

        if is_executable(&path)? {
            if let Some(exe_info) = get_executable_info(&path)? {
                seen_paths.insert(canonical_path.clone());
                executables.push(exe_info);
            }
        }
    }

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
