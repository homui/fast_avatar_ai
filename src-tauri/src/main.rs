#![cfg_attr(windows, windows_subsystem = "windows")]

mod app_log;
mod config;
mod server;
mod speech;

use std::{
  fs,
  io::Write,
  path::{Path, PathBuf},
  process::Command,
};
use serde::Serialize;
use tauri::{
  LogicalPosition, LogicalSize,
  menu::MenuBuilder,
  tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
  window::Color,
  Emitter, Manager,
};
use url::Url;

const MAIN_WINDOW_LABEL: &str = "main";
const SETTINGS_WINDOW_LABEL: &str = "settings";
const TRAY_TOGGLE_ID: &str = "tray.toggle";
const TRAY_CHAT_ID: &str = "tray.chat";
const TRAY_SETTINGS_ID: &str = "tray.settings";
const TRAY_QUIT_ID: &str = "tray.quit";
const COMPACT_WINDOW_WIDTH: f64 = 320.0;
const COMPACT_WINDOW_HEIGHT: f64 = 620.0;
const EXPANDED_WINDOW_WIDTH: f64 = 660.0;
const EXPANDED_WINDOW_HEIGHT: f64 = 820.0;

fn write_bootstrap_log(message: &str) {
  let path = std::env::temp_dir().join("fast-avatar-ai-bootstrap.log");
  if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
    let _ = writeln!(file, "{message}");
  }
}

struct RepoState {
  root: PathBuf,
}

struct RuntimePaths {
  app_root: PathBuf,
  web_dir: PathBuf,
  bundled_config_dir: PathBuf,
  config_dir: PathBuf,
  bundled_live2d_dir: PathBuf,
  live2d_dir: PathBuf,
  bundled_scripts_dir: PathBuf,
  scripts_dir: PathBuf,
  bundled_models_dir: PathBuf,
  models_dir: PathBuf,
  webview_data_dir: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VtsCompanionInfo {
  model_dir_url: String,
  vtube_config_url: Option<String>,
  pinned_items_url: Option<String>,
  item_assets: Vec<VtsPinnedItemAsset>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VtsPinnedItemAsset {
  item_name: String,
  item_file_name: String,
  asset_type: Option<String>,
  asset_url: Option<String>,
}

fn compact_window_size(avatar_scale: Option<f64>) -> (f64, f64) {
  let scale = avatar_scale.unwrap_or(1.0).clamp(0.5, 2.4);
  let width = (240.0 + scale * 120.0).clamp(340.0, 500.0);
  let height = (450.0 + scale * 270.0).clamp(620.0, 1020.0);
  (width, height)
}

fn set_main_window_layout_inner(
  window: &tauri::WebviewWindow,
  expanded: bool,
  avatar_scale: Option<f64>,
) -> Result<(), String> {
  let (target_width, target_height) = if expanded {
    (EXPANDED_WINDOW_WIDTH, EXPANDED_WINDOW_HEIGHT)
  } else {
    compact_window_size(avatar_scale)
  };

  let current_position = window.inner_position().map_err(|error| error.to_string())?;
  let current_size = window.inner_size().map_err(|error| error.to_string())?;

  let anchor_center_x = current_position.x as f64 + current_size.width as f64 / 2.0;
  let anchor_bottom_y = current_position.y as f64 + current_size.height as f64;
  let next_x = anchor_center_x - target_width / 2.0;
  let next_y = anchor_bottom_y - target_height;

  window
    .set_min_size(Some(LogicalSize::new(target_width, target_height)))
    .map_err(|error| error.to_string())?;
  window
    .set_size(LogicalSize::new(target_width, target_height))
    .map_err(|error| error.to_string())?;
  window
    .set_position(LogicalPosition::new(next_x, next_y))
    .map_err(|error| error.to_string())?;

  Ok(())
}

#[tauri::command]
fn start_dragging(window: tauri::WebviewWindow) -> Result<(), String> {
  window.start_dragging().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_main_window_layout(
  window: tauri::WebviewWindow,
  expanded: bool,
  avatar_scale: Option<f64>,
) -> Result<(), String> {
  set_main_window_layout_inner(&window, expanded, avatar_scale)
}

#[tauri::command]
fn load_app_config(repo_state: tauri::State<RepoState>) -> Result<config::ConfigEnvelope, String> {
  config::load_or_init(&repo_state.root).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_app_config(
  app: tauri::AppHandle,
  repo_state: tauri::State<RepoState>,
  settings: config::AppSettings,
) -> Result<config::ConfigEnvelope, String> {
  let envelope = config::write_settings(&repo_state.root, &settings).map_err(|error| error.to_string())?;
  let _ = app.emit("app-config-updated", &envelope);
  Ok(envelope)
}

#[tauri::command]
fn request_main_window_avatar_reload(app: tauri::AppHandle) -> Result<(), String> {
  if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
    window
      .eval("window.fastAvatarReloadFromConfig?.();")
      .map_err(|error| error.to_string())?;
  }
  Ok(())
}

#[tauri::command]
fn reset_app_config(
  app: tauri::AppHandle,
  repo_state: tauri::State<RepoState>,
) -> Result<config::ConfigEnvelope, String> {
  let envelope = config::reset(&repo_state.root).map_err(|error| error.to_string())?;
  let _ = app.emit("app-config-updated", &envelope);
  Ok(envelope)
}

#[tauri::command]
fn reveal_config_file(repo_state: tauri::State<RepoState>) -> Result<String, String> {
  let path = config::config_path(&repo_state.root);
  let parent = path
    .parent()
    .ok_or_else(|| "config directory unavailable".to_string())?;

  Command::new("explorer")
    .arg(parent)
    .spawn()
    .map_err(|error| error.to_string())?;

  Ok(path.display().to_string())
}

#[tauri::command]
fn frontend_log(level: String, message: String) -> Result<(), String> {
  app_log::write_line(&level, &message).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_log_file_path() -> Result<String, String> {
  app_log::path()
    .map(|path| path.display().to_string())
    .ok_or_else(|| "log path unavailable".to_string())
}

fn pick_live2d_model_file() -> Result<Option<PathBuf>, String> {
  let script = r#"
    Add-Type -AssemblyName System.Windows.Forms
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    $dialog = New-Object System.Windows.Forms.OpenFileDialog
    $dialog.Filter = 'Live2D Model (*.model3.json)|*.model3.json'
    $dialog.Title = '选择 Live2D 模型文件'
    $dialog.Multiselect = $false
    if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
      Write-Output $dialog.FileName
    }
  "#;
  let output = Command::new("powershell")
    .args(["-NoProfile", "-STA", "-Command", script])
    .output()
    .map_err(|error| error.to_string())?;
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    return Err(if stderr.is_empty() {
      "failed to open model picker".to_string()
    } else {
      stderr
    });
  }
  let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
  if selected.is_empty() {
    return Ok(None);
  }
  Ok(Some(PathBuf::from(selected)))
}

fn pick_live2d_model_file_native() -> Option<PathBuf> {
  rfd::FileDialog::new()
    .add_filter("Live2D Model", &["model3.json"])
    .set_title("选择 Live2D 模型文件")
    .pick_file()
}

fn model_url_to_repo_path(app_root: &Path, model_url: &str) -> Option<PathBuf> {
  let value = model_url.trim();
  if value.is_empty() {
    return None;
  }
  let path = if value.starts_with("\\\\") || value.starts_with("//") || value.contains(":\\") || value.contains(":/") {
    PathBuf::from(value)
  } else {
    app_root.join(value.trim_start_matches('/').replace('/', "\\"))
  };
  Some(path)
}

fn repo_path_to_url(app_root: &Path, path: &Path) -> Option<String> {
  let relative = path.strip_prefix(app_root).ok()?;
  Some(format!("/{}", relative.to_string_lossy().replace('\\', "/")))
}

fn collect_files_recursive(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
  for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
    let entry = entry.map_err(|error| error.to_string())?;
    let path = entry.path();
    if entry.file_type().map_err(|error| error.to_string())?.is_dir() {
      collect_files_recursive(&path, output)?;
    } else {
      output.push(path);
    }
  }
  Ok(())
}

fn normalize_vts_item_name(value: &str) -> String {
  value
    .chars()
    .filter(|ch| ch.is_alphanumeric())
    .flat_map(|ch| ch.to_lowercase())
    .collect::<String>()
}

fn parse_vts_item_names(items_path: &Path) -> Vec<(String, String)> {
  let text = match fs::read_to_string(items_path) {
    Ok(value) => value,
    Err(_) => return Vec::new(),
  };
  let json = match serde_json::from_str::<serde_json::Value>(&text) {
    Ok(value) => value,
    Err(_) => return Vec::new(),
  };
  json
    .get("Items")
    .and_then(|value| value.as_array())
    .into_iter()
    .flatten()
    .map(|item| {
      let item_name = item
        .get("ItemName")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
      let item_file_name = item
        .get("ItemFileName")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
      (item_name, item_file_name)
    })
    .filter(|(_, item_file_name)| !item_file_name.is_empty())
    .collect()
}

fn resolve_vts_item_asset(files: &[PathBuf], item_file_name: &str) -> Option<(String, PathBuf)> {
  let normalized_needle = normalize_vts_item_name(item_file_name);
  if normalized_needle.is_empty() {
    return None;
  }

  let mut best_score = i32::MIN;
  let mut best_match: Option<(String, PathBuf)> = None;

  for path in files {
    let file_name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
    let path_text = path.to_string_lossy();
    let normalized_file_name = normalize_vts_item_name(file_name);
    let normalized_path = normalize_vts_item_name(&path_text);
    let extension = path
      .extension()
      .and_then(|value| value.to_str())
      .unwrap_or_default()
      .to_ascii_lowercase();
    let asset_type = match extension.as_str() {
      "json" if file_name.ends_with(".model3.json") => Some("live2d_model"),
      "png" | "jpg" | "jpeg" | "webp" => Some("image"),
      _ => None,
    };
    let Some(asset_type) = asset_type else {
      continue;
    };

    let mut score = 0;
    if normalized_file_name == normalized_needle {
      score += 100;
    } else if normalized_file_name.contains(&normalized_needle) {
      score += 60;
    } else if normalized_path.contains(&normalized_needle) {
      score += 35;
    } else {
      continue;
    }

    if asset_type == "live2d_model" {
      score += 20;
    }

    if score > best_score {
      best_score = score;
      best_match = Some((asset_type.to_string(), path.clone()));
    }
  }

  best_match
}

#[tauri::command]
fn resolve_vts_companions(
  repo_state: tauri::State<RepoState>,
  model_url: String,
) -> Result<Option<VtsCompanionInfo>, String> {
  let Some(model_path) = model_url_to_repo_path(&repo_state.root, &model_url) else {
    return Ok(None);
  };
  let model_dir = model_path
    .parent()
    .ok_or_else(|| "model directory unavailable".to_string())?;
  if !model_dir.exists() {
    return Ok(None);
  }

  let model_stem = model_path
    .file_name()
    .and_then(|value| value.to_str())
    .map(|value| value.trim_end_matches(".model3.json"))
    .unwrap_or_default();

  let vtube_config_path = model_dir.join(format!("{model_stem}.vtube.json"));
  let pinned_items_path = model_dir.join("items_pinned_to_model.json");

  let vtube_config_url = if vtube_config_path.exists() {
    repo_path_to_url(&repo_state.root, &vtube_config_path)
  } else {
    None
  };
  let pinned_items_url = if pinned_items_path.exists() {
    repo_path_to_url(&repo_state.root, &pinned_items_path)
  } else {
    None
  };

  if vtube_config_url.is_none() && pinned_items_url.is_none() {
    return Ok(None);
  }

  let mut files = Vec::new();
  collect_files_recursive(model_dir, &mut files)?;

  let item_assets = parse_vts_item_names(&pinned_items_path)
    .into_iter()
    .map(|(item_name, item_file_name)| {
      let resolved = resolve_vts_item_asset(&files, &item_file_name);
      let (asset_type, asset_url) = match resolved {
        Some((asset_type, asset_path)) => (
          Some(asset_type),
          repo_path_to_url(&repo_state.root, &asset_path),
        ),
        None => (None, None),
      };
      VtsPinnedItemAsset {
        item_name,
        item_file_name,
        asset_type,
        asset_url,
      }
    })
    .collect::<Vec<_>>();

  Ok(Some(VtsCompanionInfo {
    model_dir_url: repo_path_to_url(&repo_state.root, model_dir).unwrap_or_default(),
    vtube_config_url,
    pinned_items_url,
    item_assets,
  }))
}

#[tauri::command]
fn import_live2d_character(
  app: tauri::AppHandle,
  repo_state: tauri::State<RepoState>,
) -> Result<Option<config::ImportCharacterResult>, String> {
  let Some(model_file) = pick_live2d_model_file_native() else {
    return Ok(None);
  };
  let result =
    config::import_character_model(&repo_state.root, &model_file).map_err(|error| error.to_string())?;
  let _ = app.emit("app-config-updated", &result.envelope);
  Ok(Some(result))
}

fn show_main_window(app: &tauri::AppHandle) {
  if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
    let _ = window.set_always_on_top(true);
    let _ = window.show();
    let _ = window.set_focus();
  }
}

fn show_settings_window(app: &tauri::AppHandle) {
  if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
    let _ = window.show();
    let _ = window.set_focus();
  }
}

fn hide_main_window(app: &tauri::AppHandle) {
  if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
    let _ = window.hide();
  }
}

fn toggle_main_window(app: &tauri::AppHandle) {
  if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
    match window.is_visible() {
      Ok(true) => hide_main_window(app),
      Ok(false) | Err(_) => show_main_window(app),
    }
  }
}

fn open_window_chat(app: &tauri::AppHandle) {
  show_main_window(app);
  if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
    let _ = window.eval("window.fastAvatarOpenChat?.();");
  }
}

fn open_window_settings(app: &tauri::AppHandle) {
  show_settings_window(app);
}

#[tauri::command]
fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
  show_settings_window(&app);
  Ok(())
}

#[tauri::command]
fn hide_current_window(window: tauri::WebviewWindow) -> Result<(), String> {
  window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn open_current_window_devtools(window: tauri::WebviewWindow) -> Result<(), String> {
  window.open_devtools();
  Ok(())
}

fn mic_permission_retry_flag_path(app_root: &Path) -> PathBuf {
  app_root.join(".runtime").join("retry-mic-permission.flag")
}

#[tauri::command]
fn reset_current_webview_permissions(window: tauri::WebviewWindow) -> Result<(), String> {
  window
    .clear_all_browsing_data()
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn reset_current_webview_permissions_and_restart(
  app: tauri::AppHandle,
  window: tauri::WebviewWindow,
  repo_state: tauri::State<RepoState>,
) -> Result<(), String> {
  let retry_flag = mic_permission_retry_flag_path(&repo_state.root);
  if let Some(parent) = retry_flag.parent() {
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
  }
  fs::write(&retry_flag, b"1").map_err(|error| error.to_string())?;
  window
    .clear_all_browsing_data()
    .map_err(|error| error.to_string())?;
  app.request_restart();
  Ok(())
}

#[tauri::command]
fn consume_pending_mic_permission_retry(repo_state: tauri::State<RepoState>) -> Result<bool, String> {
  let retry_flag = mic_permission_retry_flag_path(&repo_state.root);
  if !retry_flag.exists() {
    return Ok(false);
  }
  fs::remove_file(&retry_flag).map_err(|error| error.to_string())?;
  Ok(true)
}

fn handle_tray_menu_event(app: &tauri::AppHandle, id: &str) {
  match id {
    TRAY_TOGGLE_ID => toggle_main_window(app),
    TRAY_CHAT_ID => open_window_chat(app),
    TRAY_SETTINGS_ID => open_window_settings(app),
    TRAY_QUIT_ID => app.exit(0),
    _ => {}
  }
}

fn resolve_runtime_paths(app: &tauri::AppHandle, dev_root: &Path) -> Result<RuntimePaths, anyhow::Error> {
  let raw_resource_dir = app.path().resource_dir().unwrap_or_else(|_| dev_root.to_path_buf());
  let resource_dir = if raw_resource_dir.join("web").exists()
    || raw_resource_dir.join("config").exists()
    || raw_resource_dir.join("live2d").exists()
    || raw_resource_dir.join("models").exists()
    || raw_resource_dir.join("scripts").exists()
  {
    raw_resource_dir.clone()
  } else if raw_resource_dir.join("_up_").exists() {
    raw_resource_dir.join("_up_")
  } else {
    raw_resource_dir.clone()
  };
  let dev_target_dir = dev_root.join("src-tauri").join("target");
  let launched_from_dev_target = std::env::current_exe()
    .ok()
    .map(|path| path.starts_with(&dev_target_dir))
    .unwrap_or(false);
  let use_repo_root = resource_dir == dev_root || launched_from_dev_target;
  let app_root = if use_repo_root {
    dev_root.to_path_buf()
  } else {
    app
      .path()
      .app_data_dir()
      .unwrap_or_else(|_| dev_root.join(".runtime-data"))
  };
  let web_dir = if resource_dir.join("web").exists() {
    resource_dir.join("web")
  } else {
    dev_root.join("web")
  };
  let bundled_config_dir = if resource_dir.join("config").exists() {
    resource_dir.join("config")
  } else {
    dev_root.join("config")
  };
  let bundled_live2d_dir = if resource_dir.join("live2d").exists() {
    resource_dir.join("live2d")
  } else if dev_root.join("live2d").exists() {
    dev_root.join("live2d")
  } else if web_dir.join("live2d").exists() {
    web_dir.join("live2d")
  } else {
    dev_root.join("web").join("live2d")
  };
  let bundled_models_dir = if resource_dir.join("models").exists() {
    resource_dir.join("models")
  } else if resource_dir.join("resources").join("models").exists() {
    resource_dir.join("resources").join("models")
  } else {
    dev_root.join("resources").join("models")
  };
  let bundled_scripts_dir = if resource_dir.join("scripts").exists() {
    resource_dir.join("scripts")
  } else {
    dev_root.join("scripts")
  };
  let config_dir = app_root.join("config");
  let live2d_dir = app_root.join("live2d");
  let scripts_dir = app_root.join("scripts");
  let models_dir = app_root.join("models");
  let webview_data_dir = app_root.join(".tauri").join("webview2");

  Ok(RuntimePaths {
    app_root,
    web_dir,
    bundled_config_dir,
    config_dir,
    bundled_live2d_dir,
    live2d_dir,
    bundled_scripts_dir,
    scripts_dir,
    bundled_models_dir,
    models_dir,
    webview_data_dir,
  })
}

fn ensure_runtime_assets(paths: &RuntimePaths) -> Result<(), anyhow::Error> {
  fs::create_dir_all(&paths.app_root)?;
  fs::create_dir_all(&paths.config_dir)?;
  fs::create_dir_all(&paths.live2d_dir)?;
  fs::create_dir_all(&paths.scripts_dir)?;
  fs::create_dir_all(&paths.models_dir)?;
  sync_missing_dir(&paths.bundled_config_dir, &paths.config_dir)?;
  sync_missing_dir(&paths.bundled_live2d_dir, &paths.live2d_dir)?;
  sync_missing_dir(&paths.bundled_scripts_dir, &paths.scripts_dir)?;
  sync_missing_dir(&paths.bundled_models_dir, &paths.models_dir)?;
  Ok(())
}

fn sync_missing_dir(source: &Path, target: &Path) -> Result<(), anyhow::Error> {
  if !source.exists() {
    return Ok(());
  }
  fs::create_dir_all(target)?;
  for entry in fs::read_dir(source)? {
    let entry = entry?;
    let source_path = entry.path();
    let target_path = target.join(entry.file_name());
    if entry.file_type()?.is_dir() {
      sync_missing_dir(&source_path, &target_path)?;
    } else if !target_path.exists() {
      if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
      }
      fs::copy(&source_path, &target_path)?;
    }
  }
  Ok(())
}

fn main() {
  std::panic::set_hook(Box::new(|panic_info| {
    write_bootstrap_log(&format!("panic: {panic_info}"));
  }));
  write_bootstrap_log("main: process start");
  tracing_subscriber::fmt()
    .with_target(false)
    .compact()
    .init();
  write_bootstrap_log("main: tracing initialized");

  let dev_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .expect("src-tauri should be nested under the repository root")
    .to_path_buf();
  write_bootstrap_log(&format!("main: dev_root={}", dev_root.display()));

  let app = tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      start_dragging,
      set_main_window_layout,
      open_settings_window,
      hide_current_window,
      open_current_window_devtools,
      reset_current_webview_permissions,
      reset_current_webview_permissions_and_restart,
      consume_pending_mic_permission_retry,
      load_app_config,
      save_app_config,
      request_main_window_avatar_reload,
      reset_app_config,
      import_live2d_character,
      resolve_vts_companions,
      reveal_config_file,
      frontend_log,
      get_log_file_path
    ])
    .setup(move |app| {
      write_bootstrap_log("setup: entered");
      let runtime_paths = resolve_runtime_paths(app.handle(), &dev_root)?;
      write_bootstrap_log(&format!(
        "setup: runtime_paths app_root={} web_dir={} bundled_config_dir={} bundled_live2d_dir={} bundled_models_dir={} bundled_scripts_dir={}",
        runtime_paths.app_root.display(),
        runtime_paths.web_dir.display(),
        runtime_paths.bundled_config_dir.display(),
        runtime_paths.bundled_live2d_dir.display(),
        runtime_paths.bundled_models_dir.display(),
        runtime_paths.bundled_scripts_dir.display(),
      ));
      let log_path = app_log::init(&runtime_paths.app_root).map_err(|error| {
        write_bootstrap_log(&format!("setup: app_log init failed: {error}"));
        error
      })?;
      let _ = app_log::write_line("INFO", "tauri setup begin");
      let _ = app_log::write_line(
        "INFO",
        &format!("application bootstrap start: {}", runtime_paths.app_root.display()),
      );
      let _ = app_log::write_line("INFO", &format!("log file: {}", log_path.display()));
      write_bootstrap_log(&format!("setup: app_log initialized at {}", log_path.display()));
      ensure_runtime_assets(&runtime_paths)?;
      write_bootstrap_log("setup: runtime assets ensured");
      app.manage(RepoState {
        root: runtime_paths.app_root.clone(),
      });
      let _ = app_log::write_line("INFO", "repo state registered");
      let runtime_settings = match config::load_or_init(&runtime_paths.app_root) {
        Ok(envelope) => {
          let _ = app_log::write_line("INFO", "config loaded");
          envelope.settings
        }
        Err(error) => {
          let _ = app_log::write_line("ERROR", &format!("config init failed: {error}"));
          return Err(error.into());
        }
      };
      server::spawn_server(
        runtime_paths.app_root.clone(),
        runtime_paths.web_dir.clone(),
        runtime_paths.live2d_dir.clone(),
        Some(runtime_settings),
      )?;
      write_bootstrap_log("setup: http server spawned");
      let _ = app_log::write_line("INFO", "http server spawned on 127.0.0.1:3217");

      let window_url = Url::parse("http://127.0.0.1:3217/index.html")
        .map_err(|error| anyhow::anyhow!("failed to parse window url: {error}"))?;
      let _ = app_log::write_line("INFO", &format!("main window url: {window_url}"));
      fs::create_dir_all(&runtime_paths.webview_data_dir).map_err(|error| {
        anyhow::anyhow!(
          "failed to create WebView2 data directory {}: {error}",
          runtime_paths.webview_data_dir.display()
        )
      })?;
      write_bootstrap_log("setup: webview data dir ensured");
      let _ = app_log::write_line(
        "INFO",
        &format!("webview data directory: {}", runtime_paths.webview_data_dir.display()),
      );

      let main_window = tauri::WebviewWindowBuilder::new(
        app,
        MAIN_WINDOW_LABEL,
        tauri::WebviewUrl::External(window_url),
      )
        .data_directory(runtime_paths.webview_data_dir.clone())
        .devtools(true)
        .title("Fast Avatar AI")
        .decorations(false)
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .inner_size(COMPACT_WINDOW_WIDTH, COMPACT_WINDOW_HEIGHT)
        .min_inner_size(COMPACT_WINDOW_WIDTH, COMPACT_WINDOW_HEIGHT)
        .build()
        .map_err(|error| {
          write_bootstrap_log(&format!("setup: main window creation failed: {error}"));
          let _ = app_log::write_line("ERROR", &format!("main window creation failed: {error}"));
          anyhow::anyhow!("failed to create main window: {error}")
        })?;
      let _ = main_window.set_background_color(Some(Color(0, 0, 0, 0)));
      write_bootstrap_log("setup: main window created");
      let _ = app_log::write_line("INFO", "main window created");

      let main_window_for_events = main_window.clone();
      main_window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
          api.prevent_close();
          let _ = main_window_for_events.hide();
          let _ = app_log::write_line("INFO", "main window close intercepted -> hidden to tray");
        }
      });

      let settings_window_url = Url::parse("http://127.0.0.1:3217/index.html?view=settings")
        .map_err(|error| anyhow::anyhow!("failed to parse settings window url: {error}"))?;
      let settings_window = tauri::WebviewWindowBuilder::new(
        app,
        SETTINGS_WINDOW_LABEL,
        tauri::WebviewUrl::External(settings_window_url),
      )
        .data_directory(runtime_paths.webview_data_dir.join("settings"))
        .devtools(true)
        .title("Fast Avatar AI - 设置")
        .decorations(false)
        .transparent(false)
        .shadow(true)
        .resizable(false)
        .maximizable(false)
        .always_on_top(false)
        .visible(false)
        .inner_size(720.0, 820.0)
        .min_inner_size(560.0, 680.0)
        .build()
        .map_err(|error| {
          write_bootstrap_log(&format!("setup: settings window creation failed: {error}"));
          anyhow::anyhow!("failed to create settings window: {error}")
        })?;
      write_bootstrap_log("setup: settings window created");
      let _ = app_log::write_line("INFO", "settings window created");

      let settings_window_for_events = settings_window.clone();
      settings_window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
          api.prevent_close();
          let _ = settings_window_for_events.hide();
          let _ = app_log::write_line("INFO", "settings window close intercepted -> hidden");
        }
      });

      let tray_menu = MenuBuilder::new(app)
        .text(TRAY_TOGGLE_ID, "显示/隐藏桌宠")
        .text(TRAY_CHAT_ID, "打开聊天")
        .text(TRAY_SETTINGS_ID, "打开设置")
        .separator()
        .text(TRAY_QUIT_ID, "退出")
        .build()?;

      let tray_icon = app
        .default_window_icon()
        .cloned()
        .expect("default tray icon should be available");

      TrayIconBuilder::with_id("fast-avatar-tray")
        .icon(tray_icon)
        .tooltip("Fast Avatar AI")
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
          handle_tray_menu_event(app, event.id().as_ref());
        })
        .on_tray_icon_event(|tray, event| {
          if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
          } = event
          {
            toggle_main_window(tray.app_handle());
          }
        })
        .build(app)?;
      write_bootstrap_log("setup: tray icon created");
      let _ = app_log::write_line("INFO", "tray icon created");

      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while building Fast Avatar AI");
  write_bootstrap_log("main: builder finished");

  let _ = app_log::write_line("INFO", "application event loop starting");
  write_bootstrap_log("main: event loop starting");

  let exit_code = app.run_return(|_, _| {});

  let _ = app_log::write_line("INFO", &format!("application exit code: {exit_code}"));
  write_bootstrap_log(&format!("main: exit_code={exit_code}"));

  std::process::exit(exit_code);
}
