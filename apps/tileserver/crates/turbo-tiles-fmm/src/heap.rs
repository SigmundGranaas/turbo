//! Narrow-band priority queue for FMM.
//!
//! ## What FMM needs from a heap
//!
//! The marching loop does three operations on the CONSIDERED set:
//!   1. `pop_min` to accept the smallest-arrival-time cell.
//!   2. `push` a previously-FAR cell when a neighbour relaxation
//!      gives it a finite arrival time.
//!   3. `decrease_key` when a neighbour relaxation produces a
//!      strictly lower arrival time than the cell's current key.
//!
//! Without `decrease_key`, the heap would accumulate stale duplicate
//! entries — pop them lazily and skip when the cell is already
//! ACCEPTED. That's the standard "lazy deletion" workaround and it
//! works for plain Dijkstra/A\*, but FMM's correctness proof depends
//! on each cell being accepted *exactly once* with its final value,
//! which is what `decrease_key` enforces.
//!
//! ## This implementation
//!
//! Binary min-heap with an explicit `idx_in_heap: Vec<u32>` map from
//! cell index to its position in the heap array. `decrease_key` is
//! O(log n) sift-up; `pop_min` is O(log n) sift-down. The position
//! map costs `4 × cells` bytes (e.g. ~6 MB for a 1.5 M-cell corridor)
//! which is acceptable; the alternative — a hash map — has worse
//! constants in the hot loop.
//!
//! The Yatziv-Mirebeau paged-bucket-queue O(n) variant is a phase-7
//! performance pass once correctness is locked in. Binary heap first
//! because every FMM textbook proves causality against a binary heap.

/// Sentinel meaning "this cell is not in the heap right now". Cell
/// counts are bounded by `u32::MAX - 1` ≈ 4.3 G cells, well above
/// anything we plan to solve.
pub(crate) const NOT_IN_HEAP: u32 = u32::MAX;

/// Each heap slot stores both the key (arrival time) and the cell
/// index it represents. Packing them together in a single `Vec`
/// keeps the sift-up/sift-down loops cache-friendly.
#[derive(Debug, Clone, Copy)]
struct HeapEntry {
    /// Arrival time `u` candidate. `f32` keeps memory low and the
    /// FMM error analysis already assumes O(h) so the extra f64
    /// precision wouldn't buy correctness.
    key: f32,
    /// Flat cell index into the grid (`GridShape::idx`).
    cell: u32,
}

/// Binary min-heap with O(log n) `decrease_key`. The `idx_in_heap`
/// map is sized to the cell count once at construction; the heap
/// `entries` vector grows on demand.
#[derive(Debug)]
pub struct NarrowBandHeap {
    entries: Vec<HeapEntry>,
    /// `idx_in_heap[cell] = position in entries[]`, or `NOT_IN_HEAP`.
    /// Sized to total cell count so a flat index lookup is O(1).
    idx_in_heap: Vec<u32>,
}

impl NarrowBandHeap {
    /// Construct a heap able to hold up to `cell_count` distinct
    /// cells. The position map is allocated up front.
    pub fn with_cells(cell_count: usize) -> Self {
        Self {
            entries: Vec::with_capacity(cell_count.min(1 << 20)),
            idx_in_heap: vec![NOT_IN_HEAP; cell_count],
        }
    }

    /// Number of cells currently in the heap.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert `cell` with priority `key`. Must not already be in
    /// the heap — use `decrease_key_or_insert` when you don't know.
    pub fn push(&mut self, key: f32, cell: u32) {
        debug_assert!(self.idx_in_heap[cell as usize] == NOT_IN_HEAP);
        let pos = self.entries.len() as u32;
        self.entries.push(HeapEntry { key, cell });
        self.idx_in_heap[cell as usize] = pos;
        self.sift_up(pos as usize);
    }

    /// Lower `cell`'s key. The cell must already be in the heap and
    /// `new_key` must be `<` its current key — debug-assert both.
    pub fn decrease_key(&mut self, cell: u32, new_key: f32) {
        let pos = self.idx_in_heap[cell as usize] as usize;
        debug_assert!(pos != NOT_IN_HEAP as usize, "decrease_key on absent cell");
        debug_assert!(new_key <= self.entries[pos].key, "decrease_key with larger key");
        self.entries[pos].key = new_key;
        self.sift_up(pos);
    }

    /// Combined "push if absent, else decrease". The hot relaxation
    /// path uses this when the caller doesn't track FAR vs CONSIDERED
    /// state separately. When the cell is in the heap, the new key
    /// is accepted only if it's strictly smaller — otherwise no-op.
    pub fn decrease_key_or_insert(&mut self, cell: u32, key: f32) {
        let pos = self.idx_in_heap[cell as usize];
        if pos == NOT_IN_HEAP {
            self.push(key, cell);
        } else if key < self.entries[pos as usize].key {
            self.entries[pos as usize].key = key;
            self.sift_up(pos as usize);
        }
    }

    /// Pop the smallest-key entry. Returns `(key, cell)` or `None`
    /// when empty. The cell's position in the map is reset to
    /// `NOT_IN_HEAP` so subsequent operations behave correctly.
    pub fn pop_min(&mut self) -> Option<(f32, u32)> {
        if self.entries.is_empty() {
            return None;
        }
        let top = self.entries[0];
        self.idx_in_heap[top.cell as usize] = NOT_IN_HEAP;
        let last = self.entries.pop().unwrap();
        if !self.entries.is_empty() {
            self.entries[0] = last;
            self.idx_in_heap[last.cell as usize] = 0;
            self.sift_down(0);
        }
        Some((top.key, top.cell))
    }

    /// Test-only check: every entry's `idx_in_heap` agrees with its
    /// actual position. Cheap O(n) walk. Phase 1 unit tests call
    /// this after a sequence of operations to catch off-by-one
    /// bugs in the sift loops.
    #[cfg(test)]
    fn check_invariants(&self) {
        for (pos, entry) in self.entries.iter().enumerate() {
            assert_eq!(
                self.idx_in_heap[entry.cell as usize] as usize, pos,
                "idx_in_heap[{}]={}, expected {}",
                entry.cell, self.idx_in_heap[entry.cell as usize], pos
            );
            // Heap order
            if pos > 0 {
                let parent = (pos - 1) / 2;
                assert!(
                    self.entries[parent].key <= entry.key,
                    "heap order violated at pos {} (key {}) vs parent {} (key {})",
                    pos, entry.key, parent, self.entries[parent].key
                );
            }
        }
    }

    #[inline]
    fn sift_up(&mut self, mut pos: usize) {
        while pos > 0 {
            let parent = (pos - 1) / 2;
            if self.entries[parent].key <= self.entries[pos].key {
                break;
            }
            self.entries.swap(parent, pos);
            self.idx_in_heap[self.entries[parent].cell as usize] = parent as u32;
            self.idx_in_heap[self.entries[pos].cell as usize] = pos as u32;
            pos = parent;
        }
    }

    #[inline]
    fn sift_down(&mut self, mut pos: usize) {
        let n = self.entries.len();
        loop {
            let left = 2 * pos + 1;
            let right = 2 * pos + 2;
            let mut smallest = pos;
            if left < n && self.entries[left].key < self.entries[smallest].key {
                smallest = left;
            }
            if right < n && self.entries[right].key < self.entries[smallest].key {
                smallest = right;
            }
            if smallest == pos {
                break;
            }
            self.entries.swap(pos, smallest);
            self.idx_in_heap[self.entries[pos].cell as usize] = pos as u32;
            self.idx_in_heap[self.entries[smallest].cell as usize] = smallest as u32;
            pos = smallest;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pops_in_nondecreasing_order() {
        let mut h = NarrowBandHeap::with_cells(1024);
        // Adversarial: 1000 random keys, all distinct cells.
        let mut rng = 0xC0FFEEu64;
        let mut keys: Vec<f32> = Vec::with_capacity(1000);
        for cell in 0..1000u32 {
            // xorshift for determinism, no `rand` dep
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let k = ((rng & 0xFFFF) as f32) / 16.0;
            keys.push(k);
            h.push(k, cell);
        }
        let mut prev = f32::NEG_INFINITY;
        let mut popped = 0;
        while let Some((k, _)) = h.pop_min() {
            assert!(k >= prev, "popped {} after {}", k, prev);
            prev = k;
            popped += 1;
        }
        assert_eq!(popped, 1000);
    }

    #[test]
    fn decrease_key_reorders() {
        let mut h = NarrowBandHeap::with_cells(16);
        h.push(10.0, 0);
        h.push(20.0, 1);
        h.push(30.0, 2);
        h.push(40.0, 3);
        h.check_invariants();
        // Drop cell 3 below cell 0 — must pop first.
        h.decrease_key(3, 5.0);
        h.check_invariants();
        assert_eq!(h.pop_min(), Some((5.0, 3)));
        assert_eq!(h.pop_min(), Some((10.0, 0)));
        assert_eq!(h.pop_min(), Some((20.0, 1)));
        assert_eq!(h.pop_min(), Some((30.0, 2)));
        assert_eq!(h.pop_min(), None);
    }

    #[test]
    fn decrease_key_or_insert_handles_both() {
        let mut h = NarrowBandHeap::with_cells(8);
        // Insert path
        h.decrease_key_or_insert(2, 7.0);
        assert_eq!(h.len(), 1);
        // Decrease path (smaller key)
        h.decrease_key_or_insert(2, 3.0);
        assert_eq!(h.len(), 1);
        assert_eq!(h.pop_min(), Some((3.0, 2)));
        // No-op path (larger key when already in heap)
        h.push(5.0, 4);
        h.decrease_key_or_insert(4, 9.0);
        assert_eq!(h.pop_min(), Some((5.0, 4)));
    }

    #[test]
    fn pop_from_empty_returns_none() {
        let mut h = NarrowBandHeap::with_cells(4);
        assert!(h.pop_min().is_none());
        h.push(1.0, 0);
        assert_eq!(h.pop_min(), Some((1.0, 0)));
        assert!(h.pop_min().is_none());
    }

    #[test]
    fn stress_invariant() {
        // Push, decrease, pop, repeat with adversarial sequence.
        let mut h = NarrowBandHeap::with_cells(64);
        for cell in 0..64u32 {
            h.push((64 - cell) as f32, cell);
        }
        h.check_invariants();
        // Decrease every other cell to half its key.
        for cell in (0..64u32).step_by(2) {
            h.decrease_key(cell, ((64 - cell) as f32) * 0.5);
        }
        h.check_invariants();
        let mut prev = f32::NEG_INFINITY;
        let mut count = 0;
        while let Some((k, _)) = h.pop_min() {
            assert!(k >= prev);
            prev = k;
            count += 1;
        }
        assert_eq!(count, 64);
    }
}
