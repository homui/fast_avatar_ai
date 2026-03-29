use std::{
  fs::{self, OpenOptions},
  io::Write,
  path::{Path, PathBuf},
  sync::OnceLock,
  time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init(repo_root: &Path) -> Result<PathBuf> {
  let log_dir = repo_root.join("logs");
  fs::create_dir_all(&log_dir).context("failed to create logs directory")?;
  let log_path = log_dir.join("app.log");
  LOG_PATH.get_or_init(|| log_path.clone());
  write_line("INFO", "logger initialized")?;
  Ok(log_path)
}

pub fn path() -> Option<PathBuf> {
  LOG_PATH.get().cloned()
}

pub fn write_line(level: &str, message: &str) -> Result<()> {
  let Some(path) = LOG_PATH.get() else {
    return Ok(());
  };

  let mut file = OpenOptions::new()
    .create(true)
    .append(true)
    .open(path)
    .with_context(|| format!("failed to open log file: {}", path.display()))?;

  let ts = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();

  writeln!(file, "[{ts}] [{level}] {message}")
    .with_context(|| format!("failed to write log file: {}", path.display()))?;

  Ok(())
}
