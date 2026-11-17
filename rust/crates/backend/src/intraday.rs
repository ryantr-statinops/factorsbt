//! Backend helpers for scheduling orders against versioned minute-bar opens.

use factorsbt_contracts::{Identifier, UtcTimestamp};
use factorsbt_engine::{
    queue_order_intents, IntradayClockEvent, IntradayEventKind, PendingOrderError,
    PendingOrderGroup, ValidatedOrderIntent,
};

/// Queue order intents at the first eligible bar open strictly after the
/// decision time, constrained to the same frozen exchange session.
pub fn queue_intraday_order_intents(
    step_id: &Identifier,
    decision_session_date: &str,
    decision_time: &UtcTimestamp,
    events: &[IntradayClockEvent],
    intents: &[ValidatedOrderIntent],
) -> Result<Option<PendingOrderGroup>, PendingOrderError> {
    if intents.is_empty() {
        return Ok(None);
    }
    let eligible_at = events.iter().find(|event| {
        event.kind == IntradayEventKind::BarOpen
            && event.session_date == decision_session_date
            && factorsbt_contracts::compare_utc_timestamps(&event.at, decision_time).is_gt()
    });
    let Some(eligible_at) = eligible_at else {
        return Ok(None);
    };
    queue_order_intents(step_id, decision_time, &eligible_at.at, intents).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::{InstrumentId, SessionSchedule, SessionScheduleEntry, SessionType};
    use factorsbt_engine::IntradayClock;
    use rust_decimal::Decimal;

    fn make_clock() -> IntradayClock {
        let session = |date: &str, open: &str, close: &str| SessionScheduleEntry {
            session_date: date.into(),
            open_at: UtcTimestamp::new(open).unwrap(),
            close_at: UtcTimestamp::new(close).unwrap(),
            session_type: SessionType::Regular,
        };
        IntradayClock::new(&SessionSchedule {
            contract_version: "1.0.0".into(),
            calendar_id: Identifier::new("nasdaq-us-equities").unwrap(),
            calendar_version: Identifier::new("test-v1").unwrap(),
            timezone: "America/New_York".into(),
            source: "synthetic".into(),
            sessions: vec![
                session("2026-03-09", "2026-03-09T13:30:00Z", "2026-03-09T20:00:00Z"),
                session("2026-03-10", "2026-03-10T13:30:00Z", "2026-03-10T20:00:00Z"),
            ],
        })
        .unwrap()
    }

    fn intent() -> Vec<ValidatedOrderIntent> {
        vec![ValidatedOrderIntent {
            instrument_id: InstrumentId::new("XNAS_AAPL").unwrap(),
            signed_quantity: Decimal::ONE,
        }]
    }

    #[test]
    fn equality_is_skipped_and_next_strict_bar_open_is_selected() {
        let clock = make_clock();
        let group = queue_intraday_order_intents(
            &Identifier::new("step-1").unwrap(),
            "2026-03-09",
            &UtcTimestamp::new("2026-03-09T13:31:00Z").unwrap(),
            clock.events(),
            &intent(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(group.eligible_at.as_str(), "2026-03-09T13:32:00Z");
        assert!(factorsbt_contracts::compare_utc_timestamps(
            &group.eligible_at,
            &group.decision_time
        )
        .is_gt());
    }

    #[test]
    fn pending_intents_do_not_cross_the_session_boundary() {
        let clock = make_clock();
        let group = queue_intraday_order_intents(
            &Identifier::new("step-2").unwrap(),
            "2026-03-09",
            &UtcTimestamp::new("2026-03-09T20:00:00Z").unwrap(),
            clock.events(),
            &intent(),
        )
        .unwrap();
        assert!(group.is_none());
    }
}
