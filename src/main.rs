#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use chrono::Local;
use rodio::{Decoder, OutputStreamBuilder, Sink};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use toml_edit::{Array, DocumentMut, Item, Value};

const APP_DIR_NAME: &str = "codex-sound-manager";
const LEGACY_APP_DIR_NAME: &str = "CodexSoundManager";
const SETTINGS_FILE_NAME: &str = "settings.json";
const LOCK_FILE_NAME: &str = "notify.lock";
const LOG_FILE_NAME: &str = "notifier.log";
const CONFIG_BACKUP_NAME: &str = "config.toml.codex-sound-manager.bak";
const MAX_SOUND_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_SOUND_BYTES: &[u8] = include_bytes!("../sounds/default-notification.wav");
const SUPPORTED_SOUND_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "ogg", "m4a", "aac"];
const MAIN_WINDOW_LABEL: &str = "main";
const FLOATING_WINDOW_LABEL: &str = "floating-ball";
const TRAY_ICON_ID: &str = "main-tray";
const MENU_SHOW_MAIN: &str = "show-main";
const MENU_TOGGLE_SOUND: &str = "toggle-sound";
const MENU_TOGGLE_FLOATING: &str = "toggle-floating";
const MENU_QUIT: &str = "quit";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct AppSettings {
    enabled: bool,
    play_count: u8,
    floating_ball_enabled: bool,
    sound_path: Option<String>,
    sound_name: Option<String>,
    previous_notifier: Option<Vec<String>>,
    codex_home: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            play_count: 2,
            floating_ball_enabled: false,
            sound_path: None,
            sound_name: None,
            previous_notifier: None,
            codex_home: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanResult {
    codex_found: bool,
    config_found: bool,
    configured: bool,
    codex_home: String,
    config_path: String,
    executable_path: String,
    sound_name: String,
    using_default_sound: bool,
    settings: AppSettings,
    status_message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationResult {
    message: String,
    scan: ScanResult,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SoundSelection {
    path: String,
    name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimePreferences {
    sound_enabled: bool,
    floating_ball_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CloseChoice {
    Exit,
    MinimizeToTray,
    Cancel,
}

impl TryFrom<&str> for CloseChoice {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "exit" => Ok(Self::Exit),
            "tray" => Ok(Self::MinimizeToTray),
            "cancel" => Ok(Self::Cancel),
            _ => Err("无效的关闭选项".to_string()),
        }
    }
}

struct LockGuard {
    path: PathBuf,
    token: String,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let owns_lock = fs::read_to_string(&self.path)
            .map(|value| value.trim() == self.token)
            .unwrap_or(false);
        if owns_lock {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn default_codex_home() -> Option<PathBuf> {
    if let Some(path) = env::var_os("CODEX_HOME") {
        return Some(PathBuf::from(path));
    }
    if let Some(profile) = env::var_os("USERPROFILE") {
        return Some(PathBuf::from(profile).join(".codex"));
    }
    if let (Some(drive), Some(home_path)) = (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH")) {
        return Some(
            PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                home_path.to_string_lossy()
            ))
            .join(".codex"),
        );
    }
    None
}

fn app_data_dir() -> Result<PathBuf, String> {
    default_codex_home()
        .map(|path| path.join(APP_DIR_NAME))
        .ok_or_else(|| "无法确定当前用户的 Codex 数据目录".to_string())
}

fn legacy_app_data_dirs() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local_app_data).join(LEGACY_APP_DIR_NAME));
    }
    if let Some(profile) = env::var_os("USERPROFILE") {
        let local_app_data = PathBuf::from(&profile).join("AppData").join("Local");
        candidates.push(local_app_data.join(LEGACY_APP_DIR_NAME));

        let packages = local_app_data.join("Packages");
        if let Ok(entries) = fs::read_dir(packages) {
            for entry in entries.flatten() {
                let package_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if package_name.starts_with("openai.codex_") {
                    candidates.push(
                        entry
                            .path()
                            .join("LocalCache")
                            .join("Local")
                            .join(LEGACY_APP_DIR_NAME),
                    );
                }
            }
        }
    }

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|path| seen.insert(path.to_string_lossy().to_lowercase()))
        .collect()
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join(SETTINGS_FILE_NAME))
}

fn is_supported_sound_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            SUPPORTED_SOUND_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

fn normalize_settings(settings: &mut AppSettings) {
    settings.play_count = settings.play_count.clamp(1, 10);
    let invalid_sound = settings.sound_path.as_deref().is_some_and(|path| {
        !Path::new(path).is_file() || !is_supported_sound_file(Path::new(path))
    });
    if invalid_sound {
        settings.sound_path = None;
        settings.sound_name = None;
    }
    if settings.sound_name.as_deref().is_some_and(str::is_empty) {
        settings.sound_name = None;
    }
}

fn parse_settings_file(path: &Path) -> Result<AppSettings, String> {
    let content = fs::read_to_string(path).map_err(|error| format!("读取设置失败：{error}"))?;
    serde_json::from_str(content.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("设置文件格式无效：{error}"))
}

fn read_settings(path: &Path) -> Result<AppSettings, String> {
    let mut settings = parse_settings_file(path)?;
    normalize_settings(&mut settings);
    Ok(settings)
}

fn legacy_sound_source(settings: &AppSettings, legacy_directory: &Path) -> Option<PathBuf> {
    let configured = PathBuf::from(settings.sound_path.as_deref()?);
    if configured.is_file() {
        return Some(configured);
    }
    let file_name = configured.file_name()?;
    let redirected = legacy_directory.join("sounds").join(file_name);
    redirected.is_file().then_some(redirected)
}

fn migrate_legacy_settings(legacy_directory: &Path) -> Result<Option<AppSettings>, String> {
    let legacy_path = legacy_directory.join(SETTINGS_FILE_NAME);
    if !legacy_path.is_file() {
        return Ok(None);
    }

    let mut settings = parse_settings_file(&legacy_path)?;
    if let Some(source) = legacy_sound_source(&settings, legacy_directory) {
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| "旧版自定义提示音没有有效扩展名".to_string())?;
        let destination_directory = app_data_dir()?.join("sounds");
        fs::create_dir_all(&destination_directory)
            .map_err(|error| format!("迁移声音目录失败：{error}"))?;
        let destination = destination_directory.join(format!("custom-sound.{extension}"));
        if source != destination {
            fs::copy(&source, &destination)
                .map_err(|error| format!("迁移自定义提示音失败：{error}"))?;
        }
        settings.sound_path = Some(destination.to_string_lossy().to_string());
    }
    normalize_settings(&mut settings);
    save_settings(&settings)?;
    Ok(Some(settings))
}

fn load_settings() -> Result<AppSettings, String> {
    let path = settings_path()?;
    if path.is_file() {
        return read_settings(&path);
    }
    for legacy_directory in legacy_app_data_dirs() {
        if let Some(settings) = migrate_legacy_settings(&legacy_directory)? {
            return Ok(settings);
        }
    }
    Ok(AppSettings::default())
}

fn save_settings(settings: &AppSettings) -> Result<(), String> {
    validate_settings(settings)?;
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建设置目录失败：{error}"))?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("生成设置文件失败：{error}"))?;
    fs::write(&path, format!("{json}\n")).map_err(|error| format!("保存设置失败：{error}"))
}

fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    if !(1..=10).contains(&settings.play_count) {
        return Err("播放次数必须在 1 到 10 之间".to_string());
    }
    if let Some(path) = settings.sound_path.as_deref()
        && !Path::new(path).is_file()
    {
        return Err("自定义提示音文件不存在，请重新选择".to_string());
    }
    if let Some(path) = settings.sound_path.as_deref()
        && !is_supported_sound_file(Path::new(path))
    {
        return Err("自定义提示音格式不受支持".to_string());
    }
    Ok(())
}

fn default_sound_path() -> Result<PathBuf, String> {
    let sound_dir = app_data_dir()?.join("sounds");
    fs::create_dir_all(&sound_dir).map_err(|error| format!("创建默认声音目录失败：{error}"))?;
    let path = sound_dir.join("default-notification.wav");
    let should_write = fs::read(&path)
        .map(|bytes| bytes.as_slice() != DEFAULT_SOUND_BYTES)
        .unwrap_or(true);
    if should_write {
        fs::write(&path, DEFAULT_SOUND_BYTES)
            .map_err(|error| format!("写入默认提示音失败：{error}"))?;
    }
    Ok(path)
}

fn resolved_sound(settings: &AppSettings) -> Result<(PathBuf, bool), String> {
    if let Some(path) = settings.sound_path.as_deref() {
        let custom = PathBuf::from(path);
        if custom.is_file() {
            return Ok((custom, false));
        }
    }
    Ok((default_sound_path()?, true))
}

fn play_sound(settings: &AppSettings) -> Result<(), String> {
    validate_settings(settings)?;
    if !settings.enabled {
        return Ok(());
    }

    let (sound_path, _) = resolved_sound(settings)?;
    let stream = OutputStreamBuilder::open_default_stream()
        .map_err(|error| format!("无法打开默认音频设备：{error}"))?;
    let sink = Sink::connect_new(stream.mixer());
    sink.set_volume(1.0);

    for _ in 0..settings.play_count {
        let file = File::open(&sound_path)
            .map_err(|error| format!("无法打开提示音 {}：{error}", sound_path.display()))?;
        let source = Decoder::try_from(file)
            .map_err(|error| format!("无法解码提示音 {}：{error}", sound_path.display()))?;
        sink.append(source);
        sink.sleep_until_end();
    }
    Ok(())
}

fn candidate_codex_homes(settings: &AppSettings) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = settings.codex_home.as_deref() {
        candidates.push(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("CODEX_HOME") {
        candidates.push(PathBuf::from(path));
    }
    if let Some(profile) = env::var_os("USERPROFILE") {
        candidates.push(PathBuf::from(profile).join(".codex"));
    }
    if let (Some(drive), Some(home_path)) = (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH")) {
        candidates.push(
            PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                home_path.to_string_lossy()
            ))
            .join(".codex"),
        );
    }

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|path| seen.insert(path.to_string_lossy().to_lowercase()))
        .collect()
}

fn find_codex_home(settings: &AppSettings) -> PathBuf {
    let candidates = candidate_codex_homes(settings);
    if let Some(path) = candidates
        .iter()
        .find(|path| path.join("config.toml").is_file())
    {
        return path.clone();
    }
    if let Some(path) = candidates.iter().find(|path| path.is_dir()) {
        return path.clone();
    }
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn parse_config(path: &Path) -> Result<DocumentMut, String> {
    if !path.is_file() {
        return Ok(DocumentMut::new());
    }
    let content =
        fs::read_to_string(path).map_err(|error| format!("读取 Codex 配置失败：{error}"))?;
    content
        .trim_start_matches('\u{feff}')
        .parse::<DocumentMut>()
        .map_err(|error| format!("Codex 配置不是有效的 TOML：{error}"))
}

fn array_to_strings(array: &Array) -> Option<Vec<String>> {
    array
        .iter()
        .map(|value| value.as_str().map(str::to_string))
        .collect()
}

fn strings_to_array(values: &[String]) -> Array {
    let mut array = Array::new();
    for value in values {
        array.push(value.as_str());
    }
    array
}

fn notify_command(document: &DocumentMut) -> Option<Vec<String>> {
    document
        .get("notify")
        .and_then(Item::as_array)
        .and_then(array_to_strings)
}

fn validate_notify_entry(document: &DocumentMut) -> Result<(), String> {
    if let Some(item) = document.get("notify")
        && item.as_array().and_then(array_to_strings).is_none()
    {
        return Err(
            "Codex 配置中的 notify 必须是仅包含字符串的命令数组，请先修正该配置".to_string(),
        );
    }
    Ok(())
}

fn is_manager_command(command: &[String]) -> bool {
    command.len() >= 2
        && command[1] == "--notify"
        && Path::new(&command[0])
            .file_stem()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("codex-sound-manager"))
}

fn is_legacy_sound_command(command: &[String]) -> bool {
    command.iter().any(|argument| {
        let normalized = argument.replace('/', "\\").to_ascii_lowercase();
        normalized.ends_with(".codex\\bin\\task-complete.ps1")
            || normalized.ends_with(".codex\\bin\\play-task-complete-sound.ps1")
    })
}

fn is_computer_use_wrapper(command: &[String]) -> bool {
    command
        .first()
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("codex-computer-use.exe"))
}

fn nested_previous_notifier(command: &[String]) -> Option<(usize, Vec<String>)> {
    let index = command
        .iter()
        .position(|argument| argument == "--previous-notify")?;
    let json = command.get(index + 1)?;
    serde_json::from_str::<Vec<String>>(json)
        .ok()
        .map(|nested| (index, nested))
}

fn command_contains_manager(command: &[String]) -> bool {
    if is_manager_command(command) {
        return true;
    }
    nested_previous_notifier(command)
        .map(|(_, nested)| is_manager_command(&nested))
        .unwrap_or(false)
}

fn command_uses_exact_manager(command: &[String], manager_command: &[String]) -> bool {
    if command == manager_command {
        return true;
    }
    nested_previous_notifier(command)
        .map(|(_, nested)| nested == manager_command)
        .unwrap_or(false)
}

fn install_notify(
    document: &mut DocumentMut,
    manager_command: &[String],
    settings: &mut AppSettings,
) -> Result<(), String> {
    validate_notify_entry(document)?;
    let existing = notify_command(document);
    match existing {
        Some(mut command) if is_computer_use_wrapper(&command) => {
            if let Some(index) = command
                .iter()
                .position(|argument| argument == "--previous-notify")
            {
                let nested_json = command
                    .get(index + 1)
                    .ok_or_else(|| "Computer Use 的 --previous-notify 缺少命令参数".to_string())?;
                let nested = serde_json::from_str::<Vec<String>>(nested_json)
                    .map_err(|error| format!("Computer Use 的原通知回调格式无效：{error}"))?;
                if !is_manager_command(&nested) && !is_legacy_sound_command(&nested) {
                    settings.previous_notifier = Some(nested);
                } else if is_legacy_sound_command(&nested) {
                    settings.previous_notifier = None;
                }
                command[index + 1] = serde_json::to_string(manager_command)
                    .map_err(|error| format!("生成通知回调参数失败：{error}"))?;
            } else {
                command.push("--previous-notify".to_string());
                command.push(
                    serde_json::to_string(manager_command)
                        .map_err(|error| format!("生成通知回调参数失败：{error}"))?,
                );
            }
            document["notify"] = Item::Value(Value::Array(strings_to_array(&command)));
        }
        Some(command) if is_manager_command(&command) => {}
        Some(command) => {
            if !is_legacy_sound_command(&command) {
                settings.previous_notifier = Some(command);
            } else {
                settings.previous_notifier = None;
            }
            document["notify"] = Item::Value(Value::Array(strings_to_array(manager_command)));
        }
        None => {
            settings.previous_notifier = None;
            document["notify"] = Item::Value(Value::Array(strings_to_array(manager_command)));
        }
    }
    Ok(())
}

fn uninstall_notify(
    document: &mut DocumentMut,
    settings: &mut AppSettings,
) -> Result<bool, String> {
    validate_notify_entry(document)?;
    let Some(mut command) = notify_command(document) else {
        settings.previous_notifier = None;
        return Ok(false);
    };

    if is_computer_use_wrapper(&command) {
        if let Some((index, nested)) = nested_previous_notifier(&command) {
            if !is_manager_command(&nested) {
                return Ok(false);
            }
            if let Some(previous) = settings.previous_notifier.as_ref() {
                command[index + 1] = serde_json::to_string(previous)
                    .map_err(|error| format!("恢复原通知回调失败：{error}"))?;
            } else {
                command.drain(index..=index + 1);
            }
            document["notify"] = Item::Value(Value::Array(strings_to_array(&command)));
            settings.previous_notifier = None;
            return Ok(true);
        }
        return Ok(false);
    }

    if is_manager_command(&command) {
        if let Some(previous) = settings.previous_notifier.take() {
            document["notify"] = Item::Value(Value::Array(strings_to_array(&previous)));
        } else {
            document.remove("notify");
        }
        return Ok(true);
    }
    Ok(false)
}

fn current_manager_command() -> Result<Vec<String>, String> {
    let executable = env::current_exe().map_err(|error| format!("无法确定程序路径：{error}"))?;
    Ok(vec![
        executable.to_string_lossy().to_string(),
        "--notify".to_string(),
    ])
}

fn write_config(path: &Path, document: &DocumentMut) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 Codex 配置目录失败：{error}"))?;
        let backup = parent.join(CONFIG_BACKUP_NAME);
        if path.is_file() && !backup.exists() {
            fs::copy(path, &backup).map_err(|error| format!("备份 Codex 配置失败：{error}"))?;
        }
    }
    fs::write(path, document.to_string())
        .map_err(|error| format!("写入 Codex 全局配置失败：{error}"))
}

fn scan_internal(settings: AppSettings) -> Result<ScanResult, String> {
    let codex_home = find_codex_home(&settings);
    let config_path = codex_home.join("config.toml");
    let codex_found = codex_home.is_dir();
    let config_found = config_path.is_file();
    let configured = if config_found {
        parse_config(&config_path)
            .ok()
            .and_then(|document| notify_command(&document))
            .map(|command| command_contains_manager(&command))
            .unwrap_or(false)
    } else {
        false
    };
    let (sound_path, using_default_sound) = resolved_sound(&settings)?;
    let sound_name = if using_default_sound {
        "内置默认提示音".to_string()
    } else {
        settings.sound_name.clone().unwrap_or_else(|| {
            sound_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("自定义提示音")
                .to_string()
        })
    };
    let status_message = if !codex_found {
        "未发现 Codex 用户目录".to_string()
    } else if configured {
        "全局提示音已配置".to_string()
    } else if config_found {
        "已发现 Codex，等待应用配置".to_string()
    } else {
        "已发现 Codex 用户目录，尚未生成全局配置".to_string()
    };

    Ok(ScanResult {
        codex_found,
        config_found,
        configured,
        codex_home: codex_home.to_string_lossy().to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        executable_path: env::current_exe()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default(),
        sound_name,
        using_default_sound,
        settings,
        status_message,
    })
}

fn runtime_preferences(settings: &AppSettings) -> RuntimePreferences {
    RuntimePreferences {
        sound_enabled: settings.enabled,
        floating_ball_enabled: settings.floating_ball_enabled,
    }
}

fn apply_configuration_message(already_configured: bool) -> &'static str {
    if already_configured {
        "设置已保存，下一次任务完成立即生效，无需重启 Codex"
    } else {
        "已写入 Codex 全局配置，请完整重启 Codex"
    }
}

fn update_tray_tooltip(app: &AppHandle, settings: &AppSettings) {
    if let Some(tray) = app.tray_by_id(TRAY_ICON_ID) {
        let sound_status = if settings.enabled {
            "已开启"
        } else {
            "已关闭"
        };
        let floating_status = if settings.floating_ball_enabled {
            "已显示"
        } else {
            "已隐藏"
        };
        let tooltip =
            format!("Codex 提示音管理器\n提示音：{sound_status}\n悬浮球：{floating_status}");
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

fn emit_sound_enabled(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    app.emit("sound-enabled-changed", settings.enabled)
        .map_err(|error| format!("同步提示音状态失败：{error}"))?;
    update_tray_tooltip(app, settings);
    Ok(())
}

fn emit_floating_ball_enabled(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    app.emit(
        "floating-ball-enabled-changed",
        settings.floating_ball_enabled,
    )
    .map_err(|error| format!("同步悬浮球状态失败：{error}"))?;
    update_tray_tooltip(app, settings);
    Ok(())
}

fn show_main_window_internal(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "找不到主窗口".to_string())?;
    window
        .show()
        .map_err(|error| format!("显示主窗口失败：{error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("聚焦主窗口失败：{error}"))
}

fn set_floating_ball_visibility(app: &AppHandle, visible: bool) -> Result<(), String> {
    let window = app
        .get_webview_window(FLOATING_WINDOW_LABEL)
        .ok_or_else(|| "找不到桌面悬浮球窗口".to_string())?;
    if visible {
        window
            .show()
            .map_err(|error| format!("显示桌面悬浮球失败：{error}"))
    } else {
        window
            .hide()
            .map_err(|error| format!("隐藏桌面悬浮球失败：{error}"))
    }
}

fn position_floating_ball(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(FLOATING_WINDOW_LABEL)
        .ok_or_else(|| "找不到桌面悬浮球窗口".to_string())?;
    let Some(monitor) = window
        .primary_monitor()
        .map_err(|error| format!("读取主显示器失败：{error}"))?
    else {
        return Ok(());
    };
    let window_size = window
        .outer_size()
        .map_err(|error| format!("读取悬浮球大小失败：{error}"))?;
    let work_area = monitor.work_area();
    let margin = (20.0 * monitor.scale_factor()).round() as i32;
    let x = work_area.position.x + work_area.size.width as i32 - window_size.width as i32 - margin;
    let y =
        work_area.position.y + work_area.size.height as i32 - window_size.height as i32 - margin;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| format!("定位桌面悬浮球失败：{error}"))
}

fn persist_sound_enabled(
    app: &AppHandle,
    mut settings: AppSettings,
    enabled: bool,
) -> Result<bool, String> {
    settings.enabled = enabled;
    save_settings(&settings)?;
    emit_sound_enabled(app, &settings)?;
    Ok(settings.enabled)
}

fn set_sound_enabled_internal(app: &AppHandle, enabled: bool) -> Result<bool, String> {
    persist_sound_enabled(app, load_settings()?, enabled)
}

fn toggle_sound_enabled_internal(app: &AppHandle) -> Result<bool, String> {
    let settings = load_settings()?;
    let enabled = !settings.enabled;
    persist_sound_enabled(app, settings, enabled)
}

fn set_floating_ball_enabled_internal(app: &AppHandle, enabled: bool) -> Result<bool, String> {
    let mut settings = load_settings()?;
    let previous = settings.floating_ball_enabled;
    settings.floating_ball_enabled = enabled;
    set_floating_ball_visibility(app, enabled)?;
    if let Err(error) = save_settings(&settings) {
        let _ = set_floating_ball_visibility(app, previous);
        return Err(error);
    }
    emit_floating_ball_enabled(app, &settings)?;
    Ok(settings.floating_ball_enabled)
}

#[tauri::command]
fn scan_codex() -> Result<ScanResult, String> {
    scan_internal(load_settings()?)
}

#[tauri::command]
fn choose_sound() -> Result<Option<SoundSelection>, String> {
    let destination_dir = app_data_dir()?.join("sounds");
    fs::create_dir_all(&destination_dir).map_err(|error| format!("创建音频目录失败：{error}"))?;
    let _ = default_sound_path()?;
    let selected = rfd::FileDialog::new()
        .set_title("选择任务完成提示音")
        .set_directory(&destination_dir)
        .add_filter("音频文件", SUPPORTED_SOUND_EXTENSIONS)
        .pick_file();
    let Some(source) = selected else {
        return Ok(None);
    };
    let metadata = fs::metadata(&source).map_err(|error| format!("读取音频文件失败：{error}"))?;
    if metadata.len() > MAX_SOUND_BYTES {
        return Err("音频文件不能超过 50 MB".to_string());
    }
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "无法识别音频文件格式".to_string())?;
    if !SUPPORTED_SOUND_EXTENSIONS.contains(&extension.as_str()) {
        return Err("请选择 WAV、MP3、FLAC、OGG、M4A 或 AAC 音频文件".to_string());
    }
    let destination = destination_dir.join(format!("custom-sound.{extension}"));
    if source != destination {
        fs::copy(&source, &destination).map_err(|error| format!("导入音频文件失败：{error}"))?;
    }
    Ok(Some(SoundSelection {
        path: destination.to_string_lossy().to_string(),
        name: source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("自定义提示音")
            .to_string(),
    }))
}

#[tauri::command]
async fn preview_sound(settings: AppSettings) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || play_sound(&settings))
        .await
        .map_err(|error| format!("试听任务执行失败：{error}"))?
}

#[tauri::command]
fn get_runtime_preferences() -> Result<RuntimePreferences, String> {
    load_settings().map(|settings| runtime_preferences(&settings))
}

#[tauri::command]
fn toggle_sound_enabled(app: AppHandle) -> Result<bool, String> {
    toggle_sound_enabled_internal(&app)
}

#[tauri::command]
fn set_sound_enabled(app: AppHandle, enabled: bool) -> Result<bool, String> {
    set_sound_enabled_internal(&app, enabled)
}

#[tauri::command]
fn set_floating_ball_enabled(app: AppHandle, enabled: bool) -> Result<bool, String> {
    set_floating_ball_enabled_internal(&app, enabled)
}

#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    show_main_window_internal(&app)
}

#[tauri::command]
fn resolve_close_choice(app: AppHandle, choice: &str) -> Result<(), String> {
    match CloseChoice::try_from(choice)? {
        CloseChoice::Exit => app.exit(0),
        CloseChoice::MinimizeToTray => {
            let window = app
                .get_webview_window(MAIN_WINDOW_LABEL)
                .ok_or_else(|| "找不到主窗口".to_string())?;
            window
                .hide()
                .map_err(|error| format!("最小化到系统托盘失败：{error}"))?;
        }
        CloseChoice::Cancel => {}
    }
    Ok(())
}

#[tauri::command]
fn apply_configuration(
    app: AppHandle,
    mut settings: AppSettings,
) -> Result<OperationResult, String> {
    validate_settings(&settings)?;
    let codex_home = find_codex_home(&settings);
    if !codex_home.is_dir() {
        return Err("未发现 Codex 用户目录，请先启动一次 Codex 后重新扫描".to_string());
    }
    let config_path = codex_home.join("config.toml");
    let mut document = parse_config(&config_path)?;
    let manager_command = current_manager_command()?;
    let callback_unchanged = notify_command(&document)
        .map(|command| command_uses_exact_manager(&command, &manager_command))
        .unwrap_or(false);
    install_notify(&mut document, &manager_command, &mut settings)?;
    settings.codex_home = Some(codex_home.to_string_lossy().to_string());
    save_settings(&settings)?;
    write_config(&config_path, &document)?;
    set_floating_ball_visibility(&app, settings.floating_ball_enabled)?;
    emit_sound_enabled(&app, &settings)?;
    emit_floating_ball_enabled(&app, &settings)?;
    Ok(OperationResult {
        message: apply_configuration_message(callback_unchanged).to_string(),
        scan: scan_internal(settings)?,
    })
}

#[tauri::command]
fn remove_configuration() -> Result<OperationResult, String> {
    let mut settings = load_settings()?;
    let codex_home = find_codex_home(&settings);
    let config_path = codex_home.join("config.toml");
    let mut document = parse_config(&config_path)?;
    let changed = uninstall_notify(&mut document, &mut settings)?;
    if changed {
        write_config(&config_path, &document)?;
    }
    save_settings(&settings)?;
    Ok(OperationResult {
        message: if changed {
            "已移除提示音回调，请完整重启 Codex".to_string()
        } else {
            "当前 Codex 配置中没有本工具的回调".to_string()
        },
        scan: scan_internal(settings)?,
    })
}

fn acquire_sound_lock() -> Result<Option<LockGuard>, String> {
    let directory = app_data_dir()?;
    fs::create_dir_all(&directory).map_err(|error| format!("创建运行目录失败：{error}"))?;
    let path = directory.join(LOCK_FILE_NAME);
    if path.is_file() {
        let stale = path
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > Duration::from_secs(120));
        if stale {
            let _ = fs::remove_file(&path);
        }
    }
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            if let Err(error) = writeln!(file, "{token}") {
                let _ = fs::remove_file(&path);
                return Err(format!("写入提示音运行锁失败：{error}"));
            }
            Ok(Some(LockGuard { path, token }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(error) => Err(format!("创建提示音运行锁失败：{error}")),
    }
}

fn append_log(message: &str) {
    let Ok(directory) = app_data_dir() else {
        return;
    };
    let _ = fs::create_dir_all(&directory);
    let path = directory.join(LOG_FILE_NAME);
    if path
        .metadata()
        .map(|metadata| metadata.len() > 1024 * 1024)
        .unwrap_or(false)
    {
        let _ = fs::remove_file(&path);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "{} {message}",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );
    }
}

fn run_previous_notifier(command: &[String], payload_args: &[String]) -> Result<(), String> {
    if command.is_empty() || is_manager_command(command) || is_legacy_sound_command(command) {
        return Ok(());
    }
    let mut process = Command::new(&command[0]);
    process.args(&command[1..]).args(payload_args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        process.creation_flags(0x08000000);
    }
    let status = process
        .status()
        .map_err(|error| format!("运行原通知回调失败：{error}"))?;
    if !status.success() {
        return Err(format!("原通知回调退出码：{}", status.code().unwrap_or(-1)));
    }
    Ok(())
}

fn notification_success_message(play_count: u8) -> String {
    format!("中文动态日志：任务完成，提示音已播放 {play_count} 次")
}

fn run_notification(payload_args: &[String]) {
    let settings = match load_settings() {
        Ok(settings) => settings,
        Err(error) => {
            append_log(&format!("读取设置失败：{error}"));
            AppSettings::default()
        }
    };

    if let Some(previous) = settings.previous_notifier.as_deref()
        && let Err(error) = run_previous_notifier(previous, payload_args)
    {
        append_log(&error);
    }
    if !settings.enabled {
        append_log("中文动态日志：任务完成，提示音当前已关闭");
        return;
    }
    let Ok(Some(_lock)) = acquire_sound_lock() else {
        return;
    };
    match play_sound(&settings) {
        Ok(()) => append_log(&notification_success_message(settings.play_count)),
        Err(error) => append_log(&format!("播放失败：{error}")),
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_main = MenuItem::with_id(app, MENU_SHOW_MAIN, "打开主窗口", true, None::<&str>)?;
    let toggle_sound = MenuItem::with_id(
        app,
        MENU_TOGGLE_SOUND,
        "开启 / 关闭提示音",
        true,
        None::<&str>,
    )?;
    let toggle_floating = MenuItem::with_id(
        app,
        MENU_TOGGLE_FLOATING,
        "显示 / 隐藏悬浮球",
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出程序", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_main,
            &toggle_sound,
            &toggle_floating,
            &separator,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ICON_ID)
        .menu(&menu)
        .tooltip("Codex 提示音管理器")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let result = if event.id() == MENU_SHOW_MAIN {
                show_main_window_internal(app)
            } else if event.id() == MENU_TOGGLE_SOUND {
                toggle_sound_enabled_internal(app).map(|_| ())
            } else if event.id() == MENU_TOGGLE_FLOATING {
                load_settings().and_then(|settings| {
                    set_floating_ball_enabled_internal(app, !settings.floating_ball_enabled)
                        .map(|_| ())
                })
            } else if event.id() == MENU_QUIT {
                app.exit(0);
                Ok(())
            } else {
                Ok(())
            };
            if let Err(error) = result {
                append_log(&format!("托盘操作失败：{error}"));
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
                && let Err(error) = show_main_window_internal(tray.app_handle())
            {
                append_log(&format!("托盘打开主窗口失败：{error}"));
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn main() {
    let arguments: Vec<String> = env::args().collect();
    if let Some(index) = arguments.iter().position(|argument| argument == "--notify") {
        run_notification(&arguments[index + 1..]);
        return;
    }

    tauri::Builder::default()
        .setup(|app| {
            setup_tray(app)?;
            let settings = load_settings().unwrap_or_else(|error| {
                append_log(&format!("启动时读取设置失败：{error}"));
                AppSettings::default()
            });
            if let Err(error) = position_floating_ball(app.handle()) {
                append_log(&error);
            }
            if let Err(error) =
                set_floating_ball_visibility(app.handle(), settings.floating_ball_enabled)
            {
                append_log(&error);
            }
            update_tray_tooltip(app.handle(), &settings);
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == MAIN_WINDOW_LABEL
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                if let Err(error) = window.app_handle().emit("close-choice-requested", ()) {
                    append_log(&format!("打开关闭选择窗口失败：{error}"));
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            scan_codex,
            choose_sound,
            preview_sound,
            get_runtime_preferences,
            toggle_sound_enabled,
            set_sound_enabled,
            set_floating_ball_enabled,
            show_main_window,
            resolve_close_choice,
            apply_configuration,
            remove_configuration
        ])
        .run(tauri::generate_context!())
        .expect("启动 Codex 提示音管理器失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager_command() -> Vec<String> {
        vec![
            r"C:\Program Files\Codex Sound Manager\codex-sound-manager.exe".to_string(),
            "--notify".to_string(),
        ]
    }

    #[test]
    fn installs_and_restores_plain_notifier_without_losing_comments() {
        let mut document = r#"# 中文注释
model = "gpt-test"
notify = ["notify-send", "Codex"]
"#
        .parse::<DocumentMut>()
        .unwrap();
        let mut settings = AppSettings::default();
        install_notify(&mut document, &manager_command(), &mut settings).unwrap();
        assert!(command_contains_manager(
            &notify_command(&document).unwrap()
        ));
        assert!(document.to_string().contains("# 中文注释"));
        assert_eq!(
            settings.previous_notifier.as_ref().unwrap()[0],
            "notify-send"
        );

        assert!(uninstall_notify(&mut document, &mut settings).unwrap());
        assert_eq!(notify_command(&document).unwrap()[0], "notify-send");
    }

    #[test]
    fn updates_computer_use_previous_notifier_in_place() {
        let previous = serde_json::to_string(&vec!["notify-send", "Codex"]).unwrap();
        let source = format!(
            "notify = ['C:\\\\Codex\\\\codex-computer-use.exe', 'turn-ended', '--previous-notify', '{}']\n",
            previous.replace('\\', "\\\\").replace('\'', "\\'")
        );
        let mut document = source.parse::<DocumentMut>().unwrap();
        let mut settings = AppSettings::default();
        install_notify(&mut document, &manager_command(), &mut settings).unwrap();
        let command = notify_command(&document).unwrap();
        assert!(is_computer_use_wrapper(&command));
        assert!(command_contains_manager(&command));
        assert_eq!(
            settings.previous_notifier.as_ref().unwrap()[0],
            "notify-send"
        );

        assert!(uninstall_notify(&mut document, &mut settings).unwrap());
        let restored = notify_command(&document).unwrap();
        assert_eq!(
            nested_previous_notifier(&restored).unwrap().1[0],
            "notify-send"
        );
    }

    #[test]
    fn detects_when_the_configured_manager_command_is_unchanged() {
        let manager = manager_command();
        assert!(command_uses_exact_manager(&manager, &manager));

        let wrapped = vec![
            r"C:\Codex\codex-computer-use.exe".to_string(),
            "turn-ended".to_string(),
            "--previous-notify".to_string(),
            serde_json::to_string(&manager).unwrap(),
        ];
        assert!(command_uses_exact_manager(&wrapped, &manager));

        let mut moved_manager = manager.clone();
        moved_manager[0] = r"D:\Different\codex-sound-manager.exe".to_string();
        assert!(!command_uses_exact_manager(&moved_manager, &manager));
    }

    #[test]
    fn ignores_legacy_personal_sound_script() {
        let mut document = r#"notify = ["powershell.exe", "-File", "C:\\Users\\demo\\.codex\\bin\\task-complete.ps1"]
"#
        .parse::<DocumentMut>()
        .unwrap();
        let mut settings = AppSettings::default();
        install_notify(&mut document, &manager_command(), &mut settings).unwrap();
        assert!(settings.previous_notifier.is_none());
    }

    #[test]
    fn rejects_invalid_notify_without_overwriting_it() {
        let mut document = "notify = 'not-an-array'\n".parse::<DocumentMut>().unwrap();
        let mut settings = AppSettings::default();
        let error = install_notify(&mut document, &manager_command(), &mut settings).unwrap_err();
        assert!(error.contains("notify"));
        assert_eq!(document["notify"].as_str(), Some("not-an-array"));
    }

    #[test]
    fn rejects_malformed_computer_use_previous_notifier() {
        let mut document =
            r#"notify = ["C:\\Codex\\codex-computer-use.exe", "turn-ended", "--previous-notify"]
"#
            .parse::<DocumentMut>()
            .unwrap();
        let mut settings = AppSettings::default();
        let error = install_notify(&mut document, &manager_command(), &mut settings).unwrap_err();
        assert!(error.contains("缺少命令参数"));
        assert_eq!(
            notify_command(&document)
                .unwrap()
                .iter()
                .filter(|argument| argument.as_str() == "--previous-notify")
                .count(),
            1
        );
    }

    #[test]
    fn older_settings_files_receive_new_defaults() {
        let settings = serde_json::from_str::<AppSettings>(r#"{"enabled":false}"#).unwrap();
        assert!(!settings.enabled);
        assert_eq!(settings.play_count, 2);
        assert!(!settings.floating_ball_enabled);
        assert!(settings.sound_name.is_none());
    }

    #[test]
    fn validates_close_choices() {
        assert_eq!(CloseChoice::try_from("exit"), Ok(CloseChoice::Exit));
        assert_eq!(
            CloseChoice::try_from("tray"),
            Ok(CloseChoice::MinimizeToTray)
        );
        assert_eq!(CloseChoice::try_from("cancel"), Ok(CloseChoice::Cancel));
        assert!(CloseChoice::try_from("unknown").is_err());
    }

    #[test]
    fn exposes_runtime_preferences_without_configuration_details() {
        let settings = AppSettings {
            enabled: false,
            floating_ball_enabled: true,
            ..AppSettings::default()
        };
        let preferences = runtime_preferences(&settings);
        assert!(!preferences.sound_enabled);
        assert!(preferences.floating_ball_enabled);
    }

    #[test]
    fn explains_when_codex_restart_is_required() {
        assert_eq!(
            apply_configuration_message(true),
            "设置已保存，下一次任务完成立即生效，无需重启 Codex"
        );
        assert_eq!(
            apply_configuration_message(false),
            "已写入 Codex 全局配置，请完整重启 Codex"
        );
    }

    #[test]
    fn preserves_chinese_dynamic_log_text() {
        assert_eq!(
            notification_success_message(2),
            "中文动态日志：任务完成，提示音已播放 2 次"
        );
    }

    #[test]
    fn validates_supported_audio_extensions_case_insensitively() {
        assert!(is_supported_sound_file(Path::new("notice.MP3")));
        assert!(!is_supported_sound_file(Path::new("notice.exe")));
    }

    #[test]
    fn finds_audio_in_a_redirected_legacy_directory() {
        let directory = tempfile::tempdir().unwrap();
        let sounds = directory.path().join("sounds");
        fs::create_dir_all(&sounds).unwrap();
        let redirected = sounds.join("custom-sound.mp3");
        fs::write(&redirected, b"test-audio").unwrap();
        let settings = AppSettings {
            sound_path: Some(
                r"C:\Users\demo\AppData\Local\CodexSoundManager\sounds\custom-sound.mp3"
                    .to_string(),
            ),
            ..AppSettings::default()
        };

        assert_eq!(
            legacy_sound_source(&settings, directory.path()),
            Some(redirected)
        );
    }

    #[test]
    fn lock_guard_does_not_remove_a_replaced_lock() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notify.lock");

        fs::write(&path, "owned-token\n").unwrap();
        drop(LockGuard {
            path: path.clone(),
            token: "owned-token".to_string(),
        });
        assert!(!path.exists());

        fs::write(&path, "replacement-token\n").unwrap();
        drop(LockGuard {
            path: path.clone(),
            token: "old-token".to_string(),
        });
        assert!(path.exists());
    }
}
