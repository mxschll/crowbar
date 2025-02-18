use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Action {
    pub id: i64,
    pub name: String,
    pub action_type: String,
}

#[derive(Debug)]
pub struct ProgramItem {
    pub id: i64,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct DesktopItem {
    pub id: i64,
    pub name: String,
    pub exec: String,
    pub accepts_args: bool,
}

impl Action {
    pub fn insert(conn: &Connection, name: &str, action_type: &str) -> Result<i64> {
        // Create a searchable name by removing special chars and converting to lowercase
        let searchname = name
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .to_lowercase();

        conn.execute(
            "INSERT OR IGNORE INTO actions (name, searchname, action_type) VALUES (?1, ?2, ?3)",
            (name, &searchname, action_type),
        )?;

        let id = conn.query_row(
            "SELECT id FROM actions WHERE name = ?1 AND action_type = ?2",
            (name, action_type),
            |row| row.get(0),
        )?;

        Ok(id)
    }
}

impl ProgramItem {
    pub fn insert(conn: &Connection, name: &str, path: &str) -> Result<i64> {
        let action_id = Action::insert(conn, name, "program")?;

        conn.execute(
            "INSERT OR IGNORE INTO program_items (id, name, path) VALUES (?1, ?2, ?3)",
            (action_id, name, path),
        )?;

        Ok(action_id)
    }
}

impl DesktopItem {
    pub fn insert(conn: &Connection, name: &str, exec: &str, accepts_args: bool) -> Result<i64> {
        let action_id = Action::insert(conn, name, "desktop")?;

        conn.execute(
            "INSERT OR IGNORE INTO desktop_items (id, name, exec, accepts_args) VALUES (?1, ?2, ?3, ?4)",
            (action_id, name, exec, accepts_args),
        )?;

        Ok(action_id)
    }
}

