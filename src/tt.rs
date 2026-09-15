//! Transposition table: what the search learned about a position last time.
//!
//! Two different wins from one table. The obvious one is skipping work -- if
//! this position was already searched at least this deep, the stored score can
//! be reused. The larger one for us is move ordering: the entry remembers which
//! move was best, and trying it first is what finally makes iterative deepening
//! pay for itself. Without a table the shallower iterations are pure overhead,
//! because nothing carries their conclusions into the next one.

use crate::search::{Bound, MoveKey};

/// A move as the table stores it, in 16 bits rather than a [`MoveKey`]'s 64.
///
/// A move key is the mover's occupancy before XOR after, so an ordinary move
/// sets exactly two bits -- the square left and the square arrived on -- which
/// pack into two six-bit indices. Castling moves the rook as well and so sets
/// four; there are only four such keys, so they are enumerated above the twelve
/// bits the ordinary case needs. Zero means "none", unambiguously, because an
/// ordinary move's two squares are always different.
///
/// This is what gets `Entry` down to 16 bytes, which is a power of two: entries
/// stop straddling cache lines, and `Hash n` can allocate exactly n megabytes.
type PackedMove = u16;

/// Set on a packed castling move. Ordinary moves never reach 0x1000.
const CASTLE_TAG: u16 = 0xF000;

/// The four castling move keys: king from and to, XOR rook from and to.
const CASTLE_KEYS: [u64; 4] = [
    (1 << 4) | (1 << 6) | (1 << 7) | (1 << 5),     // white kingside:  e1g1 h1f1
    (1 << 4) | (1 << 2) | (1 << 0) | (1 << 3),     // white queenside: e1c1 a1d1
    (1 << 60) | (1 << 62) | (1 << 63) | (1 << 61), // black kingside
    (1 << 60) | (1 << 58) | (1 << 56) | (1 << 59), // black queenside
];

fn pack_move(key: MoveKey) -> PackedMove {
    if key.count_ones() == 2 {
        let first = key.trailing_zeros() as u16;
        let second = (key & (key - 1)).trailing_zeros() as u16;
        return first | (second << 6);
    }
    let mut i = 0;
    while i < CASTLE_KEYS.len() {
        if key == CASTLE_KEYS[i] {
            return CASTLE_TAG | i as u16;
        }
        i += 1;
    }
    0 // Not a shape a move key can take; store nothing rather than a wrong move.
}

fn unpack_move(packed: PackedMove) -> MoveKey {
    if packed == 0 {
        return 0;
    }
    if (packed & CASTLE_TAG) == CASTLE_TAG {
        return CASTLE_KEYS[(packed & 3) as usize];
    }
    (1u64 << (packed & 63)) | (1u64 << ((packed >> 6) & 63))
}

/// Sixteen bytes: an 8-byte key, a 4-byte score, a packed move, a depth and a
/// bound. Keep it that way -- the size divides a cache line and a megabyte.
#[derive(Clone, Copy)]
struct Entry {
    key: u64,
    score: i32,
    /// The move that was best here, packed. 0 means "none stored".
    best_move: PackedMove,
    depth: u8,
    bound: Bound,
}

impl Default for Entry {
    fn default() -> Self {
        Entry { key: 0, score: 0, best_move: 0, depth: 0, bound: Bound::Exact }
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
        // The largest power of two that fits. `next_power_of_two() / 2` was
        // wrong for a `wanted` that is already a power of two -- it halved it,
        // so the table came out at half the size asked for.
        let len = if wanted < 2 { 0 } else { 1usize << wanted.ilog2() };
        TranspositionTable {
            entries: vec![Entry::default(); len],
            mask: len.saturating_sub(1),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.entries.is_empty()
    }

    /// How full the table is, in permille, sampled from the first thousand
    /// slots. A slot's index is its key's low bits, so any thousand are a fair
    /// sample of the whole. Reported as `hashfull` over UCI.
    ///
    /// Not the same measurement as Stockfish's, which counts only entries the
    /// current search has stored or probed. Ours carry no age, because the table
    /// lasts the game and an entry from an earlier move is as valid as a new one,
    /// so this counts every occupied slot. It climbs over a game rather than
    /// restarting each move. Since a store over another position always replaces
    /// it, the reading is also roughly the share of stores that now evict
    /// something.
    pub fn hashfull(&self) -> u32 {
        let sample = &self.entries[..self.entries.len().min(1000)];
        if sample.is_empty() {
            return 0;
        }
        let used = sample.iter().filter(|e| e.key != 0).count();
        (used * 1000 / sample.len()) as u32
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
        Some(unpack_move(entry.best_move))
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

        Some(Hit { best_move: unpack_move(entry.best_move), score })
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
        let packed = pack_move(best_move);
        let slot = &mut self.entries[key as usize & self.mask];
        if slot.key == key && u32::from(slot.depth) > depth {
            // Keep the deeper score -- it cost more and proves more -- but take
            // the move anyway. A score that came from a repetition is stored at
            // depth 0, so refusing outright freezes the move at whatever depth
            // last managed a store: the root of a Ruy Lopez kept reporting its
            // depth-6 move while the depth-11 search played something else, and
            // every iteration in between ordered the root by the stale one.
            if packed != 0 {
                slot.best_move = packed;
            }
            return;
        }
        *slot = Entry {
            key,
            best_move: packed,
            score,
            depth: depth.min(u8::MAX as u32) as u8,
            bound,
        };
    }
}
