#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreakTimerSettings {
    pub interval_seconds: u64,
    pub duration_seconds: u64,
    pub snooze_seconds: u64,
    pub enabled: bool,
}

impl BreakTimerSettings {
    pub fn new(interval_seconds: u64, duration_seconds: u64, snooze_seconds: u64) -> Self {
        Self {
            interval_seconds,
            duration_seconds,
            snooze_seconds,
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DailyLimitSettings {
    pub limit_seconds: u64,
    pub snooze_seconds: u64,
    pub reset_hour_local: u8,
    pub reset_minute_local: u8,
    pub enabled: bool,
}

impl DailyLimitSettings {
    pub fn reset_offset_seconds(&self) -> u64 {
        (self.reset_hour_local as u64 * 3600) + (self.reset_minute_local as u64 * 60)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockLevel {
    Soft,
    Medium,
    Strict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationSettings {
    pub desktop_enabled: bool,
    pub overlay_enabled: bool,
    pub sound_enabled: bool,
    pub sound_theme: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupSettings {
    pub xdg_autostart_enabled: bool,
    pub systemd_user_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub micro: BreakTimerSettings,
    pub rest: BreakTimerSettings,
    pub daily_limit: DailyLimitSettings,
    pub block_level: BlockLevel,
    pub notifications: NotificationSettings,
    pub startup: StartupSettings,
    pub active_profile_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            micro: BreakTimerSettings::new(180, 20, 150),
            rest: BreakTimerSettings::new(2700, 300, 180),
            daily_limit: DailyLimitSettings {
                limit_seconds: 14_400,
                snooze_seconds: 1_200,
                reset_hour_local: 4,
                reset_minute_local: 0,
                enabled: true,
            },
            block_level: BlockLevel::Medium,
            notifications: NotificationSettings {
                desktop_enabled: true,
                overlay_enabled: true,
                sound_enabled: true,
                sound_theme: "default".to_string(),
            },
            startup: StartupSettings {
                xdg_autostart_enabled: true,
                systemd_user_enabled: false,
            },
            active_profile_id: "default".to_string(),
        }
    }
}
