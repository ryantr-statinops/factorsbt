//! Versioned, offline exchange session calendars used by the intraday clock.

use crate::{Identifier, UtcTimestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSchedule {
    pub contract_version: String,
    pub calendar_id: Identifier,
    pub calendar_version: Identifier,
    pub timezone: String,
    pub source: String,
    pub sessions: Vec<SessionScheduleEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionScheduleEntry {
    pub session_date: String,
    pub open_at: UtcTimestamp,
    pub close_at: UtcTimestamp,
    pub session_type: SessionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Regular,
    EarlyClose,
}
