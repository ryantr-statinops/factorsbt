//! Synthetic daily UTC clock for the accepted MVP calendar.

use factorsbt_contracts::{compare_utc_timestamps, UtcTimestamp};
use std::{cmp::Ordering, error::Error, fmt};

const CALENDAR_ID: &str = "synthetic-daily-utc-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationEventKind {
    Open,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockEvent {
    pub at: UtcTimestamp,
    pub kind: SimulationEventKind,
}

/// One reusable close-to-close interval. The opening event is the earliest
/// execution point for decisions produced at `start_close`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationStep {
    pub start_close: UtcTimestamp,
    pub execution_open: UtcTimestamp,
    pub end_close: UtcTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockError {
    UnsupportedCalendar,
    InvalidSessionBoundary,
    EndBeforeStart,
}

impl fmt::Display for ClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCalendar => f.write_str("unsupported synthetic calendar"),
            Self::InvalidSessionBoundary => {
                f.write_str("simulation interval must begin and end at a daily 16:00 UTC close")
            }
            Self::EndBeforeStart => f.write_str("simulation end precedes its start"),
        }
    }
}

impl Error for ClockError {}

/// Iterator over a run's event schedule. The run starts at the configured
/// close; each later calendar day emits open before close.
#[derive(Debug, Clone)]
pub struct SimulationClock {
    end_day: i64,
    current_day: i64,
    next_kind: Option<SimulationEventKind>,
}

impl SimulationClock {
    pub fn new(
        start_at: &UtcTimestamp,
        end_at: &UtcTimestamp,
        calendar_id: &str,
    ) -> Result<Self, ClockError> {
        if calendar_id != CALENDAR_ID {
            return Err(ClockError::UnsupportedCalendar);
        }
        let start_day = day_from_timestamp(start_at)?;
        let end_day = day_from_timestamp(end_at)?;
        if compare_utc_timestamps(end_at, start_at) == Ordering::Less {
            return Err(ClockError::EndBeforeStart);
        }
        if end_day < start_day {
            return Err(ClockError::EndBeforeStart);
        }
        Ok(Self {
            end_day,
            current_day: start_day,
            next_kind: Some(SimulationEventKind::Close),
        })
    }

    pub fn next_event(&mut self) -> Option<ClockEvent> {
        let kind = self.next_kind?;
        let at = timestamp_for(self.current_day, kind);
        match kind {
            SimulationEventKind::Close if self.current_day < self.end_day => {
                self.current_day += 1;
                self.next_kind = Some(SimulationEventKind::Open);
            }
            SimulationEventKind::Open => self.next_kind = Some(SimulationEventKind::Close),
            SimulationEventKind::Close => self.next_kind = None,
        }
        Some(ClockEvent { at, kind })
    }

    /// Advance the clock through the next complete close → open → close step.
    /// Returns `None` once the configured end close has been reached.
    pub fn next_step(&mut self) -> Option<SimulationStep> {
        let start = self.next_event()?;
        if start.kind != SimulationEventKind::Close {
            return None;
        }
        let open = self.next_event()?;
        if open.kind != SimulationEventKind::Open {
            return None;
        }
        let close = self.next_event()?;
        if close.kind != SimulationEventKind::Close {
            return None;
        }
        Some(SimulationStep {
            start_close: start.at,
            execution_open: open.at,
            end_close: close.at,
        })
    }
}

fn day_from_timestamp(timestamp: &UtcTimestamp) -> Result<i64, ClockError> {
    let Some((date, time)) = timestamp.as_str().split_once('T') else {
        return Err(ClockError::InvalidSessionBoundary);
    };
    if time != "16:00:00Z" {
        return Err(ClockError::InvalidSessionBoundary);
    }
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ClockError::InvalidSessionBoundary);
    };
    let year = year
        .parse::<i64>()
        .map_err(|_| ClockError::InvalidSessionBoundary)?;
    let month = month
        .parse::<i64>()
        .map_err(|_| ClockError::InvalidSessionBoundary)?;
    let day = day
        .parse::<i64>()
        .map_err(|_| ClockError::InvalidSessionBoundary)?;
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || day < 1
        || day > days_in_month(year, month)
    {
        return Err(ClockError::InvalidSessionBoundary);
    }
    Ok(days_from_civil(year, month, day))
}

fn timestamp_for(day_number: i64, kind: SimulationEventKind) -> UtcTimestamp {
    let (year, month, day) = civil_from_days(day_number);
    let clock = match kind {
        SimulationEventKind::Open => "09:00:00",
        SimulationEventKind::Close => "16:00:00",
    };
    UtcTimestamp::new(format!("{year:04}-{month:02}-{day:02}T{clock}Z"))
        .expect("generated synthetic timestamp is valid")
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

// Proleptic Gregorian calendar conversions (days relative to 1970-01-01).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
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
    year += if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timestamp(value: &str) -> UtcTimestamp {
        UtcTimestamp::new(value).unwrap()
    }

    #[test]
    fn emits_initial_close_then_each_later_open_before_close() {
        let mut clock = SimulationClock::new(
            &timestamp("2026-01-05T16:00:00Z"),
            &timestamp("2026-01-07T16:00:00Z"),
            CALENDAR_ID,
        )
        .unwrap();
        let events = std::iter::from_fn(|| clock.next_event()).collect::<Vec<_>>();
        let actual = events
            .iter()
            .map(|event| (event.at.as_str(), event.kind))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                ("2026-01-05T16:00:00Z", SimulationEventKind::Close),
                ("2026-01-06T09:00:00Z", SimulationEventKind::Open),
                ("2026-01-06T16:00:00Z", SimulationEventKind::Close),
                ("2026-01-07T09:00:00Z", SimulationEventKind::Open),
                ("2026-01-07T16:00:00Z", SimulationEventKind::Close),
            ]
        );
    }

    #[test]
    fn clock_handles_month_and_leap_day_boundaries() {
        let mut clock = SimulationClock::new(
            &timestamp("2024-02-28T16:00:00Z"),
            &timestamp("2024-03-01T16:00:00Z"),
            CALENDAR_ID,
        )
        .unwrap();
        let dates = std::iter::from_fn(|| clock.next_event())
            .map(|event| event.at.as_str().to_owned())
            .collect::<Vec<_>>();
        assert!(dates.contains(&"2024-02-29T09:00:00Z".to_owned()));
        assert!(dates.contains(&"2024-03-01T16:00:00Z".to_owned()));
    }

    #[test]
    fn rejects_unsupported_calendar_and_non_close_boundaries() {
        assert_eq!(
            SimulationClock::new(
                &timestamp("2026-01-05T16:00:00Z"),
                &timestamp("2026-01-06T16:00:00Z"),
                "exchange-calendar",
            )
            .unwrap_err(),
            ClockError::UnsupportedCalendar
        );
        assert_eq!(
            SimulationClock::new(
                &timestamp("2026-01-05T09:00:00Z"),
                &timestamp("2026-01-06T16:00:00Z"),
                CALENDAR_ID,
            )
            .unwrap_err(),
            ClockError::InvalidSessionBoundary
        );
    }
}
