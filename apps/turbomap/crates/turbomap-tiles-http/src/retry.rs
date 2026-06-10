//! Retry with exponential backoff + jitter for transient tile fetches.
//!
//! Transient failures — a dropped connection, a timeout, a momentary 5xx —
//! are the most common cause of tile-load errors on real (mobile) networks,
//! and a single retry usually succeeds. This module retries only
//! *retryable* errors (network/transport), with exponentially-growing
//! delays that are jittered so a fleet of clients doesn't synchronise into
//! a thundering herd after an outage.
//!
//! The policy is pure and unit-tested; [`retry`] wraps any fallible op.

use std::time::Duration;

use turbomap_core::TileError;

/// Backoff configuration. `max_retries` is the number of *additional*
/// attempts after the first, so `max_retries: 2` means up to 3 calls.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    /// A sensible mobile default: 3 extra attempts, 200 ms → 400 ms → 800 ms
    /// (jittered), capped at 5 s.
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(5),
        }
    }
}

impl RetryPolicy {
    /// No retries — the first failure is returned immediately. Used as the
    /// default for sources that haven't opted in, and in tests.
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            base_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
        }
    }

    /// The un-jittered exponential backoff for a 0-indexed attempt:
    /// `base * 2^attempt`, capped at `max_delay`.
    pub fn backoff(&self, attempt: u32) -> Duration {
        let factor = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
        let scaled = self
            .base_delay
            .checked_mul(factor.min(u32::MAX as u64) as u32)
            .unwrap_or(self.max_delay);
        scaled.min(self.max_delay)
    }

    /// The actual delay to sleep before `attempt`: equal-jitter — half the
    /// backoff plus a random point in the other half, so it lands in
    /// `[backoff/2, backoff]`. Spreads retries across clients without ever
    /// waiting longer than the cap.
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let b = self.backoff(attempt);
        let half = b / 2;
        let span = b.saturating_sub(half);
        half + jitter_within(span)
    }
}

/// Whether an error is worth retrying. Transport/network failures are
/// transient; a decode error (the bytes are bad) or an out-of-range zoom
/// (a permanent caller mistake) are not.
pub fn is_retryable(err: &TileError) -> bool {
    matches!(err, TileError::Network(_))
}

/// Run `op`, retrying retryable failures per `policy` with a real sleep
/// between attempts. Non-retryable errors and successes return immediately.
pub fn retry<T, F>(policy: &RetryPolicy, mut op: F) -> Result<T, TileError>
where
    F: FnMut() -> Result<T, TileError>,
{
    retry_with_sleep(policy, &mut op, std::thread::sleep)
}

/// Test seam: `retry` with an injectable sleep so unit tests can assert the
/// attempt/backoff behaviour without real delays.
fn retry_with_sleep<T, F, S>(policy: &RetryPolicy, op: &mut F, mut sleep: S) -> Result<T, TileError>
where
    F: FnMut() -> Result<T, TileError>,
    S: FnMut(Duration),
{
    let mut attempt = 0u32;
    loop {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => {
                if attempt >= policy.max_retries || !is_retryable(&e) {
                    return Err(e);
                }
                sleep(policy.delay_for(attempt));
                attempt += 1;
            }
        }
    }
}

/// A pseudo-random `Duration` in `[0, span]`. Jitter only needs to
/// de-correlate clients, not be cryptographic, so a cheap time-seeded
/// xorshift is plenty and keeps this dependency-free.
fn jitter_within(span: Duration) -> Duration {
    let nanos = span.as_nanos() as u64;
    if nanos == 0 {
        return Duration::ZERO;
    }
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    // xorshift64
    let mut x = seed | 1;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    Duration::from_nanos(x % (nanos + 1))
}

#[cfg(test)]
mod tests {
    //! Value boundary: a flaky network drops the first one or two requests
    //! for a tile, then recovers — the source must transparently succeed,
    //! while a genuinely-bad tile (decode error) or a permanent 4xx must
    //! fail fast without burning retries.
    use super::*;
    use std::cell::Cell;

    #[test]
    fn backoff_is_exponential_and_capped() {
        let p = RetryPolicy {
            max_retries: 10,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
        };
        assert_eq!(p.backoff(0), Duration::from_millis(100));
        assert_eq!(p.backoff(1), Duration::from_millis(200));
        assert_eq!(p.backoff(2), Duration::from_millis(400));
        assert_eq!(p.backoff(3), Duration::from_millis(800));
        // 1600ms would exceed the 1s cap.
        assert_eq!(p.backoff(4), Duration::from_secs(1));
        assert_eq!(p.backoff(40), Duration::from_secs(1), "no overflow panic");
    }

    #[test]
    fn jittered_delay_stays_within_half_to_full_backoff() {
        let p = RetryPolicy {
            max_retries: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
        };
        for attempt in 0..4 {
            let b = p.backoff(attempt);
            for _ in 0..50 {
                let d = p.delay_for(attempt);
                assert!(d >= b / 2, "delay {d:?} below half of {b:?}");
                assert!(d <= b, "delay {d:?} above backoff {b:?}");
            }
        }
    }

    fn flaky(fail_n: u32, err: fn() -> TileError) -> impl FnMut() -> Result<u32, TileError> {
        let calls = Cell::new(0u32);
        move || {
            let n = calls.get();
            calls.set(n + 1);
            if n < fail_n {
                Err(err())
            } else {
                Ok(n + 1) // returns the (1-based) attempt count that succeeded
            }
        }
    }

    fn run<F: FnMut() -> Result<u32, TileError>>(p: &RetryPolicy, mut op: F) -> (Result<u32, TileError>, u32) {
        let slept = Cell::new(0u32);
        let r = retry_with_sleep(p, &mut op, |_| slept.set(slept.get() + 1));
        (r, slept.get())
    }

    #[test]
    fn transient_network_failures_are_retried_until_success() {
        let p = RetryPolicy { max_retries: 3, ..RetryPolicy::none() };
        let (res, sleeps) = run(&p, flaky(2, || TileError::Network("reset".into())));
        assert_eq!(res.unwrap(), 3, "succeeds on the third attempt");
        assert_eq!(sleeps, 2, "slept once before each retry");
    }

    #[test]
    fn exhausting_retries_returns_the_last_error() {
        let p = RetryPolicy { max_retries: 2, ..RetryPolicy::none() };
        let (res, sleeps) = run(&p, flaky(99, || TileError::Network("down".into())));
        assert!(matches!(res, Err(TileError::Network(_))));
        assert_eq!(sleeps, 2, "two retries then give up (3 calls total)");
    }

    #[test]
    fn non_retryable_errors_fail_fast() {
        let p = RetryPolicy { max_retries: 5, ..RetryPolicy::none() };
        let (res, sleeps) = run(&p, flaky(99, || TileError::Decode("bad pbf".into())));
        assert!(matches!(res, Err(TileError::Decode(_))));
        assert_eq!(sleeps, 0, "a decode error is permanent — no retries");
    }
}
