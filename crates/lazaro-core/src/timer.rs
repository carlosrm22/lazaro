use crate::config::{BlockLevel, Settings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakKind {
    Micro,
    Rest,
    DailyLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakOutcome {
    Completed,
    Snoozed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineEvent {
    BreakDue(BreakKind),
    BreakStarted(BreakKind),
    BreakCompleted(BreakKind),
    BreakSnoozed(BreakKind, u64),
    DailyReset,
}

#[derive(Clone, Debug)]
struct OngoingBreak {
    kind: BreakKind,
    remaining_seconds: u64,
}

#[derive(Clone, Debug)]
pub struct TimerEngine {
    settings: Settings,
    micro_active: u64,
    rest_active: u64,
    daily_active: u64,
    micro_snooze_until: Option<u64>,
    rest_snooze_until: Option<u64>,
    daily_snooze_until: Option<u64>,
    active_break: Option<OngoingBreak>,
    last_reset_bucket: i64,
}

impl TimerEngine {
    pub fn new(settings: Settings, now_local_unix: u64) -> Self {
        let bucket =
            Self::daily_bucket(now_local_unix, settings.daily_limit.reset_offset_seconds());
        Self {
            settings,
            micro_active: 0,
            rest_active: 0,
            daily_active: 0,
            micro_snooze_until: None,
            rest_snooze_until: None,
            daily_snooze_until: None,
            active_break: None,
            last_reset_bucket: bucket,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    pub fn active_break_info(&self) -> Option<(BreakKind, u64)> {
        self.active_break
            .as_ref()
            .map(|active| (active.kind, active.remaining_seconds))
    }

    pub fn next_break_eta(&self, now_local_unix: u64) -> Option<(BreakKind, u64)> {
        if self.active_break.is_some() {
            return None;
        }

        let mut candidates: Vec<(BreakKind, u64)> = Vec::new();

        if self.settings.micro.enabled {
            let countdown = self
                .settings
                .micro
                .interval_seconds
                .saturating_sub(self.micro_active)
                .max(self.snooze_remaining(self.micro_snooze_until, now_local_unix));
            candidates.push((BreakKind::Micro, countdown));
        }

        if self.settings.rest.enabled {
            let countdown = self
                .settings
                .rest
                .interval_seconds
                .saturating_sub(self.rest_active)
                .max(self.snooze_remaining(self.rest_snooze_until, now_local_unix));
            candidates.push((BreakKind::Rest, countdown));
        }

        if self.settings.daily_limit.enabled {
            let countdown = self
                .settings
                .daily_limit
                .limit_seconds
                .saturating_sub(self.daily_active)
                .max(self.snooze_remaining(self.daily_snooze_until, now_local_unix));

            let until_reset = self.seconds_until_next_reset(
                now_local_unix,
                self.settings.daily_limit.reset_offset_seconds(),
            );

            if countdown < until_reset {
                candidates.push((BreakKind::DailyLimit, countdown));
            }
        }

        candidates
            .into_iter()
            .min_by_key(|(kind, countdown)| (*countdown, Self::kind_priority(*kind)))
    }

    pub fn on_activity(&mut self, active_seconds: u64, now_local_unix: u64) -> Vec<EngineEvent> {
        let mut events = Vec::new();
        if self.maybe_daily_reset(now_local_unix) {
            events.push(EngineEvent::DailyReset);
        }

        if active_seconds == 0 || self.active_break.is_some() {
            return events;
        }

        self.micro_active = self.micro_active.saturating_add(active_seconds);
        self.rest_active = self.rest_active.saturating_add(active_seconds);
        self.daily_active = self.daily_active.saturating_add(active_seconds);

        if let Some(kind) = self.next_due(now_local_unix) {
            events.push(EngineEvent::BreakDue(kind));
            if matches!(self.settings.block_level, BlockLevel::Strict) {
                events.extend(self.start_break(kind));
            }
        }

        events
    }

    pub fn start_break(&mut self, kind: BreakKind) -> Vec<EngineEvent> {
        if self.active_break.is_some() {
            return Vec::new();
        }
        let duration = match kind {
            BreakKind::Micro => self.settings.micro.duration_seconds,
            BreakKind::Rest => self.settings.rest.duration_seconds,
            BreakKind::DailyLimit => 60,
        };
        self.active_break = Some(OngoingBreak {
            kind,
            remaining_seconds: duration,
        });
        vec![EngineEvent::BreakStarted(kind)]
    }

    pub fn tick_break(&mut self, elapsed_seconds: u64) -> Vec<EngineEvent> {
        let mut events = Vec::new();
        let Some(active) = self.active_break.as_mut() else {
            return events;
        };

        if elapsed_seconds >= active.remaining_seconds {
            let kind = active.kind;
            self.active_break = None;
            self.complete_break(kind);
            events.push(EngineEvent::BreakCompleted(kind));
        } else {
            active.remaining_seconds -= elapsed_seconds;
        }

        events
    }

    pub fn snooze(&mut self, kind: BreakKind, now_local_unix: u64) -> Option<EngineEvent> {
        let until = match kind {
            BreakKind::Micro => now_local_unix.saturating_add(self.settings.micro.snooze_seconds),
            BreakKind::Rest => now_local_unix.saturating_add(self.settings.rest.snooze_seconds),
            BreakKind::DailyLimit => {
                now_local_unix.saturating_add(self.settings.daily_limit.snooze_seconds)
            }
        };

        match kind {
            BreakKind::Micro => self.micro_snooze_until = Some(until),
            BreakKind::Rest => self.rest_snooze_until = Some(until),
            BreakKind::DailyLimit => self.daily_snooze_until = Some(until),
        }

        Some(EngineEvent::BreakSnoozed(kind, until))
    }

    fn next_due(&self, now_local_unix: u64) -> Option<BreakKind> {
        if self.settings.micro.enabled
            && self.micro_active >= self.settings.micro.interval_seconds
            && !Self::is_snoozed(self.micro_snooze_until, now_local_unix)
        {
            return Some(BreakKind::Micro);
        }

        if self.settings.rest.enabled
            && self.rest_active >= self.settings.rest.interval_seconds
            && !Self::is_snoozed(self.rest_snooze_until, now_local_unix)
        {
            return Some(BreakKind::Rest);
        }

        if self.settings.daily_limit.enabled
            && self.daily_active >= self.settings.daily_limit.limit_seconds
            && !Self::is_snoozed(self.daily_snooze_until, now_local_unix)
        {
            return Some(BreakKind::DailyLimit);
        }

        None
    }

    fn kind_priority(kind: BreakKind) -> u8 {
        match kind {
            BreakKind::Micro => 0,
            BreakKind::Rest => 1,
            BreakKind::DailyLimit => 2,
        }
    }

    fn snooze_remaining(&self, until: Option<u64>, now_local_unix: u64) -> u64 {
        until
            .map(|value| value.saturating_sub(now_local_unix))
            .unwrap_or(0)
    }

    fn complete_break(&mut self, kind: BreakKind) {
        match kind {
            BreakKind::Micro => self.micro_active = 0,
            BreakKind::Rest => {
                self.rest_active = 0;
                self.micro_active = 0;
            }
            BreakKind::DailyLimit => {
                self.daily_active = 0;
                self.rest_active = 0;
                self.micro_active = 0;
            }
        }
    }

    fn is_snoozed(until: Option<u64>, now_local_unix: u64) -> bool {
        until.is_some_and(|value| now_local_unix < value)
    }

    fn maybe_daily_reset(&mut self, now_local_unix: u64) -> bool {
        let bucket = Self::daily_bucket(
            now_local_unix,
            self.settings.daily_limit.reset_offset_seconds(),
        );
        if bucket != self.last_reset_bucket {
            self.last_reset_bucket = bucket;
            self.daily_active = 0;
            self.daily_snooze_until = None;
            return true;
        }
        false
    }

    fn daily_bucket(now_local_unix: u64, reset_offset_seconds: u64) -> i64 {
        ((now_local_unix as i64 - reset_offset_seconds as i64) / 86_400) as i64
    }

    fn seconds_until_next_reset(&self, now_local_unix: u64, reset_offset_seconds: u64) -> u64 {
        let current_bucket = Self::daily_bucket(now_local_unix, reset_offset_seconds);
        let next_reset = (current_bucket + 1) * 86_400 + reset_offset_seconds as i64;
        if next_reset <= now_local_unix as i64 {
            0
        } else {
            (next_reset as u64).saturating_sub(now_local_unix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    #[test]
    fn micro_break_becomes_due_after_interval() {
        let settings = Settings::default();
        let mut engine = TimerEngine::new(settings, 0);

        let events = engine.on_activity(180, 180);
        assert_eq!(events, vec![EngineEvent::BreakDue(BreakKind::Micro)]);
    }

    #[test]
    fn strict_mode_autostarts_break() {
        let mut settings = Settings::default();
        settings.block_level = BlockLevel::Strict;
        let mut engine = TimerEngine::new(settings, 0);

        let events = engine.on_activity(180, 180);
        assert_eq!(
            events,
            vec![
                EngineEvent::BreakDue(BreakKind::Micro),
                EngineEvent::BreakStarted(BreakKind::Micro)
            ]
        );
    }

    #[test]
    fn snooze_delays_due_event() {
        let settings = Settings::default();
        let mut engine = TimerEngine::new(settings, 0);

        let _ = engine.on_activity(180, 180);
        engine.snooze(BreakKind::Micro, 180);

        let events = engine.on_activity(1, 200);
        assert!(events.is_empty());

        let events = engine.on_activity(1, 400);
        assert_eq!(events, vec![EngineEvent::BreakDue(BreakKind::Micro)]);
    }

    #[test]
    fn daily_reset_resets_limit_counter() {
        let settings = Settings::default();
        let mut engine = TimerEngine::new(settings, 0);

        let _ = engine.on_activity(14_400, 10_000);
        let events = engine.on_activity(1, 200_000);

        assert!(events.contains(&EngineEvent::DailyReset));
        assert!(!events.contains(&EngineEvent::BreakDue(BreakKind::DailyLimit)));
    }

    #[test]
    fn next_break_eta_prefers_earliest_kind() {
        let settings = Settings::default();
        let mut engine = TimerEngine::new(settings, 0);

        let _ = engine.on_activity(120, 120);
        let (kind, eta) = engine.next_break_eta(120).expect("expected eta");
        assert_eq!(kind, BreakKind::Micro);
        assert_eq!(eta, 60);
    }

    #[test]
    fn next_break_eta_accounts_for_snooze() {
        let settings = Settings::default();
        let mut engine = TimerEngine::new(settings, 0);

        let _ = engine.on_activity(180, 180);
        let _ = engine.snooze(BreakKind::Micro, 180);

        let (kind, eta) = engine.next_break_eta(200).expect("expected eta");
        assert_eq!(kind, BreakKind::Micro);
        assert_eq!(eta, 130);
    }
}
