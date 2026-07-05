//! Drain barrier that makes the Delete-All success boundary airtight.
//!
//! The problem it solves: a background operation (an import's post-commit
//! cascade, an agent categorization job) reads the ledger, then writes derived
//! state some time later. If a factory reset ("Delete All Data") lands in
//! between, those later writes would commit against a freshly-wiped ledger —
//! leaving stale user/derived state behind *after* Delete-All reported success.
//!
//! The barrier exposes two coordinated signals:
//!
//! - a monotonic **epoch**, bumped once per reset. A long or looping writer
//!   snapshots it when it starts and bails as soon as it notices the value
//!   changed. This is the prompt, best-effort layer — it stops wasted work fast.
//!
//! - a shared/exclusive **gate**. A writer holds a shared [`WriterLease`] across
//!   the critical section that spans its epoch re-check *and* its commit. A
//!   reset takes the **exclusive** guard via [`ResetBarrier::begin_reset`],
//!   which cannot be granted until every outstanding lease has drained. Because
//!   shared and exclusive access are mutually exclusive, a writer's
//!   `(re-check → commit)` can never interleave with a reset's `(bump → wipe)`:
//!
//!     * If the writer holds the lease first, the reset's exclusive acquire
//!       blocks until the writer commits and drops the lease; the wipe then runs
//!       and removes whatever the writer committed.
//!     * If the reset holds the exclusive guard first, the writer's lease
//!       acquire blocks until the wipe completes; the writer then re-checks the
//!       epoch, sees it advanced, and aborts without committing.
//!
//!   Either way, nothing an operation started against the previous epoch can
//!   survive past the moment Delete-All reports success.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

/// Shared, cloneable handle to the reset barrier. Lives inside [`crate::Db`] so
/// every writer and the reset path coordinate through one instance.
#[derive(Clone)]
pub struct ResetBarrier {
    epoch: Arc<AtomicU64>,
    gate: Arc<RwLock<()>>,
}

impl Default for ResetBarrier {
    fn default() -> Self {
        Self::new()
    }
}

impl ResetBarrier {
    pub fn new() -> Self {
        Self {
            epoch: Arc::new(AtomicU64::new(0)),
            gate: Arc::new(RwLock::new(())),
        }
    }

    /// The current ledger epoch. An operation snapshots this when it begins so
    /// it can later tell whether a reset happened in the meantime.
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    /// Acquire a shared writer lease bound to `start_epoch` — the epoch the
    /// operation captured when it began reading the ledger. The caller MUST:
    ///   1. hold the returned lease continuously across its commit, and
    ///   2. check [`WriterLease::superseded`] first and skip the commit if true.
    ///
    /// While any lease is outstanding, [`begin_reset`](Self::begin_reset)
    /// cannot complete — so a commit performed under a non-superseded lease is
    /// guaranteed to land *before* any wipe (and thus be wiped by it), never
    /// after.
    pub async fn writer_lease(&self, start_epoch: u64) -> WriterLease {
        let guard = Arc::clone(&self.gate).read_owned().await;
        WriterLease {
            _guard: guard,
            start_epoch,
            epoch: Arc::clone(&self.epoch),
        }
    }

    /// Begin a reset. Advances the epoch immediately (so looping writers bail
    /// promptly), then acquires the exclusive guard, which **blocks until every
    /// outstanding [`WriterLease`] has drained**. Hold the returned guard across
    /// the wipe; dropping it completes the reset. Once this returns and the wipe
    /// has committed, no lease acquired before the drain can still be writing,
    /// and any lease acquired after will observe the advanced epoch.
    pub async fn begin_reset(&self) -> ResetGuard {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let guard = Arc::clone(&self.gate).write_owned().await;
        ResetGuard { _guard: guard }
    }
}

/// A held shared lease. The commit it protects can only proceed while this is
/// alive; a reset draining the barrier waits for it to drop.
pub struct WriterLease {
    _guard: OwnedRwLockReadGuard<()>,
    start_epoch: u64,
    epoch: Arc<AtomicU64>,
}

impl WriterLease {
    /// True if a reset advanced the epoch since the operation captured
    /// `start_epoch`. When true the caller MUST NOT commit — the ledger it read
    /// no longer exists.
    pub fn superseded(&self) -> bool {
        self.epoch.load(Ordering::SeqCst) != self.start_epoch
    }
}

/// The exclusive guard held across a wipe. While it is alive no writer lease can
/// be granted.
pub struct ResetGuard {
    _guard: OwnedRwLockWriteGuard<()>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn a_reset_drains_an_in_flight_lease_before_completing() {
        let b = ResetBarrier::new();
        let start = b.epoch();
        // An in-flight writer holds a lease (mid-commit).
        let lease = b.writer_lease(start).await;

        // A reset starts: it bumps the epoch immediately, then must wait for the
        // outstanding lease to drain before its exclusive guard is granted.
        let b2 = b.clone();
        let reset = tokio::spawn(async move {
            let _g = b2.begin_reset().await; // blocks until `lease` drops
        });

        // Give the reset task a chance to run: epoch is already bumped, but it
        // cannot have finished because our lease is still held.
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!reset.is_finished(), "reset must block until the lease drains");
        assert_ne!(b.epoch(), start, "epoch is advanced the instant a reset begins");
        // The in-flight writer notices it was superseded and would skip its commit.
        assert!(lease.superseded());

        // Writer drains → reset can now complete.
        drop(lease);
        tokio::time::timeout(Duration::from_secs(1), reset)
            .await
            .expect("reset completes once the lease drains")
            .unwrap();
    }

    #[tokio::test]
    async fn a_lease_taken_after_a_reset_sees_the_new_epoch() {
        let b = ResetBarrier::new();
        let start = b.epoch();
        {
            let _g = b.begin_reset().await; // reset fully completes here
        }
        // A writer that began before the reset (start_epoch snapshot) but only
        // now reaches its commit acquires the lease and must see it's stale.
        let lease = b.writer_lease(start).await;
        assert!(
            lease.superseded(),
            "a pre-reset operation must observe the advanced epoch at commit time"
        );
    }

    #[tokio::test]
    async fn a_current_epoch_lease_is_not_superseded() {
        let b = ResetBarrier::new();
        let lease = b.writer_lease(b.epoch()).await;
        assert!(!lease.superseded(), "no reset happened → commit may proceed");
    }
}
