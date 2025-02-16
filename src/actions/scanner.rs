use crate::app_finder::scan_desktopentries;
use crate::database::Database;
use crate::executable_finder::scan_path_executables;
use log::info;
use rusqlite::Connection;

pub struct ActionScanner;

impl ActionScanner {
    pub fn needs_scan(conn: &Connection) -> bool {
        // Check if we have any program or desktop entries
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM actions WHERE action_type IN ('program', 'desktop')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        count == 0
    }

    pub fn scan_system(db: &Database) {
        info!("Starting system scan for actions");
        let scan_start = std::time::Instant::now();

        info!("Starting executable scan");
        let exec_start = std::time::Instant::now();
        let executables = scan_path_executables().unwrap_or_default();
        info!("Executable scan took {:?}", exec_start.elapsed());

        info!("Starting to insert executables");
        executables.iter().for_each(|elem| {
            let _ = db.insert_binary(&elem.name, &elem.path.to_string_lossy());
        });

        let applications = scan_desktopentries();
        applications.iter().for_each(|elem| {
            let _ = db.insert_application(&elem.name, &elem.exec);
        });

        info!("System scan completed in {:?}", scan_start.elapsed());
    }
}

