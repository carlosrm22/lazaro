use std::collections::BTreeMap;

use crate::timer::{BreakKind, BreakOutcome};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DailyAggregate {
    pub active_seconds: u64,
    pub micro_done: u32,
    pub rest_done: u32,
    pub daily_limit_hits: u32,
    pub skipped: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeeklySummary {
    pub total_active_seconds: u64,
    pub micro_done: u32,
    pub rest_done: u32,
    pub daily_limit_hits: u32,
    pub skipped: u32,
}

#[derive(Clone, Debug, Default)]
pub struct AnalyticsStore {
    by_day: BTreeMap<i64, DailyAggregate>,
}

impl AnalyticsStore {
    pub fn record_activity(&mut self, day_index: i64, seconds: u64) {
        let entry = self.by_day.entry(day_index).or_default();
        entry.active_seconds = entry.active_seconds.saturating_add(seconds);
    }

    pub fn record_break(&mut self, day_index: i64, kind: BreakKind, outcome: BreakOutcome) {
        let entry = self.by_day.entry(day_index).or_default();
        match (kind, outcome) {
            (BreakKind::Micro, BreakOutcome::Completed) => entry.micro_done += 1,
            (BreakKind::Rest, BreakOutcome::Completed) => entry.rest_done += 1,
            (BreakKind::DailyLimit, BreakOutcome::Completed) => entry.daily_limit_hits += 1,
            (_, BreakOutcome::Skipped) => entry.skipped += 1,
            (_, BreakOutcome::Snoozed) => {}
        }
    }

    pub fn summarize_week_ending(&self, end_day_index: i64) -> WeeklySummary {
        let start = end_day_index - 6;
        let mut summary = WeeklySummary::default();
        for (_day, agg) in self.by_day.range(start..=end_day_index) {
            summary.total_active_seconds += agg.active_seconds;
            summary.micro_done += agg.micro_done;
            summary.rest_done += agg.rest_done;
            summary.daily_limit_hits += agg.daily_limit_hits;
            summary.skipped += agg.skipped;
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_week_counts_breaks_and_activity() {
        let mut store = AnalyticsStore::default();
        store.record_activity(10, 120);
        store.record_activity(11, 240);
        store.record_break(10, BreakKind::Micro, BreakOutcome::Completed);
        store.record_break(11, BreakKind::Rest, BreakOutcome::Completed);
        store.record_break(11, BreakKind::Micro, BreakOutcome::Skipped);

        let weekly = store.summarize_week_ending(11);
        assert_eq!(weekly.total_active_seconds, 360);
        assert_eq!(weekly.micro_done, 1);
        assert_eq!(weekly.rest_done, 1);
        assert_eq!(weekly.skipped, 1);
    }
}
