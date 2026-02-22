use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use lazaro_core::config::{BlockLevel, Settings};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("io error: {0}")]
    Io(String),
    #[error("profile not found: {0}")]
    ProfileNotFound(String),
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StartupMode {
    XdgOnly,
    XdgAndSystemd,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProfileDto {
    id: String,
    name: String,
    settings: SettingsDto,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WeeklyStatsDto {
    total_active_seconds: u64,
    micro_done: u32,
    rest_done: u32,
    daily_limit_hits: u32,
    skipped: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SettingsDto {
    micro_interval_seconds: u64,
    micro_duration_seconds: u64,
    micro_snooze_seconds: u64,
    rest_interval_seconds: u64,
    rest_duration_seconds: u64,
    rest_snooze_seconds: u64,
    daily_limit_seconds: u64,
    daily_limit_snooze_seconds: u64,
    daily_reset_time: String,
    block_level: String,
    desktop_notifications: bool,
    overlay_notifications: bool,
    sound_notifications: bool,
    sound_theme: String,
    startup_xdg: bool,
    startup_systemd_user: bool,
    active_profile_id: String,
}

impl Default for SettingsDto {
    fn default() -> Self {
        Self::from(Settings::default())
    }
}

impl From<Settings> for SettingsDto {
    fn from(value: Settings) -> Self {
        let block_level = match value.block_level {
            BlockLevel::Soft => "soft",
            BlockLevel::Medium => "medium",
            BlockLevel::Strict => "strict",
        }
        .to_string();

        Self {
            micro_interval_seconds: value.micro.interval_seconds,
            micro_duration_seconds: value.micro.duration_seconds,
            micro_snooze_seconds: value.micro.snooze_seconds,
            rest_interval_seconds: value.rest.interval_seconds,
            rest_duration_seconds: value.rest.duration_seconds,
            rest_snooze_seconds: value.rest.snooze_seconds,
            daily_limit_seconds: value.daily_limit.limit_seconds,
            daily_limit_snooze_seconds: value.daily_limit.snooze_seconds,
            daily_reset_time: format!(
                "{:02}:{:02}",
                value.daily_limit.reset_hour_local, value.daily_limit.reset_minute_local
            ),
            block_level,
            desktop_notifications: value.notifications.desktop_enabled,
            overlay_notifications: value.notifications.overlay_enabled,
            sound_notifications: value.notifications.sound_enabled,
            sound_theme: value.notifications.sound_theme,
            startup_xdg: value.startup.xdg_autostart_enabled,
            startup_systemd_user: value.startup.systemd_user_enabled,
            active_profile_id: value.active_profile_id,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AppStateOnDisk {
    settings: SettingsDto,
    profiles: BTreeMap<String, ProfileDto>,
    weekly_stats: WeeklyStatsDto,
}

impl Default for AppStateOnDisk {
    fn default() -> Self {
        let default_profile = ProfileDto {
            id: "default".into(),
            name: "Default".into(),
            settings: SettingsDto::default(),
        };

        let mut profiles = BTreeMap::new();
        profiles.insert(default_profile.id.clone(), default_profile);

        Self {
            settings: SettingsDto::default(),
            profiles,
            weekly_stats: WeeklyStatsDto {
                total_active_seconds: 0,
                micro_done: 0,
                rest_done: 0,
                daily_limit_hits: 0,
                skipped: 0,
            },
        }
    }
}

struct AppState {
    path: PathBuf,
    data: Mutex<AppStateOnDisk>,
}

impl AppState {
    fn init() -> Result<Self, AppError> {
        let base = default_data_dir();
        fs::create_dir_all(&base)?;
        let path = base.join("state.json");

        let data = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            AppStateOnDisk::default()
        };

        let state = Self {
            path,
            data: Mutex::new(data),
        };
        state.save()?;
        Ok(state)
    }

    fn save(&self) -> Result<(), AppError> {
        let payload = {
            let guard = self
                .data
                .lock()
                .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
            serde_json::to_string_pretty(&*guard).map_err(|e| AppError::Io(e.to_string()))?
        };
        fs::write(&self.path, payload)?;
        Ok(())
    }
}

fn default_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("lazaro");
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".local/share/lazaro")
}

fn ensure_xdg_autostart() -> Result<(), AppError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir = Path::new(&home).join(".config/autostart");
    fs::create_dir_all(&dir)?;
    let file = dir.join("io.lazaro.Lazaro.desktop");

    let content = r#"[Desktop Entry]
Type=Application
Name=Lazaro
Comment=Personalized break reminder
Exec=lazaro
Terminal=false
X-GNOME-Autostart-enabled=true
"#;

    fs::write(file, content)?;
    Ok(())
}

fn ensure_systemd_user_service() -> Result<(), AppError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir = Path::new(&home).join(".config/systemd/user");
    fs::create_dir_all(&dir)?;
    let file = dir.join("lazaro.service");

    let content = r#"[Unit]
Description=Lazaro break reminder
After=graphical-session.target

[Service]
Type=simple
ExecStart=lazaro
Restart=on-failure

[Install]
WantedBy=default.target
"#;

    fs::write(file, content)?;
    Ok(())
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Result<SettingsDto, AppError> {
    let guard = state
        .data
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    Ok(guard.settings.clone())
}

#[tauri::command]
fn update_settings(
    settings: SettingsDto,
    state: tauri::State<'_, AppState>,
) -> Result<SettingsDto, AppError> {
    {
        let mut guard = state
            .data
            .lock()
            .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
        guard.settings = settings.clone();
    }
    state.save()?;
    Ok(settings)
}

#[tauri::command]
fn list_profiles(state: tauri::State<'_, AppState>) -> Result<Vec<ProfileDto>, AppError> {
    let guard = state
        .data
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    Ok(guard.profiles.values().cloned().collect())
}

#[tauri::command]
fn save_profile(
    profile: ProfileDto,
    state: tauri::State<'_, AppState>,
) -> Result<ProfileDto, AppError> {
    {
        let mut guard = state
            .data
            .lock()
            .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
        guard.profiles.insert(profile.id.clone(), profile.clone());
    }
    state.save()?;
    Ok(profile)
}

#[tauri::command]
fn activate_profile(profile_id: String, state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    {
        let mut guard = state
            .data
            .lock()
            .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
        let Some(profile) = guard.profiles.get(&profile_id).cloned() else {
            return Err(AppError::ProfileNotFound(profile_id));
        };

        guard.settings = profile.settings;
        guard.settings.active_profile_id = profile_id;
    }
    state.save()?;
    Ok(())
}

#[tauri::command]
fn get_weekly_stats(state: tauri::State<'_, AppState>) -> Result<WeeklyStatsDto, AppError> {
    let guard = state
        .data
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    Ok(guard.weekly_stats.clone())
}

#[tauri::command]
fn set_startup_mode(mode: StartupMode, state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    ensure_xdg_autostart()?;

    if matches!(mode, StartupMode::XdgAndSystemd) {
        ensure_systemd_user_service()?;
    }

    {
        let mut guard = state
            .data
            .lock()
            .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
        match mode {
            StartupMode::XdgOnly => {
                guard.settings.startup_xdg = true;
                guard.settings.startup_systemd_user = false;
            }
            StartupMode::XdgAndSystemd => {
                guard.settings.startup_xdg = true;
                guard.settings.startup_systemd_user = true;
            }
        }
    }
    state.save()?;
    Ok(())
}

#[tauri::command]
fn trigger_break(kind: String) -> Result<String, AppError> {
    let output = format!("break_triggered:{kind}");
    Ok(output)
}

fn main() {
    let state = AppState::init().expect("failed to initialize state");

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            list_profiles,
            save_profile,
            activate_profile,
            get_weekly_stats,
            set_startup_mode,
            trigger_break
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
