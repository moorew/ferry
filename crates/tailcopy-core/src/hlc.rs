//! Hybrid logical clock (SPEC 6.1).
//!
//! A timestamp is a `u64` laid out as `wall_ms << 16 | counter`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Remote timestamps further ahead of local wall time than this are clamped.
pub const MAX_SKEW_MS: u64 = 60_000;

const COUNTER_BITS: u32 = 16;
const COUNTER_MAX: u64 = (1 << COUNTER_BITS) - 1;
const WALL_MAX: u64 = (1 << 48) - 1;

/// Builds an HLC value from its parts.
pub const fn pack(wall_ms: u64, counter: u64) -> u64 {
    (wall_ms << COUNTER_BITS) | (counter & COUNTER_MAX)
}

/// Milliseconds part of an HLC value.
pub const fn wall_ms(hlc: u64) -> u64 {
    hlc >> COUNTER_BITS
}

/// Counter part of an HLC value.
pub const fn counter(hlc: u64) -> u64 {
    hlc & COUNTER_MAX
}

/// Source of wall time in milliseconds since the Unix epoch. Injected so tests are
/// deterministic.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

/// The system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// A clock that only moves when told to. Clones share the same time.
#[derive(Clone, Debug, Default)]
pub struct ManualClock(Arc<AtomicU64>);

impl ManualClock {
    pub fn new(now_ms: u64) -> Self {
        Self(Arc::new(AtomicU64::new(now_ms)))
    }

    pub fn set(&self, now_ms: u64) {
        self.0.store(now_ms, Ordering::SeqCst);
    }

    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Result of [`Hlc::receive`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Received {
    /// The new local clock value.
    pub hlc: u64,
    /// The remote wall time was more than [`MAX_SKEW_MS`] ahead and was clamped.
    pub clamped: bool,
}

/// HLC state. Persist [`Hlc::last`] after every event so it stays monotonic across
/// restarts.
pub struct Hlc {
    last: u64,
    clock: Arc<dyn Clock>,
}

impl Hlc {
    /// Starts from a persisted value (0 on first run).
    pub fn new(clock: Arc<dyn Clock>, last: u64) -> Self {
        Self { last, clock }
    }

    pub fn last(&self) -> u64 {
        self.last
    }

    pub fn now_ms(&self) -> u64 {
        self.clock.now_ms().min(WALL_MAX)
    }

    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// A local event.
    pub fn tick(&mut self) -> u64 {
        let now = self.now_ms();
        let (lw, lc) = (wall_ms(self.last), counter(self.last));
        self.last = if now > lw {
            pack(now, 0)
        } else {
            bump(lw, lc + 1)
        };
        self.last
    }

    /// Merges a remote timestamp.
    pub fn receive(&mut self, remote: u64) -> Received {
        let now = self.now_ms();
        let (mut rw, rc) = (wall_ms(remote), counter(remote));
        let limit = now + MAX_SKEW_MS;
        let clamped = rw > limit;
        if clamped {
            rw = limit;
        }
        let (lw, lc) = (wall_ms(self.last), counter(self.last));
        let w = now.max(lw).max(rw);
        let c = if w == lw && w == rw {
            lc.max(rc) + 1
        } else if w == lw {
            lc + 1
        } else if w == rw {
            rc + 1
        } else {
            0
        };
        self.last = bump(w, c);
        Received {
            hlc: self.last,
            clamped,
        }
    }
}

/// Packs `(wall, counter)`, carrying counter overflow into the wall.
fn bump(wall: u64, counter: u64) -> u64 {
    if counter > COUNTER_MAX {
        pack(wall + 1, 0)
    } else {
        pack(wall, counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hlc_at(ms: u64, last: u64) -> (Hlc, ManualClock) {
        let clock = ManualClock::new(ms);
        (Hlc::new(Arc::new(clock.clone()), last), clock)
    }

    #[test]
    fn tick_is_strictly_monotonic() {
        let (mut h, clock) = hlc_at(1_000, 0);
        let mut prev = 0;
        for i in 0..1000 {
            if i % 7 == 0 {
                clock.advance(1);
            }
            if i % 13 == 0 {
                // Wall clock steps backwards.
                clock.set(clock.now_ms().saturating_sub(5));
            }
            let t = h.tick();
            assert!(t > prev);
            prev = t;
        }
    }

    #[test]
    fn tick_uses_wall_when_ahead() {
        let (mut h, clock) = hlc_at(1_000, 0);
        assert_eq!(h.tick(), pack(1_000, 0));
        assert_eq!(h.tick(), pack(1_000, 1));
        clock.set(2_000);
        assert_eq!(h.tick(), pack(2_000, 0));
    }

    #[test]
    fn counter_overflow_carries_into_wall() {
        let (mut h, _) = hlc_at(1_000, pack(1_000, COUNTER_MAX));
        assert_eq!(h.tick(), pack(1_001, 0));
        let (mut h, _) = hlc_at(1_000, pack(1_000, COUNTER_MAX));
        assert_eq!(h.receive(pack(1_000, COUNTER_MAX)).hlc, pack(1_001, 0));
    }

    #[test]
    fn receive_rules() {
        // Remote ahead: take remote wall, counter + 1.
        let (mut h, _) = hlc_at(1_000, pack(900, 3));
        assert_eq!(h.receive(pack(1_500, 7)).hlc, pack(1_500, 8));
        // Equal walls: max counter + 1.
        let (mut h, _) = hlc_at(1_000, pack(1_500, 9));
        assert_eq!(h.receive(pack(1_500, 7)).hlc, pack(1_500, 10));
        // Local last ahead.
        let (mut h, _) = hlc_at(1_000, pack(2_000, 4));
        assert_eq!(h.receive(pack(1_500, 7)).hlc, pack(2_000, 5));
        // Wall clock ahead of both.
        let (mut h, _) = hlc_at(3_000, pack(2_000, 4));
        assert_eq!(h.receive(pack(1_500, 7)).hlc, pack(3_000, 0));
    }

    #[test]
    fn receive_clamps_skew() {
        let (mut h, _) = hlc_at(10_000, 0);
        let r = h.receive(pack(10_000 + MAX_SKEW_MS + 50_000, 5));
        assert!(r.clamped);
        assert_eq!(r.hlc, pack(10_000 + MAX_SKEW_MS, 6));
        let r = h.receive(pack(10_000 + MAX_SKEW_MS, 0));
        assert!(!r.clamped);
        assert!(r.hlc > pack(10_000 + MAX_SKEW_MS, 6));
    }

    #[test]
    fn restart_from_persisted_value_stays_monotonic() {
        let (mut h, _) = hlc_at(5_000, 0);
        let mut t = 0;
        for _ in 0..10 {
            t = h.tick();
        }
        // Restart with the wall clock behind the persisted value.
        let (mut h2, _) = hlc_at(4_000, h.last());
        assert!(h2.tick() > t);
    }
}
