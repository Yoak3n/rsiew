use crate::core::error::*;
use chrono::{Local, MappedLocalTime, NaiveDateTime, TimeZone};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

fn safe_local_timestamp(ndt: NaiveDateTime) -> i64 {
    match Local.from_local_datetime(&ndt) {
        MappedLocalTime::Single(dt) => dt.timestamp(),
        MappedLocalTime::Ambiguous(dt, _) => dt.timestamp(),
        MappedLocalTime::None => {
            // DST 跳变导致该本地时间不存在，向前偏移1小时
            let shifted = ndt + chrono::Duration::hours(1);
            Local
                .from_local_datetime(&shifted)
                .earliest()
                .map(|dt| dt.timestamp())
                .unwrap_or_else(|| ndt.and_utc().timestamp())
        }
    }
}

/// 规范化 URL（用于合并判断）
/// 移除末尾斜杠、规范化空白字符
pub fn normalize_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Activity {
    pub id: Option<i64>,
    pub timestamp: i64,
    pub app_name: String,
    pub window_title: String,
    pub duration: i64,
    pub category: String,
    #[serde(default)]
    pub browser_url: Option<String>,
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
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS activities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                app_name TEXT NOT NULL,
                window_title TEXT NOT NULL,
                duration INTEGER NOT NULL,
                category TEXT NOT NULL,
                browser_url TEXT
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

    pub fn insert_activity(&self, activity: &Activity) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        // 插入时附带 exe_path
        conn.execute(
            "INSERT INTO activities (timestamp, app_name, window_title, duration, category, browser_url, screenshot_path, ocr_text, exe_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                activity.timestamp,
                activity.app_name,
                activity.window_title,
                activity.duration,
                activity.category,
                activity.browser_url,
                activity.screenshot_path,
                activity.ocr_text,
                activity.exe_path,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 合并活动：累加时长、追加OCR、更新截图路径
    pub fn merge_activity(
        &self,
        id: i64,
        duration_delta: i64,
        new_ocr: Option<&str>,
        _new_screenshot_path: &str,
        new_timestamp: i64,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            AppError::Database(rusqlite::Error::InvalidParameterName(e.to_string()))
        })?;

        // 获取现有的 OCR 内容
        let existing_ocr: Option<String> = conn
            .query_row(
                "SELECT ocr_text FROM activities WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok();

        // 合并 OCR：追加新内容
        let merged_ocr = match (existing_ocr, new_ocr) {
            (Some(existing), Some(new)) if !new.is_empty() => {
                // 追加新内容，用分隔符隔开
                Some(format!("{existing}\n---\n{new}"))
            }
            (Some(existing), _) => Some(existing),
            (None, Some(new)) => Some(new.to_string()),
            (None, None) => None,
        };

        conn.execute(
            "UPDATE activities 
             SET duration = duration + ?1, 
                 ocr_text = ?2, 
                 timestamp = ?3
             WHERE id = ?4",
            params![duration_delta, merged_ocr, new_timestamp, id],
        )?;

        Ok(())
    }

    pub fn update_activity_ocr(&self, id: i64, ocr_text: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE activities SET ocr_text = ?1 WHERE id = ?2",
            params![ocr_text, id],
        )?;
        Ok(())
    }

    pub fn update_activity_screenshot(&self, id: i64, screenshot_path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE activities SET screenshot_path = ?1 WHERE id = ?2",
            params![screenshot_path, id],
        )?;
        Ok(())
    }

    pub fn get_latest_activity(&self) -> Result<Option<Activity>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, app_name, window_title, duration, category, browser_url, screenshot_path, ocr_text, exe_path
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
                category: row.get(5)?,
                browser_url: row.get(6).unwrap_or(None),
                screenshot_path: row.get(7).unwrap_or(None),
                ocr_text: row.get(8).unwrap_or(None),
                exe_path: row.get(9).unwrap_or(None),
            }))
        } else {
            Ok(None)
        }
    }

    /// 获取指定应用 + 窗口标题今天的最近一条活动记录
    /// 当浏览器 URL 暂时不可用时，用于避免不同标签页互相串时长
    pub fn get_latest_activity_by_app_title(
        &self,
        app_name: &str,
        window_title: &str,
    ) -> Result<Option<Activity>> {
        let conn = self.conn.lock().map_err(|e| {
            AppError::Database(rusqlite::Error::InvalidParameterName(e.to_string()))
        })?;

        let today_start = {
            let now = chrono::Local::now();
            let ndt = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
            safe_local_timestamp(ndt)
        };

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, app_name, window_title, screenshot_path, ocr_text, category, duration, browser_url, exe_path
             FROM activities
             WHERE app_name = ?1 AND window_title = ?2 AND timestamp >= ?3
             ORDER BY id DESC
             LIMIT 1"
        )?;

        let mut rows = stmt.query(params![app_name, window_title, today_start])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Activity {
                id: Some(row.get(0)?),
                timestamp: row.get(1)?,
                app_name: row.get(2)?,
                window_title: row.get(3)?,
                screenshot_path: row.get(4)?,
                ocr_text: row.get(5)?,
                category: row.get(6)?,
                duration: row.get(7)?,
                browser_url: row.get(8)?,
                exe_path: row.get(9)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// 按 URL 获取今天的活动记录（用于浏览器 URL 合并）
    /// 使用规范化 URL 进行匹配，解决末尾斜杠差异问题
    pub fn get_latest_activity_by_url(&self, browser_url: &str) -> Result<Option<Activity>> {
        let conn = self.conn.lock().map_err(|e| {
            AppError::Database(rusqlite::Error::InvalidParameterName(e.to_string()))
        })?;

        let today_start = {
            let now = chrono::Local::now();
            let ndt = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
            safe_local_timestamp(ndt)
        };

        // 规范化输入 URL
        let normalized_url = normalize_url(browser_url);
        log::debug!("URL 合并查询: 原始='{browser_url}', 规范化='{normalized_url}'");

        // 使用 RTRIM 规范化数据库中的 URL 进行比较
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, app_name, window_title, screenshot_path, ocr_text, category, duration, browser_url, exe_path
             FROM activities
             WHERE RTRIM(browser_url, '/') = ?1 AND timestamp >= ?2
             ORDER BY id DESC
             LIMIT 1"
        )?;

        let mut rows = stmt.query(params![normalized_url, today_start])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Activity {
                id: Some(row.get(0)?),
                timestamp: row.get(1)?,
                app_name: row.get(2)?,
                window_title: row.get(3)?,
                screenshot_path: row.get(4)?,
                ocr_text: row.get(5)?,
                category: row.get(6)?,
                duration: row.get(7)?,
                browser_url: row.get(8)?,
                exe_path: row.get(9)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// 获取指定应用今天的最近一条活动记录（用于合并判断）
    pub fn get_latest_activity_by_app(&self, app_name: &str) -> Result<Option<Activity>> {
        let conn = self.conn.lock().map_err(|e| {
            AppError::Database(rusqlite::Error::InvalidParameterName(e.to_string()))
        })?;

        // 获取今天的开始时间戳（当天 00:00:00）
        let today_start = {
            let now = chrono::Local::now();
            let ndt = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
            safe_local_timestamp(ndt)
        };

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, app_name, window_title, screenshot_path, ocr_text, category, duration, browser_url, exe_path
             FROM activities
             WHERE app_name = ?1 AND timestamp >= ?2
             ORDER BY id DESC
             LIMIT 1"
        )?;

        let mut rows = stmt.query(params![app_name, today_start])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Activity {
                id: Some(row.get(0)?),
                timestamp: row.get(1)?,
                app_name: row.get(2)?,
                window_title: row.get(3)?,
                screenshot_path: row.get(4)?,
                ocr_text: row.get(5)?,
                category: row.get(6)?,
                duration: row.get(7)?,
                browser_url: row.get(8)?,
                exe_path: row.get(9)?,
            }))
        } else {
            Ok(None)
        }
    }

    // 关键点：在查询统计的时候，使用 MAX(exe_path) 顺便把该应用最近有值的一个 exe_path 提取出来！
    pub fn get_stats_by_range(&self, start_ts: i64, end_ts: i64) -> Result<Vec<AppUsage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT app_name, SUM(duration) as total_duration, MAX(exe_path) as exe_path
             FROM activities
             WHERE timestamp >= ?1 AND timestamp <= ?2
             GROUP BY app_name
             ORDER BY total_duration DESC",
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

    pub fn cleanup_old_records(&self, before_ts: i64) -> Result<Vec<String>> {
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
        conn.execute(
            "DELETE FROM activities WHERE timestamp < ?1",
            params![before_ts],
        )?;

        Ok(paths_to_delete)
    }
}
