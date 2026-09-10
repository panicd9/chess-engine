//! Transposition table: what the search learned about a position last time.
//!
//! Two different wins from one table. The obvious one is skipping work -- if
//! this position was already searched at least this deep, the stored score can
//! be reused. The larger one for us is move ordering: the entry remembers which
//! move was best, and trying it first is what finally makes iterative deepening
//! pay for itself. Without a table the shallower iterations are pure overhead,
//! because nothing carries their conclusions into the next one.

use crate::search::{Bound, MoveKey};

#[derive(Clone, Copy)]
struct Entry {
    key: u64,
    /// The move that was best here, as a [`MoveKey`]. 0 means "none stored".
    best_move: MoveKey,
    score: i32,
    depth: u8,
    bound: Bound,
}

impl Default for Entry {
    fn default() -> Self {
        Entry { key: 0, best_move: 0, score: 0, depth: 0, bound: Bound::Exact }
    }
}

/// What a probe found. A hit always carries the stored move, because that is
/// useful for ordering even when the score cannot be trusted.
pub struct Hit {
    pub best_move: MoveKey,
    /// `Some` only when the entry is deep enough and its bound permits a cutoff.
    pub score: Option<i32>,
}

/// `Clone` exists so a game history can be handed to a search thread. Only ever
/// cloned while empty -- copying a sized table would defeat the point.
#[derive(Clone)]
pub struct TranspositionTable {
    entries: Vec<Entry>,
    /// `index = key & mask`, so the length is always a power of two.
    mask: usize,
}

impl TranspositionTable {
    /// A table holding roughly `megabytes` of entries, rounded down to a power
    /// of two. Zero gives a disabled table that never hits.
    pub fn new(megabytes: usize) -> Self {
        let wanted = megabytes * 1024 * 1024 / std::mem::size_of::<Entry>();
        let len = if wanted < 2 { 0 } else { wanted.next_power_of_two() / 2 };
        TranspositionTable {
            entries: vec![Entry::default(); len],
            mask: len.saturating_sub(1),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Forget everything. Called between games, not between moves: entries from
    /// earlier in the same game are still valid and worth keeping.
    pub fn clear(&mut self) {
        self.entries.iter_mut().for_each(|e| *e = Entry::default());
    }

    /// The move stored for this position, ignoring depth and bounds. Used to
    /// walk the principal variation, where any remembered move is better than
    /// none.
    pub fn best_move(&self, key: u64) -> Option<MoveKey> {
        if !self.is_enabled() {
            return None;
        }
        let entry = &self.entries[key as usize & self.mask];
        if entry.key != key || entry.best_move == 0 {
            return None;
        }
        Some(entry.best_move)
    }

    pub fn probe(&self, key: u64, depth: u32, alpha: i32, beta: i32) -> Option<Hit> {
        if !self.is_enabled() {
            return None;
        }
        let entry = &self.entries[key as usize & self.mask];
        if entry.key != key {
            return None; // Empty slot, or another position living here.
        }

        // The move is always worth having. The score is only usable if the
        // stored search was at least as deep as the one being done now, and if
        // its bound actually settles the question for this window.
        let score = if u32::from(entry.depth) >= depth {
            match entry.bound {
                Bound::Exact => Some(entry.score),
                Bound::Lower if entry.score >= beta => Some(entry.score),
                Bound::Upper if entry.score <= alpha => Some(entry.score),
                _ => None,
            }
        } else {
            None
        };

        Some(Hit { best_move: entry.best_move, score })
    }

    /// Depth-preferred replacement: a deeper result cost more to produce and is
    /// worth more, so it is only displaced by an equal-or-deeper one. An entry
    /// for a different position is always replaced, otherwise a stale key could
    /// hold a slot forever.
    pub fn store(
        &mut self,
        key: u64,
        depth: u32,
        score: i32,
        bound: Bound,
        best_move: MoveKey,
    ) {
        if !self.is_enabled() {
            return;
        }
        let slot = &mut self.entries[key as usize & self.mask];
        if slot.key == key && u32::from(slot.depth) > depth {
            return;
        }
        *slot = Entry {
            key,
            best_move,
            score,
            depth: depth.min(u8::MAX as u32) as u8,
            bound,
        };
    }
}
