pub mod executable_finder;
pub mod app_finder;
pub mod desktop_entry_categories;

// Re-export commonly used items for convenience
pub use app_finder::{DesktopEntry, scan_desktopentries};
pub use executable_finder::{FileInfo, FileType, scan_path_executables};
pub use desktop_entry_categories::Category; 