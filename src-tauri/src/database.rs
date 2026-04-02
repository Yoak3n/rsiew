use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Activity {
    pub id: Option<i64>,
    pub timestamp: i64,
    pub app_name: String,
    pub window_title: String,
    pub duration: i64,
    pub screenshot_path: Option<String>,
    pub ocr_text: Option<String>,
    pub exe_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppUsage {
    pub app_name: String,
    pub duration: i64,
    pub exe_path: String,
}

pub struct Database {
    pub conn: std::sync::Mutex<Connection>,
}

impl Database {
    pub fn new(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS activities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                app_name TEXT NOT NULL,
                window_title TEXT NOT NULL,
                duration INTEGER NOT NULL
            )",
            [],
        )?;

        // 动态升级表结构
        let _ = conn.execute("ALTER TABLE activities ADD COLUMN screenshot_path TEXT", []);
        let _ = conn.execute("ALTER TABLE activities ADD COLUMN ocr_text TEXT", []);
        let _ = conn.execute("ALTER TABLE activities ADD COLUMN exe_path TEXT", []);

        Ok(Database {
            conn: std::sync::Mutex::new(conn),
        })
    }

    pub fn insert_activity(&self, activity: &Activity) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // 插入时附带 exe_path
        conn.execute(
            "INSERT INTO activities (timestamp, app_name, window_title, duration, screenshot_path, ocr_text, exe_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                activity.timestamp,
                activity.app_name,
                activity.window_title,
                activity.duration,
                activity.screenshot_path,
                activity.ocr_text,
                activity.exe_path,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn merge_activity(&self, id: i64, duration_delta: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE activities SET duration = duration + ?1 WHERE id = ?2",
            params![duration_delta, id],
        )?;
        Ok(())
    }

    pub fn update_activity_ocr(&self, id: i64, ocr_text: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE activities SET ocr_text = ?1 WHERE id = ?2",
            params![ocr_text, id],
        )?;
        Ok(())
    }

    pub fn get_latest_activity(&self) -> Result<Option<Activity>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, app_name, window_title, duration, screenshot_path, ocr_text, exe_path
             FROM activities ORDER BY timestamp DESC LIMIT 1"
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Activity {
                id: Some(row.get(0)?),
                timestamp: row.get(1)?,
                app_name: row.get(2)?,
                window_title: row.get(3)?,
                duration: row.get(4)?,
                screenshot_path: row.get(5).unwrap_or(None),
                ocr_text: row.get(6).unwrap_or(None),
                exe_path: row.get(7).unwrap_or(None),
            }))
        } else {
            Ok(None)
        }
    }

    // 关键点：在查询统计的时候，使用 MAX(exe_path) 顺便把该应用最近有值的一个 exe_path 提取出来！
    pub fn get_stats_by_range(&self, start_ts: i64, end_ts: i64) -> Result<Vec<AppUsage>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT app_name, SUM(duration) as total_duration, MAX(exe_path) as exe_path
             FROM activities
             WHERE timestamp >= ?1 AND timestamp <= ?2
             GROUP BY app_name
             ORDER BY total_duration DESC"
        )?;
        let iter = stmt.query_map(params![start_ts, end_ts], |row| {
            let path: Option<String> = row.get(2).unwrap_or(None);
            Ok(AppUsage {
                app_name: row.get(0)?,
                duration: row.get(1)?,
                exe_path: path.unwrap_or_default(),
            })
        })?;

        let mut stats = Vec::new();
        for item in iter {
            stats.push(item?);
        }
        Ok(stats)
    }

    pub fn cleanup_old_records(&self, before_ts: i64) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        // 1. 先找出即将被删除的记录里的所有截图路径
        let mut stmt = conn.prepare(
            "SELECT screenshot_path FROM activities WHERE timestamp < ?1 AND screenshot_path IS NOT NULL"
        )?;
        let paths_iter = stmt.query_map(params![before_ts], |row| row.get(0))?;
        
        let mut paths_to_delete = Vec::new();
        for path in paths_iter {
            if let Ok(Some(p)) = path {
                paths_to_delete.push(p);
            }
        }

        // 2. 从数据库中彻底删除这些旧记录
        conn.execute("DELETE FROM activities WHERE timestamp < ?1", params![before_ts])?;
        
        Ok(paths_to_delete)
    }
}