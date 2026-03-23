use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

use crate::error::{AppError, Result};

const SENSITIVE_FIELDS: &[&str] = &[
    "password",
    "token",
    "secret",
    "credential",
    "api_key",
    "private_key",
    "auth_token",
];

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const MAX_ROTATED_FILES: u32 = 5;

#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub cloud: String,
    pub user: String,
    pub project: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub resource_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    pub result: AuditResult,
}

#[derive(Debug, Serialize)]
pub struct AuditResultEntry {
    pub timestamp: String,
    pub resource_id: String,
    pub result: AuditResult,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
    Initiated,
    Success,
    Failed(String),
}

pub struct AuditLogger {
    log_path: PathBuf,
    writer: Mutex<BufWriter<File>>,
}

impl AuditLogger {
    /// Open (or create) audit log file in append mode.
    pub fn new(log_path: PathBuf) -> Result<Self> {
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Other(format!("Failed to create audit log directory: {e}"))
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| AppError::Other(format!("Failed to open audit log: {e}")))?;
        Ok(Self {
            log_path,
            writer: Mutex::new(BufWriter::new(file)),
        })
    }

    /// Log a CUD action initiation. Masks sensitive fields in details.
    pub fn log_entry(&self, mut entry: AuditEntry) -> Result<()> {
        if let Some(ref mut details) = entry.details {
            Self::mask_sensitive(details);
        }
        let line = serde_json::to_string(&entry)
            .map_err(|e| AppError::Other(format!("Failed to serialize audit entry: {e}")))?;
        self.write_line(&line)
    }

    /// Log a CUD action result (success or failure) for a previously initiated action.
    pub fn log_result(
        &self,
        resource_id: &str,
        result: AuditResult,
        timestamp: &str,
    ) -> Result<()> {
        let entry = AuditResultEntry {
            timestamp: timestamp.to_string(),
            resource_id: resource_id.to_string(),
            result,
        };
        let line = serde_json::to_string(&entry)
            .map_err(|e| AppError::Other(format!("Failed to serialize audit result: {e}")))?;
        self.write_line(&line)
    }

    /// Write a JSON line to the audit log. Propagates lock and IO errors.
    fn write_line(&self, line: &str) -> Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|e| AppError::Other(format!("Audit log lock poisoned: {e}")))?;
        writeln!(writer, "{line}")
            .map_err(|e| AppError::Other(format!("Failed to write audit log: {e}")))?;
        writer
            .flush()
            .map_err(|e| AppError::Other(format!("Failed to flush audit log: {e}")))?;
        Ok(())
    }

    /// Rotate log if size exceeds MAX_LOG_SIZE.
    pub fn rotate_if_needed(&self) -> Result<()> {
        let size = match fs::metadata(&self.log_path) {
            Ok(m) => m.len(),
            Err(_) => return Ok(()),
        };
        if size < MAX_LOG_SIZE {
            return Ok(());
        }
        for i in (1..MAX_ROTATED_FILES).rev() {
            let from = rotated_path(&self.log_path, i);
            let to = rotated_path(&self.log_path, i + 1);
            if from.exists() {
                fs::rename(&from, &to).map_err(|e| {
                    AppError::Other(format!("Failed to rotate audit log {from:?} -> {to:?}: {e}"))
                })?;
            }
        }
        let rotated = rotated_path(&self.log_path, 1);
        let mut writer = self
            .writer
            .lock()
            .map_err(|e| AppError::Other(format!("Audit log lock poisoned: {e}")))?;
        writer
            .flush()
            .map_err(|e| AppError::Other(format!("Failed to flush before rotation: {e}")))?;
        fs::rename(&self.log_path, &rotated).map_err(|e| {
            AppError::Other(format!("Failed to rotate current audit log: {e}"))
        })?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| AppError::Other(format!("Failed to reopen audit log after rotation: {e}")))?;
        *writer = BufWriter::new(file);
        Ok(())
    }

    /// Mask sensitive fields in a serde_json::Value recursively.
    fn mask_sensitive(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    let key_lower = key.to_lowercase();
                    if SENSITIVE_FIELDS.iter().any(|f| key_lower.contains(f)) {
                        *val = serde_json::Value::String("****".to_string());
                    } else {
                        Self::mask_sensitive(val);
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Self::mask_sensitive(item);
                }
            }
            _ => {}
        }
    }
}

fn rotated_path(base: &Path, index: u32) -> PathBuf {
    let name = base
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    base.with_file_name(format!("{name}.{index}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Read as _;
    use tempfile::TempDir;

    fn temp_logger() -> (AuditLogger, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("audit.log");
        let logger = AuditLogger::new(path).unwrap();
        (logger, dir)
    }

    fn sample_entry() -> AuditEntry {
        AuditEntry {
            timestamp: "2026-03-23T14:30:00+09:00".to_string(),
            cloud: "prod".to_string(),
            user: "admin".to_string(),
            project: Some("infra".to_string()),
            action: "DELETE_SERVER".to_string(),
            resource_type: "server".to_string(),
            resource_id: "abc-123".to_string(),
            resource_name: Some("web-01".to_string()),
            details: None,
            result: AuditResult::Initiated,
        }
    }

    #[test]
    fn test_log_entry_writes_json_line() {
        let (logger, _dir) = temp_logger();
        logger.log_entry(sample_entry()).unwrap();

        let mut content = String::new();
        File::open(&logger.log_path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["action"], "DELETE_SERVER");
        assert_eq!(parsed["cloud"], "prod");
    }

    #[test]
    fn test_log_entry_masks_sensitive_details() {
        let (logger, _dir) = temp_logger();
        let mut entry = sample_entry();
        entry.details = Some(json!({
            "password": "my-secret",
            "name": "web-01",
            "nested": {
                "auth_token": "tok-123"
            }
        }));
        logger.log_entry(entry).unwrap();

        let mut content = String::new();
        File::open(&logger.log_path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["details"]["password"], "****");
        assert_eq!(parsed["details"]["name"], "web-01");
        assert_eq!(parsed["details"]["nested"]["auth_token"], "****");
    }

    #[test]
    fn test_multiple_entries_are_newline_separated() {
        let (logger, _dir) = temp_logger();
        logger.log_entry(sample_entry()).unwrap();
        logger.log_entry(sample_entry()).unwrap();

        let mut content = String::new();
        File::open(&logger.log_path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_creates_parent_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subdir/nested/audit.log");
        let logger = AuditLogger::new(path.clone()).unwrap();
        logger.log_entry(sample_entry()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_rotate_if_needed_no_op_when_small() {
        let (logger, _dir) = temp_logger();
        logger.log_entry(sample_entry()).unwrap();
        logger.rotate_if_needed().unwrap();
        assert!(!rotated_path(&logger.log_path, 1).exists());
    }

    #[test]
    fn test_mask_sensitive_recursive() {
        let mut val = json!({
            "params": [
                {"password": "p1", "user": "u1"},
                {"api_key": "k1", "host": "h1"}
            ],
            "nested": {
                "token": "tok-123",
                "name": "test"
            }
        });
        AuditLogger::mask_sensitive(&mut val);
        assert_eq!(val["params"][0]["password"], "****");
        assert_eq!(val["params"][0]["user"], "u1");
        assert_eq!(val["params"][1]["api_key"], "****");
        assert_eq!(val["params"][1]["host"], "h1");
        assert_eq!(val["nested"]["token"], "****");
        assert_eq!(val["nested"]["name"], "test");
    }

    #[test]
    fn test_log_result_2phase() {
        let (logger, _dir) = temp_logger();
        logger.log_entry(sample_entry()).unwrap();
        logger
            .log_result("abc-123", AuditResult::Success, "2026-03-23T14:30:02+09:00")
            .unwrap();

        let mut content = String::new();
        File::open(&logger.log_path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        let result_line: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(result_line["resource_id"], "abc-123");
        assert_eq!(result_line["result"], "success");
    }
}
