//! Backend-generated request and run UUIDs.

use factorsbt_contracts::Uuid;
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static UUID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Produce a process-unique UUIDv4-shaped identifier without adding a runtime
/// dependency. These identifiers are correlation keys, not security tokens.
pub fn new_uuid_v4() -> Uuid {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = UUID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let digest = Sha256::digest(format!("{}:{now}:{sequence}", std::process::id()).as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Uuid::new(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
    .expect("SHA-256-derived UUID has valid wire form")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generated_request_and_run_ids_are_unique_valid_uuids() {
        let ids = (0..64).map(|_| new_uuid_v4()).collect::<Vec<_>>();
        assert!(ids.iter().all(|id| Uuid::new(id.as_str()).is_ok()));
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    }
}
