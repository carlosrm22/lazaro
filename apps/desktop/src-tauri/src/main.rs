use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use lazaro_core::{
    config::{
        BlockLevel, BreakTimerSettings, DailyLimitSettings, NotificationSettings, Settings,
        StartupSettings,
    },
    timer::{BreakKind, EngineEvent, TimerEngine},
};
use notify_rust::Notification;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("io error: {0}")]
    Io(String),
    #[error("profile not found: {0}")]
    ProfileNotFound(String),
    #[error("invalid break kind: {0}")]
    InvalidBreakKind(String),
    #[error("invalid reset time format: {0}")]
    InvalidResetTime(String),
    #[error("runtime is not running")]
    RuntimeNotRunning,
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

    fn add_active_seconds(&self, seconds: u64) {
        if let Ok(mut guard) = self.data.lock() {
            guard.weekly_stats.total_active_seconds = guard
                .weekly_stats
                .total_active_seconds
                .saturating_add(seconds);
        }
    }

    fn record_completed_break(&self, kind: BreakKind) {
        if let Ok(mut guard) = self.data.lock() {
            match kind {
                BreakKind::Micro => {
                    guard.weekly_stats.micro_done = guard.weekly_stats.micro_done.saturating_add(1)
                }
                BreakKind::Rest => {
                    guard.weekly_stats.rest_done = guard.weekly_stats.rest_done.saturating_add(1)
                }
                BreakKind::DailyLimit => {
                    guard.weekly_stats.daily_limit_hits =
                        guard.weekly_stats.daily_limit_hits.saturating_add(1)
                }
            }
        }
    }

    fn record_skipped_break(&self) {
        if let Ok(mut guard) = self.data.lock() {
            guard.weekly_stats.skipped = guard.weekly_stats.skipped.saturating_add(1);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RuntimeStatusDto {
    running: bool,
    pending_break: Option<String>,
    active_break: Option<String>,
    remaining_seconds: Option<u64>,
    strict_mode: bool,
    last_event: String,
}

impl Default for RuntimeStatusDto {
    fn default() -> Self {
        Self {
            running: false,
            pending_break: None,
            active_break: None,
            remaining_seconds: None,
            strict_mode: false,
            last_event: "idle".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeEventDto {
    kind: String,
    message: String,
    break_kind: Option<String>,
    remaining_seconds: Option<u64>,
    strict_mode: bool,
}

enum RuntimeControl {
    Stop,
    UpdateSettings { core: Settings, dto: SettingsDto },
    StartBreak(BreakKind),
    StartPending,
    SnoozePending,
}

struct RuntimeController {
    tx: Option<Sender<RuntimeControl>>,
    handle: Option<JoinHandle<()>>,
    status: Arc<Mutex<RuntimeStatusDto>>,
}

impl Default for RuntimeController {
    fn default() -> Self {
        Self {
            tx: None,
            handle: None,
            status: Arc::new(Mutex::new(RuntimeStatusDto::default())),
        }
    }
}

struct BackendState {
    persistent: Arc<AppState>,
    runtime: Mutex<RuntimeController>,
}

fn default_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("lazaro");
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".local/share/lazaro")
}

fn parse_reset_time(value: &str) -> Result<(u8, u8), AppError> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 2 {
        return Err(AppError::InvalidResetTime(value.to_string()));
    }

    let hour: u8 = parts[0]
        .parse()
        .map_err(|_| AppError::InvalidResetTime(value.to_string()))?;
    let minute: u8 = parts[1]
        .parse()
        .map_err(|_| AppError::InvalidResetTime(value.to_string()))?;

    if hour > 23 || minute > 59 {
        return Err(AppError::InvalidResetTime(value.to_string()));
    }

    Ok((hour, minute))
}

fn settings_to_core(dto: &SettingsDto) -> Result<Settings, AppError> {
    let block_level = match dto.block_level.as_str() {
        "soft" => BlockLevel::Soft,
        "medium" => BlockLevel::Medium,
        "strict" => BlockLevel::Strict,
        _ => BlockLevel::Medium,
    };

    let (reset_hour, reset_minute) = parse_reset_time(&dto.daily_reset_time)?;

    Ok(Settings {
        micro: BreakTimerSettings {
            interval_seconds: dto.micro_interval_seconds,
            duration_seconds: dto.micro_duration_seconds,
            snooze_seconds: dto.micro_snooze_seconds,
            enabled: true,
        },
        rest: BreakTimerSettings {
            interval_seconds: dto.rest_interval_seconds,
            duration_seconds: dto.rest_duration_seconds,
            snooze_seconds: dto.rest_snooze_seconds,
            enabled: true,
        },
        daily_limit: DailyLimitSettings {
            limit_seconds: dto.daily_limit_seconds,
            snooze_seconds: dto.daily_limit_snooze_seconds,
            reset_hour_local: reset_hour,
            reset_minute_local: reset_minute,
            enabled: true,
        },
        block_level,
        notifications: NotificationSettings {
            desktop_enabled: dto.desktop_notifications,
            overlay_enabled: dto.overlay_notifications,
            sound_enabled: dto.sound_notifications,
            sound_theme: dto.sound_theme.clone(),
        },
        startup: StartupSettings {
            xdg_autostart_enabled: dto.startup_xdg,
            systemd_user_enabled: dto.startup_systemd_user,
        },
        active_profile_id: dto.active_profile_id.clone(),
    })
}

fn break_kind_to_string(kind: BreakKind) -> String {
    match kind {
        BreakKind::Micro => "micro".into(),
        BreakKind::Rest => "rest".into(),
        BreakKind::DailyLimit => "daily_limit".into(),
    }
}

fn parse_break_kind(value: &str) -> Result<BreakKind, AppError> {
    match value {
        "micro" => Ok(BreakKind::Micro),
        "rest" => Ok(BreakKind::Rest),
        "daily_limit" => Ok(BreakKind::DailyLimit),
        _ => Err(AppError::InvalidBreakKind(value.to_string())),
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn emit_runtime_event(app: &AppHandle, event: RuntimeEventDto) {
    let _ = app.emit("runtime://event", event);
}

fn send_notification(settings: &SettingsDto, title: &str, body: &str) {
    if !settings.desktop_notifications {
        return;
    }

    let _ = Notification::new().summary(title).body(body).show();
}

fn open_overlay(
    app: &AppHandle,
    kind: BreakKind,
    remaining: u64,
    overlay_enabled: bool,
    strict_mode: bool,
) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if !overlay_enabled {
            if let Some(window) = app_handle.get_webview_window("break-overlay") {
                let _ = window.close();
            }
            return;
        }

        if let Some(window) = app_handle.get_webview_window("break-overlay") {
            let _ = window.close();
        }

        let base_builder = WebviewWindowBuilder::new(
            &app_handle,
            "break-overlay",
            WebviewUrl::App("overlay.html".into()),
        )
        .title("Lazaro - Descanso")
        .decorations(false)
        .always_on_top(true)
        .fullscreen(true)
        .resizable(false)
        .skip_taskbar(true);

        let builder = if strict_mode {
            base_builder.closable(false)
        } else {
            base_builder.closable(true)
        };

        let _ = builder.build();
    });

    emit_runtime_event(
        app,
        RuntimeEventDto {
            kind: "break_started".into(),
            message: "Descanso iniciado".into(),
            break_kind: Some(break_kind_to_string(kind)),
            remaining_seconds: Some(remaining),
            strict_mode,
        },
    );
}

fn close_overlay(app: &AppHandle) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_handle.get_webview_window("break-overlay") {
            let _ = window.close();
        }
    });
}

fn resolve_autostart_exec() -> String {
    let flatpak_available = Command::new("flatpak")
        .arg("info")
        .arg("io.lazaro.Lazaro")
        .output()
        .is_ok_and(|result| result.status.success());

    if flatpak_available {
        "flatpak run io.lazaro.Lazaro".into()
    } else {
        "lazaro".into()
    }
}

fn ensure_xdg_autostart() -> Result<(), AppError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir = Path::new(&home).join(".config/autostart");
    fs::create_dir_all(&dir)?;
    let file = dir.join("io.lazaro.Lazaro.desktop");
    let exec = resolve_autostart_exec();

    let content = format!(
        "[Desktop Entry]\nType=Application\nName=Lazaro\nComment=Personalized break reminder\nExec={exec}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    );

    fs::write(file, content)?;
    Ok(())
}

fn ensure_systemd_user_service() -> Result<(), AppError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir = Path::new(&home).join(".config/systemd/user");
    fs::create_dir_all(&dir)?;
    let file = dir.join("lazaro.service");
    let exec = resolve_autostart_exec();

    let content = format!(
        "[Unit]\nDescription=Lazaro break reminder\nAfter=graphical-session.target\n\n[Service]\nType=simple\nExecStart={exec}\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n"
    );

    fs::write(file, content)?;
    Ok(())
}

fn runtime_loop(
    app: AppHandle,
    persistent: Arc<AppState>,
    status: Arc<Mutex<RuntimeStatusDto>>,
    rx: mpsc::Receiver<RuntimeControl>,
    mut core_settings: Settings,
    mut settings_dto: SettingsDto,
) {
    let mut engine = TimerEngine::new(core_settings.clone(), unix_now());
    let mut pending_break: Option<BreakKind> = None;
    let mut running = true;
    let mut tick_counter: u64 = 0;

    if let Ok(mut guard) = status.lock() {
        guard.running = true;
        guard.strict_mode = matches!(core_settings.block_level, BlockLevel::Strict);
        guard.last_event = "runtime_started".into();
    }

    while running {
        while let Ok(message) = rx.try_recv() {
            match message {
                RuntimeControl::Stop => {
                    running = false;
                }
                RuntimeControl::UpdateSettings { core, dto } => {
                    core_settings = core;
                    *engine.settings_mut() = core_settings.clone();
                    settings_dto = dto;
                    if let Ok(mut guard) = status.lock() {
                        guard.strict_mode = matches!(core_settings.block_level, BlockLevel::Strict);
                        guard.last_event = "settings_updated".into();
                    }
                }
                RuntimeControl::StartBreak(kind) => {
                    pending_break = None;
                    let events = engine.start_break(kind);
                    for event in events {
                        if let EngineEvent::BreakStarted(kind) = event {
                            let remaining = engine.active_break_info().map(|(_, r)| r).unwrap_or(0);
                            open_overlay(
                                &app,
                                kind,
                                remaining,
                                settings_dto.overlay_notifications,
                                matches!(core_settings.block_level, BlockLevel::Strict),
                            );
                            send_notification(
                                &settings_dto,
                                "Lazaro",
                                &format!("Comienza el descanso {}", break_kind_to_string(kind)),
                            );
                        }
                    }
                }
                RuntimeControl::StartPending => {
                    if let Some(kind) = pending_break.take() {
                        let events = engine.start_break(kind);
                        for event in events {
                            if let EngineEvent::BreakStarted(kind) = event {
                                let remaining =
                                    engine.active_break_info().map(|(_, r)| r).unwrap_or(0);
                                open_overlay(
                                    &app,
                                    kind,
                                    remaining,
                                    settings_dto.overlay_notifications,
                                    matches!(core_settings.block_level, BlockLevel::Strict),
                                );
                            }
                        }
                    }
                }
                RuntimeControl::SnoozePending => {
                    if !matches!(core_settings.block_level, BlockLevel::Strict)
                        && let Some(kind) = pending_break.take()
                    {
                        let _ = engine.snooze(kind, unix_now());
                        persistent.record_skipped_break();
                        emit_runtime_event(
                            &app,
                            RuntimeEventDto {
                                kind: "break_snoozed".into(),
                                message: format!(
                                    "Se pospone descanso {}",
                                    break_kind_to_string(kind)
                                ),
                                break_kind: Some(break_kind_to_string(kind)),
                                remaining_seconds: None,
                                strict_mode: false,
                            },
                        );
                    }
                }
            }
        }

        if !running {
            break;
        }

        let now = unix_now();
        let events = if engine.active_break_info().is_some() {
            engine.tick_break(1)
        } else {
            persistent.add_active_seconds(1);
            engine.on_activity(1, now)
        };

        for event in events {
            match event {
                EngineEvent::BreakDue(kind) => {
                    pending_break = Some(kind);
                    let strict_mode = matches!(core_settings.block_level, BlockLevel::Strict);
                    emit_runtime_event(
                        &app,
                        RuntimeEventDto {
                            kind: "break_due".into(),
                            message: format!("Descanso {} disponible", break_kind_to_string(kind)),
                            break_kind: Some(break_kind_to_string(kind)),
                            remaining_seconds: None,
                            strict_mode,
                        },
                    );
                    send_notification(
                        &settings_dto,
                        "Lazaro",
                        &format!("Toca descanso {}", break_kind_to_string(kind)),
                    );
                }
                EngineEvent::BreakStarted(kind) => {
                    pending_break = None;
                    let remaining = engine.active_break_info().map(|(_, r)| r).unwrap_or(0);
                    open_overlay(
                        &app,
                        kind,
                        remaining,
                        settings_dto.overlay_notifications,
                        matches!(core_settings.block_level, BlockLevel::Strict),
                    );
                    emit_runtime_event(
                        &app,
                        RuntimeEventDto {
                            kind: "break_started".into(),
                            message: format!("Descanso {} iniciado", break_kind_to_string(kind)),
                            break_kind: Some(break_kind_to_string(kind)),
                            remaining_seconds: Some(remaining),
                            strict_mode: matches!(core_settings.block_level, BlockLevel::Strict),
                        },
                    );
                }
                EngineEvent::BreakCompleted(kind) => {
                    persistent.record_completed_break(kind);
                    close_overlay(&app);
                    emit_runtime_event(
                        &app,
                        RuntimeEventDto {
                            kind: "break_completed".into(),
                            message: format!("Descanso {} completado", break_kind_to_string(kind)),
                            break_kind: Some(break_kind_to_string(kind)),
                            remaining_seconds: Some(0),
                            strict_mode: matches!(core_settings.block_level, BlockLevel::Strict),
                        },
                    );
                    send_notification(
                        &settings_dto,
                        "Lazaro",
                        "Buen trabajo. Descanso completado.",
                    );
                    let _ = persistent.save();
                }
                EngineEvent::BreakSnoozed(kind, until) => {
                    emit_runtime_event(
                        &app,
                        RuntimeEventDto {
                            kind: "break_snoozed".into(),
                            message: format!(
                                "Descanso {} pospuesto hasta {}",
                                break_kind_to_string(kind),
                                until
                            ),
                            break_kind: Some(break_kind_to_string(kind)),
                            remaining_seconds: None,
                            strict_mode: false,
                        },
                    );
                }
                EngineEvent::DailyReset => {
                    emit_runtime_event(
                        &app,
                        RuntimeEventDto {
                            kind: "daily_reset".into(),
                            message: "Reinicio diario aplicado".into(),
                            break_kind: None,
                            remaining_seconds: None,
                            strict_mode: false,
                        },
                    );
                }
            }
        }

        if let Some((kind, remaining)) = engine.active_break_info() {
            emit_runtime_event(
                &app,
                RuntimeEventDto {
                    kind: "break_tick".into(),
                    message: "Cuenta regresiva activa".into(),
                    break_kind: Some(break_kind_to_string(kind)),
                    remaining_seconds: Some(remaining),
                    strict_mode: matches!(core_settings.block_level, BlockLevel::Strict),
                },
            );
        }

        if let Ok(mut guard) = status.lock() {
            guard.running = true;
            guard.pending_break = pending_break.map(break_kind_to_string);
            guard.active_break = engine
                .active_break_info()
                .map(|(kind, _)| break_kind_to_string(kind));
            guard.remaining_seconds = engine.active_break_info().map(|(_, remaining)| remaining);
            guard.strict_mode = matches!(core_settings.block_level, BlockLevel::Strict);
            guard.last_event = "tick".into();
        }

        tick_counter = tick_counter.saturating_add(1);
        if tick_counter.is_multiple_of(20) {
            let _ = persistent.save();
        }

        thread::sleep(Duration::from_secs(1));
    }

    close_overlay(&app);
    let _ = persistent.save();

    if let Ok(mut guard) = status.lock() {
        guard.running = false;
        guard.pending_break = None;
        guard.active_break = None;
        guard.remaining_seconds = None;
        guard.last_event = "runtime_stopped".into();
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, BackendState>) -> Result<SettingsDto, AppError> {
    let guard = state
        .persistent
        .data
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    Ok(guard.settings.clone())
}

#[tauri::command]
fn update_settings(
    settings: SettingsDto,
    state: tauri::State<'_, BackendState>,
) -> Result<SettingsDto, AppError> {
    {
        let mut guard = state
            .persistent
            .data
            .lock()
            .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
        guard.settings = settings.clone();
    }
    state.persistent.save()?;

    let core = settings_to_core(&settings)?;
    if let Ok(runtime) = state.runtime.lock()
        && let Some(tx) = runtime.tx.clone()
    {
        let _ = tx.send(RuntimeControl::UpdateSettings {
            core,
            dto: settings.clone(),
        });
    }

    Ok(settings)
}

#[tauri::command]
fn list_profiles(state: tauri::State<'_, BackendState>) -> Result<Vec<ProfileDto>, AppError> {
    let guard = state
        .persistent
        .data
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    Ok(guard.profiles.values().cloned().collect())
}

#[tauri::command]
fn save_profile(
    profile: ProfileDto,
    state: tauri::State<'_, BackendState>,
) -> Result<ProfileDto, AppError> {
    {
        let mut guard = state
            .persistent
            .data
            .lock()
            .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
        guard.profiles.insert(profile.id.clone(), profile.clone());
    }
    state.persistent.save()?;
    Ok(profile)
}

#[tauri::command]
fn activate_profile(
    profile_id: String,
    state: tauri::State<'_, BackendState>,
) -> Result<(), AppError> {
    let updated_settings = {
        let mut guard = state
            .persistent
            .data
            .lock()
            .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
        let Some(profile) = guard.profiles.get(&profile_id).cloned() else {
            return Err(AppError::ProfileNotFound(profile_id));
        };

        guard.settings = profile.settings;
        guard.settings.active_profile_id = profile_id;
        guard.settings.clone()
    };
    state.persistent.save()?;

    let core = settings_to_core(&updated_settings)?;
    if let Ok(runtime) = state.runtime.lock()
        && let Some(tx) = runtime.tx.clone()
    {
        let _ = tx.send(RuntimeControl::UpdateSettings {
            core,
            dto: updated_settings,
        });
    }

    Ok(())
}

#[tauri::command]
fn get_weekly_stats(state: tauri::State<'_, BackendState>) -> Result<WeeklyStatsDto, AppError> {
    let guard = state
        .persistent
        .data
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    Ok(guard.weekly_stats.clone())
}

#[tauri::command]
fn set_startup_mode(
    mode: StartupMode,
    state: tauri::State<'_, BackendState>,
) -> Result<(), AppError> {
    ensure_xdg_autostart()?;

    if matches!(mode, StartupMode::XdgAndSystemd) {
        ensure_systemd_user_service()?;
    }

    {
        let mut guard = state
            .persistent
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
    state.persistent.save()?;
    Ok(())
}

#[tauri::command]
fn start_runtime(
    app: AppHandle,
    state: tauri::State<'_, BackendState>,
) -> Result<RuntimeStatusDto, AppError> {
    let settings = {
        let guard = state
            .persistent
            .data
            .lock()
            .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
        guard.settings.clone()
    };
    let core = settings_to_core(&settings)?;

    let mut runtime = state
        .runtime
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;

    if runtime.tx.is_none() {
        let (tx, rx) = mpsc::channel::<RuntimeControl>();
        let status = Arc::clone(&runtime.status);
        let persistent = Arc::clone(&state.persistent);
        let app_handle = app.clone();

        let join = thread::spawn(move || {
            runtime_loop(app_handle, persistent, status, rx, core, settings);
        });

        runtime.tx = Some(tx);
        runtime.handle = Some(join);
    }

    let status = runtime
        .status
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?
        .clone();
    Ok(status)
}

#[tauri::command]
fn stop_runtime(state: tauri::State<'_, BackendState>) -> Result<RuntimeStatusDto, AppError> {
    let handle = {
        let mut runtime = state
            .runtime
            .lock()
            .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;

        let Some(tx) = runtime.tx.take() else {
            return Err(AppError::RuntimeNotRunning);
        };

        let _ = tx.send(RuntimeControl::Stop);
        runtime.handle.take()
    };

    if let Some(join) = handle {
        let _ = join.join();
    }

    let runtime = state
        .runtime
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    let status = runtime
        .status
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?
        .clone();

    Ok(status)
}

#[tauri::command]
fn get_runtime_status(state: tauri::State<'_, BackendState>) -> Result<RuntimeStatusDto, AppError> {
    let runtime = state
        .runtime
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    let status = runtime
        .status
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?
        .clone();
    Ok(status)
}

#[tauri::command]
fn start_pending_break(state: tauri::State<'_, BackendState>) -> Result<(), AppError> {
    let runtime = state
        .runtime
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    let Some(tx) = runtime.tx.clone() else {
        return Err(AppError::RuntimeNotRunning);
    };
    let _ = tx.send(RuntimeControl::StartPending);
    Ok(())
}

#[tauri::command]
fn snooze_pending_break(state: tauri::State<'_, BackendState>) -> Result<(), AppError> {
    let runtime = state
        .runtime
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    let Some(tx) = runtime.tx.clone() else {
        return Err(AppError::RuntimeNotRunning);
    };
    let _ = tx.send(RuntimeControl::SnoozePending);
    Ok(())
}

#[tauri::command]
fn trigger_break(kind: String, state: tauri::State<'_, BackendState>) -> Result<String, AppError> {
    let break_kind = parse_break_kind(&kind)?;
    let runtime = state
        .runtime
        .lock()
        .map_err(|e| AppError::Io(format!("mutex poisoned: {e}")))?;
    let Some(tx) = runtime.tx.clone() else {
        return Err(AppError::RuntimeNotRunning);
    };

    let _ = tx.send(RuntimeControl::StartBreak(break_kind));
    Ok(format!("break_triggered:{kind}"))
}

fn main() {
    configure_linux_webkit_runtime();

    let persistent = Arc::new(AppState::init().expect("failed to initialize state"));
    let backend = BackendState {
        persistent,
        runtime: Mutex::new(RuntimeController::default()),
    };

    tauri::Builder::default()
        .manage(backend)
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            list_profiles,
            save_profile,
            activate_profile,
            get_weekly_stats,
            set_startup_mode,
            start_runtime,
            stop_runtime,
            get_runtime_status,
            start_pending_break,
            snooze_pending_break,
            trigger_break
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "linux")]
fn configure_linux_webkit_runtime() {
    let wayland_session = std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|value| value.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false);

    if !wayland_session {
        return;
    }

    // Wayland + WebKit can fail with protocol errors on some drivers/GBM stacks.
    if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
        unsafe { std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1") };
    }
    if std::env::var("GSK_RENDERER").is_err() {
        unsafe { std::env::set_var("GSK_RENDERER", "cairo") };
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_webkit_runtime() {}
