use factorsbt_backend::intraday::queue_intraday_order_intents;
use factorsbt_contracts::{
    DecimalString, HistoricalBar, Identifier, InstrumentId, SessionSchedule, SessionScheduleEntry,
    SessionType, UtcTimestamp,
};
use factorsbt_data::{check_session_coverage, DataQualityCode, SourceCoverageStatus};
use factorsbt_engine::{
    IntradayClock, IntradayEventKind, SimulationClock, SimulationEventKind, ValidatedOrderIntent,
};
use rust_decimal::Decimal;

fn session(date: &str, open: &str, close: &str, session_type: SessionType) -> SessionScheduleEntry {
    SessionScheduleEntry {
        session_date: date.into(),
        open_at: UtcTimestamp::new(open).unwrap(),
        close_at: UtcTimestamp::new(close).unwrap(),
        session_type,
    }
}

fn schedule() -> SessionSchedule {
    SessionSchedule {
        contract_version: "1.0.0".into(),
        calendar_id: Identifier::new("nasdaq-us-equities").unwrap(),
        calendar_version: Identifier::new("synthetic-acceptance-v1").unwrap(),
        timezone: "America/New_York".into(),
        source: "synthetic acceptance schedule".into(),
        // Monday after the US DST transition, Tuesday early close, Wednesday
        // intentionally absent as a synthetic holiday, then Thursday standard time.
        sessions: vec![
            session(
                "2026-03-09",
                "2026-03-09T13:30:00Z",
                "2026-03-09T20:00:00Z",
                SessionType::Regular,
            ),
            session(
                "2026-03-10",
                "2026-03-10T13:30:00Z",
                "2026-03-10T17:00:00Z",
                SessionType::EarlyClose,
            ),
            session(
                "2026-11-05",
                "2026-11-05T14:30:00Z",
                "2026-11-05T21:00:00Z",
                SessionType::Regular,
            ),
        ],
    }
}

fn bar(id: &str, start: &str, end: &str) -> HistoricalBar {
    HistoricalBar {
        instrument_id: InstrumentId::new(id).unwrap(),
        provider_symbol: "TEST".into(),
        interval_start: UtcTimestamp::new(start).unwrap(),
        interval_end: UtcTimestamp::new(end).unwrap(),
        available_at: UtcTimestamp::new(end).unwrap(),
        open: DecimalString::new("100").unwrap(),
        high: DecimalString::new("101").unwrap(),
        low: DecimalString::new("99").unwrap(),
        close: DecimalString::new("100.25").unwrap(),
        volume: DecimalString::new("1000").unwrap(),
        source_identity: Identifier::new("synthetic:TEST:bar").unwrap(),
        source_revision: 0,
    }
}

#[test]
fn frozen_calendar_emits_dst_and_early_close_events_without_holiday_events() {
    let calendar = schedule();
    let clock = IntradayClock::new(&calendar).unwrap();
    assert_eq!(clock.events().len(), (390 + 210 + 390) * 2);

    let opens = clock
        .events()
        .iter()
        .filter(|event| event.kind == IntradayEventKind::BarOpen)
        .collect::<Vec<_>>();
    assert_eq!(opens[0].at.as_str(), "2026-03-09T13:30:00Z");
    let early_close = clock
        .events()
        .iter()
        .find(|event| event.is_session_close && event.session_date == "2026-03-10")
        .unwrap();
    assert_eq!(early_close.at.as_str(), "2026-03-10T17:00:00Z");
    let standard_open = opens
        .iter()
        .find(|event| event.session_date == "2026-11-05")
        .unwrap();
    assert_eq!(standard_open.at.as_str(), "2026-11-05T14:30:00Z");
    assert!(!clock
        .events()
        .iter()
        .any(|event| event.session_date == "2026-03-11"));
}

#[test]
fn early_close_coverage_reports_real_gaps_and_distinguishes_incomplete_source() {
    let calendar = schedule();
    let early = &calendar.sessions[1];
    let instrument = InstrumentId::new("XNAS_AAPL").unwrap();
    let final_bar = bar("XNAS_AAPL", "2026-03-10T16:59:00Z", "2026-03-10T17:00:00Z");
    let complete_source = check_session_coverage(
        early,
        std::slice::from_ref(&instrument),
        std::slice::from_ref(&final_bar),
        SourceCoverageStatus::Complete,
    )
    .unwrap();
    assert_eq!(
        complete_source
            .diagnostics
            .iter()
            .filter(|d| d.code == DataQualityCode::MissingInterval)
            .count(),
        209
    );
    assert!(complete_source.all_final_closes_available);

    let incomplete_source =
        check_session_coverage(early, &[instrument], &[], SourceCoverageStatus::Incomplete)
            .unwrap();
    assert_eq!(incomplete_source.diagnostics.len(), 1);
    assert_eq!(
        incomplete_source.diagnostics[0].code,
        DataQualityCode::SourceRequestIncomplete
    );
    assert!(!incomplete_source.all_final_closes_available);
}

#[test]
fn order_eligibility_skips_equal_open_and_does_not_cross_sessions() {
    let clock = IntradayClock::new(&schedule()).unwrap();
    let intent = [ValidatedOrderIntent {
        instrument_id: InstrumentId::new("XNAS_AAPL").unwrap(),
        signed_quantity: Decimal::ONE,
    }];
    let selected = queue_intraday_order_intents(
        &Identifier::new("after-close-decision").unwrap(),
        "2026-03-09",
        &UtcTimestamp::new("2026-03-09T13:31:00Z").unwrap(),
        clock.events(),
        &intent,
    )
    .unwrap()
    .unwrap();
    assert_eq!(selected.eligible_at.as_str(), "2026-03-09T13:32:00Z");

    let at_close = queue_intraday_order_intents(
        &Identifier::new("close-decision").unwrap(),
        "2026-03-09",
        &UtcTimestamp::new("2026-03-09T20:00:00Z").unwrap(),
        clock.events(),
        &intent,
    )
    .unwrap();
    assert!(at_close.is_none());
}

#[test]
fn existing_daily_clock_keeps_its_event_sequence() {
    let mut clock = SimulationClock::new(
        &UtcTimestamp::new("2026-03-09T16:00:00Z").unwrap(),
        &UtcTimestamp::new("2026-03-10T16:00:00Z").unwrap(),
        "synthetic-daily-utc-v1",
    )
    .unwrap();
    let events = std::iter::from_fn(|| clock.next_event()).collect::<Vec<_>>();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].kind, SimulationEventKind::Close);
    assert_eq!(events[1].kind, SimulationEventKind::Open);
    assert_eq!(events[2].kind, SimulationEventKind::Close);
}

#[test]
fn missing_final_close_remains_missing_in_coverage() {
    // The engine-level flatten acceptance is exercised with real portfolio
    // state in its unit tests; this boundary confirms coverage never infers a close.
    let calendar = schedule();
    let early = &calendar.sessions[1];
    let result = check_session_coverage(
        early,
        &[InstrumentId::new("XNAS_AAPL").unwrap()],
        &[],
        SourceCoverageStatus::Complete,
    )
    .unwrap();
    assert!(!result.all_final_closes_available);
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == DataQualityCode::MissingInterval
            && d.interval_start.as_deref() == Some("2026-03-10T16:59:00Z")));
}
