use std::fmt;
use std::sync::Arc;

use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkKind {
    Scan,
    Write,
    Transfer,
}

impl WorkKind {
    fn label(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Write => "write",
            Self::Transfer => "transfer",
        }
    }
}

impl fmt::Display for WorkKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug)]
pub struct WorkPermit {
    kind: WorkKind,
    _guard: OwnedMutexGuard<()>,
}

impl WorkPermit {
    pub fn kind(&self) -> WorkKind {
        self.kind
    }
}

#[derive(Debug, Default)]
pub struct WorkLimiter {
    scan: Arc<Mutex<()>>,
    write: Arc<Mutex<()>>,
    transfer: Arc<Mutex<()>>,
}

impl WorkLimiter {
    pub fn try_begin(&self, kind: WorkKind) -> Result<WorkPermit, String> {
        let gate = match kind {
            WorkKind::Scan => Arc::clone(&self.scan),
            WorkKind::Write => Arc::clone(&self.write),
            WorkKind::Transfer => Arc::clone(&self.transfer),
        };
        let guard = gate
            .try_lock_owned()
            .map_err(|_| format!("BUSY: {kind} work is already running"))?;
        Ok(WorkPermit {
            kind,
            _guard: guard,
        })
    }

    pub fn try_scan(&self) -> Result<WorkPermit, String> {
        self.try_begin(WorkKind::Scan)
    }

    pub fn try_write(&self) -> Result<WorkPermit, String> {
        self.try_begin(WorkKind::Write)
    }

    pub fn try_transfer(&self) -> Result<WorkPermit, String> {
        self.try_begin(WorkKind::Transfer)
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkKind, WorkLimiter};

    #[test]
    fn same_work_kind_is_exclusive_until_permit_drops() {
        let limiter = WorkLimiter::default();
        let first = limiter.try_scan().expect("first scan");

        let error = limiter.try_scan().expect_err("second scan must be busy");
        assert_eq!(error, "BUSY: scan work is already running");

        drop(first);
        let next = limiter.try_scan().expect("released scan");
        assert_eq!(next.kind(), WorkKind::Scan);
    }

    #[test]
    fn different_work_kinds_can_progress_independently() {
        let limiter = WorkLimiter::default();

        let scan = limiter.try_scan().expect("scan");
        let write = limiter.try_write().expect("write");
        let transfer = limiter.try_transfer().expect("transfer");

        assert_eq!(scan.kind(), WorkKind::Scan);
        assert_eq!(write.kind(), WorkKind::Write);
        assert_eq!(transfer.kind(), WorkKind::Transfer);
    }

    #[test]
    fn busy_error_does_not_include_user_paths_or_payloads() {
        let limiter = WorkLimiter::default();
        let _first = limiter.try_transfer().expect("transfer");

        let error = limiter
            .try_transfer()
            .expect_err("second transfer must be busy");

        assert_eq!(error, "BUSY: transfer work is already running");
        assert!(!error.contains('\\'));
        assert!(!error.contains('/'));
    }
}
