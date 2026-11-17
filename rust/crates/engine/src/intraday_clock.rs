//! Clock over a frozen, versioned one-minute session schedule.

use factorsbt_contracts::{compare_utc_timestamps, SessionSchedule, SessionType, UtcTimestamp};
use std::{cmp::Ordering, error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntradayEventKind {
    BarOpen,
    BarClose,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntradayClockEvent {
    pub at: UtcTimestamp,
    pub kind: IntradayEventKind,
    pub session_date: String,
    pub interval_start: UtcTimestamp,
    pub is_session_close: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntradayClockError {
    UnsupportedContract,
    UnsupportedTimezone,
    InvalidSchedule,
    UnorderedSessions,
}

impl fmt::Display for IntradayClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedContract => {
                f.write_str("intraday session contract version is unsupported")
            }
            Self::UnsupportedTimezone => f.write_str("intraday session timezone is unsupported"),
            Self::InvalidSchedule => f.write_str("intraday session schedule is invalid"),
            Self::UnorderedSessions => {
                f.write_str("session schedule must be sorted and non-overlapping")
            }
        }
    }
}

impl Error for IntradayClockError {}

#[derive(Debug, Clone)]
pub struct IntradayClock {
    events: Vec<IntradayClockEvent>,
    next: usize,
}

impl IntradayClock {
    pub fn new(schedule: &SessionSchedule) -> Result<Self, IntradayClockError> {
        if schedule.contract_version != "1.0.0" {
            return Err(IntradayClockError::UnsupportedContract);
        }
        if schedule.timezone != "America/New_York" {
            return Err(IntradayClockError::UnsupportedTimezone);
        }
        if schedule.sessions.is_empty() || schedule.source.trim().is_empty() {
            return Err(IntradayClockError::InvalidSchedule);
        }

        let mut events = Vec::new();
        let mut previous_close: Option<UtcTimestamp> = None;
        let mut previous_date: Option<&str> = None;
        for session in &schedule.sessions {
            if session.session_date.len() != 10
                || !session.open_at.as_str().starts_with(&session.session_date)
                || !session.close_at.as_str().starts_with(&session.session_date)
                || previous_date.is_some_and(|date| date >= session.session_date.as_str())
                || previous_close.as_ref().is_some_and(|close| {
                    compare_utc_timestamps(&session.open_at, close) == Ordering::Less
                })
            {
                return Err(IntradayClockError::UnorderedSessions);
            }
            let open = minute_index(&session.open_at)?;
            let close = minute_index(&session.close_at)?;
            let minutes = close
                .checked_sub(open)
                .ok_or(IntradayClockError::InvalidSchedule)?;
            if minutes <= 0 || minutes > 390 {
                return Err(IntradayClockError::InvalidSchedule);
            }
            match session.session_type {
                SessionType::Regular if minutes != 390 => {
                    return Err(IntradayClockError::InvalidSchedule)
                }
                SessionType::EarlyClose if minutes >= 390 => {
                    return Err(IntradayClockError::InvalidSchedule)
                }
                _ => {}
            }
            let open_clock = session.open_at.as_str().split_once('T').unwrap().1;
            let close_clock = session.close_at.as_str().split_once('T').unwrap().1;
            let expected_close = match (open_clock, session.session_type) {
                ("13:30:00Z", SessionType::Regular) => "20:00:00Z",
                ("14:30:00Z", SessionType::Regular) => "21:00:00Z",
                ("13:30:00Z", SessionType::EarlyClose) => "17:00:00Z",
                ("14:30:00Z", SessionType::EarlyClose) => "18:00:00Z",
                _ => return Err(IntradayClockError::InvalidSchedule),
            };
            if close_clock != expected_close {
                return Err(IntradayClockError::InvalidSchedule);
            }
            for index in 0..minutes {
                let start = timestamp_for_minute(open + index)?;
                let end = timestamp_for_minute(open + index + 1)?;
                events.push(IntradayClockEvent {
                    at: start.clone(),
                    kind: IntradayEventKind::BarOpen,
                    session_date: session.session_date.clone(),
                    interval_start: start.clone(),
                    is_session_close: false,
                });
                events.push(IntradayClockEvent {
                    at: end.clone(),
                    kind: IntradayEventKind::BarClose,
                    session_date: session.session_date.clone(),
                    interval_start: start,
                    is_session_close: index + 1 == minutes,
                });
            }
            previous_close = Some(session.close_at.clone());
            previous_date = Some(&session.session_date);
        }
        events.sort_by(|left, right| {
            compare_utc_timestamps(&left.at, &right.at)
                .then_with(|| event_priority(left.kind).cmp(&event_priority(right.kind)))
                .then_with(|| left.session_date.cmp(&right.session_date))
        });
        Ok(Self { events, next: 0 })
    }

    pub fn next_event(&mut self) -> Option<IntradayClockEvent> {
        let event = self.events.get(self.next)?.clone();
        self.next += 1;
        Some(event)
    }

    pub fn events(&self) -> &[IntradayClockEvent] {
        &self.events
    }
}

fn event_priority(kind: IntradayEventKind) -> u8 {
    match kind {
        IntradayEventKind::BarOpen => 0,
        IntradayEventKind::BarClose => 1,
    }
}

fn minute_index(value: &UtcTimestamp) -> Result<i64, IntradayClockError> {
    let (date, time) = value
        .as_str()
        .split_once('T')
        .ok_or(IntradayClockError::InvalidSchedule)?;
    if !time.ends_with("Z") {
        return Err(IntradayClockError::InvalidSchedule);
    }
    let mut date_parts = date.split('-');
    let (Some(y), Some(m), Some(d), None) = (
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
    ) else {
        return Err(IntradayClockError::InvalidSchedule);
    };
    let mut time_parts = time.trim_end_matches('Z').split(':');
    let (Some(h), Some(minute), Some(second), None) = (
        time_parts.next(),
        time_parts.next(),
        time_parts.next(),
        time_parts.next(),
    ) else {
        return Err(IntradayClockError::InvalidSchedule);
    };
    if second != "00" {
        return Err(IntradayClockError::InvalidSchedule);
    }
    let year = y
        .parse::<i64>()
        .map_err(|_| IntradayClockError::InvalidSchedule)?;
    let month = m
        .parse::<i64>()
        .map_err(|_| IntradayClockError::InvalidSchedule)?;
    let day = d
        .parse::<i64>()
        .map_err(|_| IntradayClockError::InvalidSchedule)?;
    let hour = h
        .parse::<i64>()
        .map_err(|_| IntradayClockError::InvalidSchedule)?;
    let minute = minute
        .parse::<i64>()
        .map_err(|_| IntradayClockError::InvalidSchedule)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return Err(IntradayClockError::InvalidSchedule);
    }
    Ok(days_from_civil(year, month, day) * 1440 + hour * 60 + minute)
}

fn timestamp_for_minute(index: i64) -> Result<UtcTimestamp, IntradayClockError> {
    let day = index.div_euclid(1440);
    let minute = index.rem_euclid(1440);
    let (year, month, date) = civil_from_days(day);
    UtcTimestamp::new(format!(
        "{year:04}-{month:02}-{date:02}T{:02}:{:02}:00Z",
        minute / 60,
        minute % 60
    ))
    .map_err(|_| IntradayClockError::InvalidSchedule)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era - 719468
}

fn civil_from_days(day_number: i64) -> (i64, i64, i64) {
    let shifted = day_number + 719468;
    let era = shifted.div_euclid(146097);
    let day_of_era = shifted - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::{Identifier, SessionScheduleEntry};

    fn schedule(sessions: Vec<SessionScheduleEntry>) -> SessionSchedule {
        SessionSchedule {
            contract_version: "1.0.0".into(),
            calendar_id: Identifier::new("nasdaq-us-equities").unwrap(),
            calendar_version: Identifier::new("fixture-2026a").unwrap(),
            timezone: "America/New_York".into(),
            source: "synthetic acceptance calendar".into(),
            sessions,
        }
    }

    fn session(
        date: &str,
        open: &str,
        close: &str,
        session_type: SessionType,
    ) -> SessionScheduleEntry {
        SessionScheduleEntry {
            session_date: date.into(),
            open_at: UtcTimestamp::new(open).unwrap(),
            close_at: UtcTimestamp::new(close).unwrap(),
            session_type,
        }
    }

    #[test]
    fn expands_regular_and_early_close_sessions_with_dst_utc_instants() {
        let schedule = schedule(vec![
            session(
                "2026-03-09",
                "2026-03-09T13:30:00Z",
                "2026-03-09T20:00:00Z",
                SessionType::Regular,
            ),
            session(
                "2026-11-02",
                "2026-11-02T14:30:00Z",
                "2026-11-02T21:00:00Z",
                SessionType::Regular,
            ),
            session(
                "2026-11-27",
                "2026-11-27T14:30:00Z",
                "2026-11-27T18:00:00Z",
                SessionType::EarlyClose,
            ),
        ]);
        let clock = IntradayClock::new(&schedule).unwrap();
        assert_eq!(clock.events().len(), 390 * 2 + 390 * 2 + 210 * 2);
        assert_eq!(clock.events()[0].at.as_str(), "2026-03-09T13:30:00Z");
        assert_eq!(clock.events()[0].kind, IntradayEventKind::BarOpen);
        assert_eq!(
            clock.events().iter().filter(|e| e.is_session_close).count(),
            3
        );
        assert!(clock
            .events()
            .iter()
            .any(|e| e.at.as_str() == "2026-11-02T14:30:00Z"));
        assert!(!clock
            .events()
            .iter()
            .any(|e| e.session_date == "2026-11-26"));
    }

    #[test]
    fn orders_open_before_close_at_equal_utc_boundary_and_rejects_bad_schedule() {
        let schedule_value = schedule(vec![session(
            "2026-03-09",
            "2026-03-09T13:30:00Z",
            "2026-03-09T20:00:00Z",
            SessionType::Regular,
        )]);
        let clock = IntradayClock::new(&schedule_value).unwrap();
        let boundary = clock
            .events()
            .iter()
            .filter(|e| e.at.as_str() == "2026-03-09T13:31:00Z")
            .collect::<Vec<_>>();
        assert_eq!(boundary[0].kind, IntradayEventKind::BarOpen);
        assert_eq!(boundary[1].kind, IntradayEventKind::BarClose);
        let invalid_schedule = schedule(vec![session(
            "2026-03-09",
            "2026-03-09T13:30:00Z",
            "2026-03-09T19:00:00Z",
            SessionType::Regular,
        )]);
        assert_eq!(
            IntradayClock::new(&invalid_schedule).unwrap_err(),
            IntradayClockError::InvalidSchedule
        );
    }
}
