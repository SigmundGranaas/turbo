//! The resource lifecycle table — ONE place that answers "where is this
//! chunk?" (plan slice B1, replacing the state smeared across six collections
//! in three layers: `Scene.ingested`, FFI `queued`, host `inFlight`/`retryAt`,
//! GPU cache residency, fade bookkeeping — the home of every flicker/race bug
//! of June).
//!
//! Design rules:
//! - **Transitions are methods; illegal ones are `Err`, never silent.** The
//!   table cannot be driven into an inconsistent state, only refused.
//! - **No clocks.** Recency is a caller-supplied frame counter, so the table
//!   is deterministic and replayable in plain tests (the same reason
//!   `Date.now` is banned in the golden/sim harnesses).
//! - **Priorities are opaque `u64`s** (lower = more urgent). The explainable
//!   score that composes tier/SSE/motion lands in slice B2; the table only
//!   promises to order by it deterministically (ties break on the key).
//! - The coherence law from the tile pipeline ("a GPU eviction re-pends a
//!   still-wanted chunk — never a permanent hole") is a transition here
//!   ([`Lifecycle::evicted`]), not a bookkeeping convention.

use std::collections::HashMap;

use crate::chunk::ChunkKey;

/// Identifies one in-flight fetch, so a plan's `cancel` list and a late
/// `ingest` can name exactly which attempt they mean (a stale response for a
/// superseded request must not be mistaken for the current one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestId(pub u64);

/// Where a chunk sits in its life. The five phases of the plan, verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Wanted, not resident, no fetch running — eligible for the next plan.
    Desired,
    /// A fetch is in flight (transport owned by the host).
    Fetching,
    /// Bytes arrived; a codec is decoding off the render thread.
    Decoding,
    /// Wanted and usable by the renderer.
    Resident,
    /// Resident but no longer wanted — an eviction candidate, kept only while
    /// the byte budget allows.
    Retained,
}

/// A refused transition. The message names the phase the chunk was actually
/// in — the exact information the old scattered-sets design lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    /// The operation requires the chunk to exist in the table.
    UnknownChunk,
    /// The chunk was in `actual`, which `op` is not legal from.
    WrongPhase { op: &'static str, actual: Phase },
    /// `want` would grow the desired set past the configured capacity — the
    /// capacity governor's law (`desired ≤ capacity`) enforced at the door
    /// instead of hoped for.
    DesiredSetFull,
    /// The `RequestId` doesn't match the chunk's current attempt (a stale
    /// completion for a superseded fetch).
    StaleRequest,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    phase: Phase,
    /// Lower = more urgent. Meaningful for wanted, non-resident phases.
    priority: u64,
    /// Current fetch attempt (Fetching/Decoding).
    request: Option<RequestId>,
    /// Wanted-but-in-flight chunks the camera moved away from: still tracked
    /// (the response may yet arrive) but surfaced on the cancel list.
    stale: bool,
    /// Caller's frame counter at the last touch — recency for eviction order.
    last_touch: u64,
    /// Payload size once resident (0 before) — the byte ledger for budgets.
    bytes: u64,
}

/// Aggregate counts — the same shape the render trace publishes
/// (`FrameMetrics::tiles`), computed from the one table instead of six sets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhaseHistogram {
    pub desired: usize,
    pub fetching: usize,
    pub decoding: usize,
    pub resident: usize,
    pub retained: usize,
}

/// The table. One per streaming domain (the engine holds one; tests hold
/// many).
#[derive(Debug)]
pub struct Lifecycle {
    entries: HashMap<ChunkKey, Entry>,
    /// Max size of the *desired set* (wanted, non-resident chunks). The
    /// compile-time capacity proof (`capacity.rs`) generalized to a runtime
    /// governor the property tests pin.
    capacity: usize,
    next_request: u64,
}

impl Lifecycle {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity,
            next_request: 0,
        }
    }

    /// Declare a chunk wanted this frame with the given priority.
    /// - absent → `Desired`
    /// - `Retained` → `Resident` (it was already on the GPU; wanting it back
    ///   costs nothing — this is the retention win)
    /// - wanted phases → refresh priority/recency, clear staleness
    pub fn want(
        &mut self,
        key: ChunkKey,
        priority: u64,
        frame: u64,
    ) -> Result<Phase, LifecycleError> {
        if let Some(e) = self.entries.get_mut(&key) {
            e.priority = priority;
            e.last_touch = frame;
            e.stale = false;
            if e.phase == Phase::Retained {
                e.phase = Phase::Resident;
            }
            return Ok(e.phase);
        }
        if self.wanted_missing_count() >= self.capacity {
            return Err(LifecycleError::DesiredSetFull);
        }
        self.entries.insert(
            key,
            Entry {
                phase: Phase::Desired,
                priority,
                request: None,
                stale: false,
                last_touch: frame,
                bytes: 0,
            },
        );
        Ok(Phase::Desired)
    }

    /// Declare a chunk no longer wanted.
    /// - `Desired` → forgotten (it never cost anything)
    /// - `Fetching`/`Decoding` → marked stale (surfaces on [`Self::cancelable`])
    /// - `Resident` → `Retained` (eviction candidate)
    pub fn unwant(&mut self, key: ChunkKey) -> Result<(), LifecycleError> {
        let phase = self
            .entries
            .get(&key)
            .ok_or(LifecycleError::UnknownChunk)?
            .phase;
        match phase {
            Phase::Desired => {
                self.entries.remove(&key);
            }
            Phase::Fetching | Phase::Decoding => {
                self.entries.get_mut(&key).expect("present").stale = true;
            }
            Phase::Resident => {
                self.entries.get_mut(&key).expect("present").phase = Phase::Retained;
            }
            Phase::Retained => {}
        }
        Ok(())
    }

    /// `Desired` → `Fetching`; mints the attempt's [`RequestId`].
    pub fn fetch_started(&mut self, key: ChunkKey) -> Result<RequestId, LifecycleError> {
        let next = RequestId(self.next_request);
        let e = self
            .entries
            .get_mut(&key)
            .ok_or(LifecycleError::UnknownChunk)?;
        if e.phase != Phase::Desired {
            return Err(LifecycleError::WrongPhase {
                op: "fetch_started",
                actual: e.phase,
            });
        }
        self.next_request += 1;
        e.phase = Phase::Fetching;
        e.request = Some(next);
        Ok(next)
    }

    /// `Fetching` → `Decoding` (bytes handed to a codec).
    pub fn decode_started(
        &mut self,
        key: ChunkKey,
        request: RequestId,
    ) -> Result<(), LifecycleError> {
        let e = self
            .entries
            .get_mut(&key)
            .ok_or(LifecycleError::UnknownChunk)?;
        if e.request != Some(request) {
            return Err(LifecycleError::StaleRequest);
        }
        if e.phase != Phase::Fetching {
            return Err(LifecycleError::WrongPhase {
                op: "decode_started",
                actual: e.phase,
            });
        }
        e.phase = Phase::Decoding;
        Ok(())
    }

    /// `Decoding` (or `Fetching`, for the raw-ingest path that skips codecs)
    /// → `Resident`, recording the payload's byte cost. A chunk unwanted
    /// mid-flight lands as `Retained` — resident, but an eviction candidate.
    pub fn resident(
        &mut self,
        key: ChunkKey,
        request: RequestId,
        bytes: u64,
        frame: u64,
    ) -> Result<(), LifecycleError> {
        let e = self
            .entries
            .get_mut(&key)
            .ok_or(LifecycleError::UnknownChunk)?;
        if e.request != Some(request) {
            return Err(LifecycleError::StaleRequest);
        }
        match e.phase {
            Phase::Fetching | Phase::Decoding => {
                e.phase = if e.stale {
                    Phase::Retained
                } else {
                    Phase::Resident
                };
                e.stale = false;
                e.request = None;
                e.bytes = bytes;
                e.last_touch = frame;
                Ok(())
            }
            actual => Err(LifecycleError::WrongPhase {
                op: "resident",
                actual,
            }),
        }
    }

    /// A fetch/decode attempt failed. Still-wanted chunks return to `Desired`
    /// (retry policy lives above the table); stale ones are forgotten.
    pub fn failed(&mut self, key: ChunkKey, request: RequestId) -> Result<(), LifecycleError> {
        let e = self
            .entries
            .get_mut(&key)
            .ok_or(LifecycleError::UnknownChunk)?;
        if e.request != Some(request) {
            return Err(LifecycleError::StaleRequest);
        }
        match e.phase {
            Phase::Fetching | Phase::Decoding => {
                if e.stale {
                    self.entries.remove(&key);
                } else {
                    e.phase = Phase::Desired;
                    e.request = None;
                }
                Ok(())
            }
            actual => Err(LifecycleError::WrongPhase {
                op: "failed",
                actual,
            }),
        }
    }

    /// The GPU cache evicted this chunk's upload. The coherence law:
    /// - `Retained` → forgotten (that's what eviction candidates are for)
    /// - `Resident` (still wanted!) → back to `Desired`, so it re-pends on the
    ///   next plan — the "grey tile that never reloads" class of bug is a
    ///   transition here, not a convention callers must remember.
    pub fn evicted(&mut self, key: ChunkKey) -> Result<(), LifecycleError> {
        let phase = self
            .entries
            .get(&key)
            .ok_or(LifecycleError::UnknownChunk)?
            .phase;
        match phase {
            Phase::Retained => {
                self.entries.remove(&key);
                Ok(())
            }
            Phase::Resident => {
                if self.wanted_missing_count() >= self.capacity {
                    // Desired set is full; the chunk simply drops out and is
                    // re-wanted by a later frame's selection instead.
                    self.entries.remove(&key);
                } else {
                    let e = self.entries.get_mut(&key).expect("just present");
                    e.phase = Phase::Desired;
                    e.bytes = 0;
                }
                Ok(())
            }
            actual => Err(LifecycleError::WrongPhase {
                op: "evicted",
                actual,
            }),
        }
    }

    /// `Desired` chunks in priority order (then key order — total and
    /// deterministic), up to `limit`. The head of the next streaming plan.
    pub fn pending(&self, limit: usize) -> Vec<(ChunkKey, u64)> {
        let mut v: Vec<(ChunkKey, u64)> = self
            .entries
            .iter()
            .filter(|(_, e)| e.phase == Phase::Desired)
            .map(|(k, e)| (*k, e.priority))
            .collect();
        v.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        v.truncate(limit);
        v
    }

    /// In-flight attempts the camera moved away from — the plan's `cancel`
    /// list (the verb the old design didn't have).
    pub fn cancelable(&self) -> Vec<(ChunkKey, RequestId)> {
        let mut v: Vec<(ChunkKey, RequestId)> = self
            .entries
            .iter()
            .filter(|(_, e)| e.stale && matches!(e.phase, Phase::Fetching | Phase::Decoding))
            .filter_map(|(k, e)| e.request.map(|r| (*k, r)))
            .collect();
        v.sort();
        v
    }

    /// `Retained` chunks, least-recently-touched first — the eviction queue.
    pub fn eviction_candidates(&self) -> Vec<(ChunkKey, u64)> {
        let mut v: Vec<(ChunkKey, u64, u64)> = self
            .entries
            .iter()
            .filter(|(_, e)| e.phase == Phase::Retained)
            .map(|(k, e)| (*k, e.last_touch, e.bytes))
            .collect();
        v.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        v.into_iter().map(|(k, _, bytes)| (k, bytes)).collect()
    }

    pub fn phase_of(&self, key: ChunkKey) -> Option<Phase> {
        self.entries.get(&key).map(|e| e.phase)
    }

    /// Reconcile the want-set against this frame's selection: every tracked
    /// chunk that is currently wanted but fails `still_wanted` is
    /// [`Self::unwant`]ed. The per-frame companion to calling
    /// [`Self::want`] for the selected set.
    pub fn retain_wanted(&mut self, mut still_wanted: impl FnMut(ChunkKey) -> bool) {
        let stale: Vec<ChunkKey> = self
            .entries
            .iter()
            .filter(|(_, e)| {
                !e.stale
                    && matches!(
                        e.phase,
                        Phase::Desired | Phase::Fetching | Phase::Decoding | Phase::Resident
                    )
            })
            .map(|(k, _)| *k)
            .filter(|k| !still_wanted(*k))
            .collect();
        for k in stale {
            let _ = self.unwant(k);
        }
    }

    /// Legacy-shim delivery: a payload arrived WITHOUT a plan-issued request
    /// (today's hosts fetch on their own initiative — the pull/push contract
    /// predates the plan boundary). Infallible by design: whatever phase the
    /// chunk was in, it is now resident, wanted-ness preserved (unknown
    /// chunks land `Retained` — bytes we hold but never asked for). This
    /// method is DELETED in slice B3.4 when `ingest` becomes
    /// `RequestId`-keyed; nothing but the dual-write shim may call it.
    pub fn delivered_unrequested(&mut self, key: ChunkKey, bytes: u64, frame: u64) -> Phase {
        let e = self.entries.entry(key).or_insert(Entry {
            phase: Phase::Retained,
            priority: u64::MAX,
            request: None,
            stale: false,
            last_touch: frame,
            bytes: 0,
        });
        e.phase = match e.phase {
            Phase::Desired | Phase::Fetching | Phase::Decoding => {
                if e.stale {
                    Phase::Retained
                } else {
                    Phase::Resident
                }
            }
            keep @ (Phase::Resident | Phase::Retained) => keep,
        };
        e.stale = false;
        e.request = None;
        e.bytes = bytes;
        e.last_touch = frame;
        e.phase
    }

    /// Drop every chunk of `layer` — the layer was removed from the scene.
    pub fn forget_layer(&mut self, layer: crate::chunk::WorldLayerId) {
        self.entries.retain(|k, _| k.layer != layer);
    }

    /// The chunk a live fetch attempt belongs to. Linear scan — the table is
    /// a few hundred entries and this runs on host-reported completions, not
    /// per chunk per frame.
    pub fn key_of_request(&self, request: RequestId) -> Option<ChunkKey> {
        self.entries
            .iter()
            .find(|(_, e)| e.request == Some(request))
            .map(|(k, _)| *k)
    }

    /// The host honoured a cancellation (or abandoned the fetch for its own
    /// reasons). Stale attempts are forgotten — the chunk was unwanted;
    /// still-wanted attempts return to `Desired` so the next plan can
    /// restart them.
    pub fn cancelled(&mut self, key: ChunkKey, request: RequestId) -> Result<(), LifecycleError> {
        let e = self
            .entries
            .get_mut(&key)
            .ok_or(LifecycleError::UnknownChunk)?;
        if e.request != Some(request) {
            return Err(LifecycleError::StaleRequest);
        }
        match e.phase {
            Phase::Fetching | Phase::Decoding => {
                if e.stale {
                    self.entries.remove(&key);
                } else {
                    e.phase = Phase::Desired;
                    e.request = None;
                }
                Ok(())
            }
            actual => Err(LifecycleError::WrongPhase {
                op: "cancelled",
                actual,
            }),
        }
    }

    /// Total resident + retained payload bytes — the VRAM-ledger view.
    pub fn resident_bytes(&self) -> u64 {
        self.entries
            .values()
            .filter(|e| matches!(e.phase, Phase::Resident | Phase::Retained))
            .map(|e| e.bytes)
            .sum()
    }

    pub fn histogram(&self) -> PhaseHistogram {
        let mut h = PhaseHistogram::default();
        for e in self.entries.values() {
            match e.phase {
                Phase::Desired => h.desired += 1,
                Phase::Fetching => h.fetching += 1,
                Phase::Decoding => h.decoding += 1,
                Phase::Resident => h.resident += 1,
                Phase::Retained => h.retained += 1,
            }
        }
        h
    }

    /// Wanted chunks that are not yet usable — what the capacity governs.
    fn wanted_missing_count(&self) -> usize {
        self.entries
            .values()
            .filter(|e| {
                !e.stale && matches!(e.phase, Phase::Desired | Phase::Fetching | Phase::Decoding)
            })
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{NodeId, WorldLayerId};

    fn key(n: u64) -> ChunkKey {
        ChunkKey {
            layer: WorldLayerId(1),
            node: NodeId(n),
        }
    }

    #[test]
    fn happy_path_walks_all_five_phases() {
        let mut t = Lifecycle::with_capacity(8);
        let k = key(7);
        assert_eq!(t.want(k, 10, 1), Ok(Phase::Desired));
        let r = t.fetch_started(k).unwrap();
        assert_eq!(t.phase_of(k), Some(Phase::Fetching));
        t.decode_started(k, r).unwrap();
        t.resident(k, r, 4096, 2).unwrap();
        assert_eq!(t.phase_of(k), Some(Phase::Resident));
        assert_eq!(t.resident_bytes(), 4096);
        t.unwant(k).unwrap();
        assert_eq!(t.phase_of(k), Some(Phase::Retained));
        // Wanting it back is free — the retention win.
        assert_eq!(t.want(k, 5, 3), Ok(Phase::Resident));
    }

    #[test]
    fn illegal_transitions_are_refused_with_the_actual_phase() {
        let mut t = Lifecycle::with_capacity(8);
        let k = key(1);
        assert_eq!(t.fetch_started(k), Err(LifecycleError::UnknownChunk));
        t.want(k, 1, 0).unwrap();
        let r = t.fetch_started(k).unwrap();
        assert_eq!(
            t.fetch_started(k),
            Err(LifecycleError::WrongPhase {
                op: "fetch_started",
                actual: Phase::Fetching
            })
        );
        // A stale RequestId can't complete the current attempt.
        assert_eq!(
            t.resident(k, RequestId(r.0 + 999), 1, 1),
            Err(LifecycleError::StaleRequest)
        );
    }

    #[test]
    fn desired_set_capacity_is_enforced_at_the_door() {
        let mut t = Lifecycle::with_capacity(2);
        t.want(key(1), 1, 0).unwrap();
        t.want(key(2), 2, 0).unwrap();
        assert_eq!(t.want(key(3), 3, 0), Err(LifecycleError::DesiredSetFull));
        // Residency frees a slot (resident chunks are no longer "missing").
        let r = t.fetch_started(key(1)).unwrap();
        t.resident(key(1), r, 10, 1).unwrap();
        assert!(t.want(key(3), 3, 1).is_ok());
    }

    #[test]
    fn eviction_of_a_wanted_chunk_re_pends_it() {
        // The un_ingest coherence law: no permanent grey holes.
        let mut t = Lifecycle::with_capacity(4);
        let k = key(9);
        t.want(k, 1, 0).unwrap();
        let r = t.fetch_started(k).unwrap();
        t.resident(k, r, 100, 1).unwrap();
        t.evicted(k).unwrap();
        assert_eq!(t.phase_of(k), Some(Phase::Desired));
        assert_eq!(t.pending(10), vec![(k, 1)]);
        assert_eq!(t.resident_bytes(), 0);
    }

    #[test]
    fn unwanted_in_flight_surfaces_on_the_cancel_list_and_lands_retained() {
        let mut t = Lifecycle::with_capacity(4);
        let k = key(3);
        t.want(k, 1, 0).unwrap();
        let r = t.fetch_started(k).unwrap();
        t.unwant(k).unwrap();
        assert_eq!(t.cancelable(), vec![(k, r)]);
        // The response arrives anyway (host chose not to cancel): the bytes
        // are kept as Retained, not shown as wanted.
        t.resident(k, r, 50, 2).unwrap();
        assert_eq!(t.phase_of(k), Some(Phase::Retained));
        // A failed stale attempt would instead be forgotten entirely.
        let k2 = key(4);
        t.want(k2, 1, 2).unwrap();
        let r2 = t.fetch_started(k2).unwrap();
        t.unwant(k2).unwrap();
        t.failed(k2, r2).unwrap();
        assert_eq!(t.phase_of(k2), None);
    }

    #[test]
    fn pending_orders_by_priority_then_key_deterministically() {
        let mut t = Lifecycle::with_capacity(8);
        t.want(key(5), 20, 0).unwrap();
        t.want(key(1), 10, 0).unwrap();
        t.want(key(9), 10, 0).unwrap();
        assert_eq!(
            t.pending(10)
                .iter()
                .map(|(k, _)| k.node.0)
                .collect::<Vec<_>>(),
            vec![1, 9, 5]
        );
        assert_eq!(t.pending(2).len(), 2);
    }

    #[test]
    fn retain_wanted_unwants_exactly_the_dropped_keys() {
        let mut t = Lifecycle::with_capacity(8);
        t.want(key(1), 1, 0).unwrap();
        t.want(key(2), 2, 0).unwrap();
        let r = t.fetch_started(key(2)).unwrap();
        t.want(key(3), 3, 0).unwrap();
        let r3 = t.fetch_started(key(3)).unwrap();
        t.resident(key(3), r3, 10, 1).unwrap();
        // Keep only key(1): Desired key(2)'s fetch goes stale, Resident
        // key(3) becomes Retained.
        t.retain_wanted(|k| k.node.0 == 1);
        assert_eq!(t.phase_of(key(1)), Some(Phase::Desired));
        assert_eq!(t.cancelable(), vec![(key(2), r)]);
        assert_eq!(t.phase_of(key(3)), Some(Phase::Retained));
        // Idempotent: nothing further changes.
        t.retain_wanted(|k| k.node.0 == 1);
        assert_eq!(t.histogram().retained, 1);
    }

    #[test]
    fn delivered_unrequested_models_the_legacy_push_paths() {
        let mut t = Lifecycle::with_capacity(8);
        // Wanted → Resident.
        t.want(key(1), 1, 0).unwrap();
        assert_eq!(t.delivered_unrequested(key(1), 64, 1), Phase::Resident);
        // Unknown push → Retained (bytes we hold but never asked for).
        assert_eq!(t.delivered_unrequested(key(2), 32, 1), Phase::Retained);
        // Unwanted mid-flight → Retained, request cleared.
        t.want(key(3), 1, 1).unwrap();
        let _r = t.fetch_started(key(3)).unwrap();
        t.unwant(key(3)).unwrap();
        assert_eq!(t.delivered_unrequested(key(3), 16, 2), Phase::Retained);
        assert!(
            t.cancelable().is_empty(),
            "delivery clears the cancel entry"
        );
        assert_eq!(t.resident_bytes(), 64 + 32 + 16);
    }

    #[test]
    fn cancelled_forgets_stale_attempts_and_re_pends_wanted_ones() {
        let mut t = Lifecycle::with_capacity(8);
        // Stale attempt (unwanted mid-flight): cancellation forgets it.
        t.want(key(1), 1, 0).unwrap();
        let r1 = t.fetch_started(key(1)).unwrap();
        t.unwant(key(1)).unwrap();
        assert_eq!(t.key_of_request(r1), Some(key(1)));
        t.cancelled(key(1), r1).unwrap();
        assert_eq!(t.phase_of(key(1)), None);
        assert!(t.cancelable().is_empty());
        // Still-wanted attempt: host abandoned it → back to Desired.
        t.want(key(2), 1, 1).unwrap();
        let r2 = t.fetch_started(key(2)).unwrap();
        t.cancelled(key(2), r2).unwrap();
        assert_eq!(t.phase_of(key(2)), Some(Phase::Desired));
        // A RequestId that isn't the entry's live attempt is refused.
        assert_eq!(
            t.cancelled(key(2), RequestId(999)),
            Err(LifecycleError::StaleRequest)
        );
    }

    #[test]
    fn forget_layer_drops_only_that_layer() {
        let mut t = Lifecycle::with_capacity(8);
        t.want(key(1), 1, 0).unwrap();
        let other = ChunkKey {
            layer: WorldLayerId(9),
            node: NodeId(1),
        };
        t.want(other, 1, 0).unwrap();
        t.forget_layer(WorldLayerId(1));
        assert_eq!(t.phase_of(key(1)), None);
        assert_eq!(t.phase_of(other), Some(Phase::Desired));
    }

    /// The fuzz capstone (repo pattern — deterministic LCG, no proptest dep):
    /// ANY operation sequence keeps the invariants:
    /// 1. wanted-missing ≤ capacity;
    /// 2. histogram totals equal the entry count;
    /// 3. every Fetching/Decoding entry carries a RequestId, nothing else does;
    /// 4. replaying the same sequence reproduces the same views (determinism).
    #[test]
    fn fuzz_any_op_sequence_preserves_invariants_and_is_deterministic() {
        fn run(seed: u64, ops: usize) -> (Vec<(ChunkKey, u64)>, PhaseHistogram, u64) {
            let mut state = seed;
            let mut next = move || {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                state
            };
            let mut t = Lifecycle::with_capacity(16);
            let mut live_reqs: HashMap<ChunkKey, RequestId> = HashMap::new();
            for frame in 0..ops as u64 {
                let k = key(next() % 24);
                match next() % 6 {
                    0 => {
                        let _ = t.want(k, next() % 100, frame);
                    }
                    1 => {
                        let _ = t.unwant(k);
                    }
                    2 => {
                        if let Ok(r) = t.fetch_started(k) {
                            live_reqs.insert(k, r);
                        }
                    }
                    3 => {
                        if let Some(&r) = live_reqs.get(&k) {
                            let _ = t.decode_started(k, r);
                        }
                    }
                    4 => {
                        if let Some(&r) = live_reqs.get(&k) {
                            if t.resident(k, r, next() % 4096, frame).is_ok() {
                                live_reqs.remove(&k);
                            }
                        }
                    }
                    _ => {
                        let _ = t.evicted(k);
                    }
                }
                // Invariant sweep every step. Stale in-flight entries sit
                // outside the wanted budget (they're awaiting cancellation),
                // so the governed quantity is wanted-missing = in-motion
                // phases minus the cancel list.
                let h = t.histogram();
                let wanted_missing = h.desired + h.fetching + h.decoding - t.cancelable().len();
                assert!(wanted_missing <= 16, "capacity breached: {h:?}");
                let total = h.desired + h.fetching + h.decoding + h.resident + h.retained;
                assert_eq!(total, t.entries.len());
                for (k, e) in &t.entries {
                    match e.phase {
                        Phase::Fetching | Phase::Decoding => {
                            assert!(e.request.is_some(), "{k:?} in flight without a request")
                        }
                        _ => assert!(e.request.is_none(), "{k:?} carries a dead request"),
                    }
                }
            }
            (t.pending(32), t.histogram(), t.resident_bytes())
        }
        for seed in [1u64, 0x9E37_79B9, 42, 7_777_777] {
            assert_eq!(
                run(seed, 500),
                run(seed, 500),
                "seed {seed} not deterministic"
            );
        }
    }
}
