//! A pawn hash table: the pawn-only part of the evaluation, cached.
//!
//! Pawn structure barely changes from one node to the next -- most moves do not
//! touch a pawn at all -- so the same structure is evaluated over and over.
//! Caching it on a key derived from the two pawn bitboards turns that into one
//! lookup. The Chess Programming Wiki's *Pawn Hash Table* reports that "a few K"
//! entries reach a hit rate "above 95% or even 99% for most positions".
//!
//! Measured here before writing any of it, which is what justified it:
//!
//! ```text
//! evaluate()        148.3 ns        (middlegame)
//! passed_pawns()     47.1 ns        31.7% of it
//! the key             1.4 ns        3% of the term
//! ```
//!
//! **The key is mixed from the bitboards rather than maintained incrementally.**
//! A real engine folds a pawn key into its Zobrist hash as moves are made, but
//! this one is copy-make: `Chessboard` is `Copy` and cloned at every node, so its
//! size is on the hot path and `tests/regressions.rs::board_stays_small` guards
//! it. Mixing two `u64`s costs 1.4 ns and adds nothing to the board.
//!
//! **The weight is part of the key.** `PASSED_PAWN_SCALE` is settable at runtime
//! and `examples/texel` sweeps it in a loop, so a cache keyed on pawns alone
//! would serve values fitted with the previous weight. Folding the scale into the
//! key invalidates the old entries for free.
//!
//! The cache cannot change what the evaluation returns: an entry is used only
//! when the full 64-bit key matches, and the key is a function of exactly the
//! three things the score depends on. `examples/equiv` must stay byte-identical.

use std::cell::Cell;

use crate::chessboard::{Chessboard, FILE_MASKS};
use crate::eval::weights;

/// 4096 entries, 64 KB -- small enough to stay in L2, and past the point where
/// the wiki says the hit rate stops improving.
const BITS: usize = 14;
const SIZE: usize = 1 << BITS;
const MASK: usize = SIZE - 1;

#[derive(Clone, Copy)]
struct Entry {
    key: u64,
    score: i32,
}

const EMPTY: Cell<Entry> = Cell::new(Entry { key: 0, score: 0 });

thread_local! {
    /// Per thread, so this stays correct if the search is ever parallelised --
    /// unlike a global, which would need synchronising on the hot path.
    //
    // `const`, and it matters far more than it looks. With the lazy initializer
    // `= [EMPTY; SIZE]` the engine searched 46% slower: identical node counts,
    // 1.46x the time, on 24 of 24 positions. With `const` it is 0.98x of the
    // uncached engine, which is the gain the cache promised. Why the lazy form
    // costs this much is not established -- only that this one word removes it.
    //
    // It went unseen because it was timed through the library, on the main
    // thread (`examples/bench_all`, `examples/pawnprobe`). The engine searches on
    // a freshly spawned thread (`uci.rs`), and that is where it cost 1.46x. Time
    // speed changes through the UCI binary.
    static TABLE: [Cell<Entry>; SIZE] = const { [EMPTY; SIZE] };
}

/// Hit/miss counting, off unless the `pawnstats` feature is on: a thread-local
/// increment on every probe is not something the shipped engine should pay for
/// an instrument. Build the hit-rate example with
/// `cargo run --release --features pawnstats --example pawnhitrate`.
#[cfg(feature = "pawnstats")]
mod stats_impl {
    use std::cell::Cell;
    thread_local! {
        static HITS: Cell<u64> = const { Cell::new(0) };
        static MISSES: Cell<u64> = const { Cell::new(0) };
    }
    #[inline]
    pub fn hit() { HITS.with(|h| h.set(h.get() + 1)); }
    #[inline]
    pub fn miss() { MISSES.with(|m| m.set(m.get() + 1)); }
    pub fn get() -> (u64, u64) { (HITS.with(|h| h.get()), MISSES.with(|m| m.get())) }
    pub fn reset() { HITS.with(|h| h.set(0)); MISSES.with(|m| m.set(0)); }
}

#[cfg(feature = "pawnstats")]
pub fn stats() -> (u64, u64) { stats_impl::get() }
#[cfg(feature = "pawnstats")]
pub fn reset_stats() { stats_impl::reset() }

#[inline(always)]
fn record_hit() {
    #[cfg(feature = "pawnstats")]
    stats_impl::hit();
}

#[inline(always)]
fn record_miss() {
    #[cfg(feature = "pawnstats")]
    stats_impl::miss();
}

/// Bonus for a passed pawn, by the rank it has reached (from its own side's
/// point of view). Scaled by `weights::PASSED_PAWN_SCALE`.
///
/// These are half the values originally guessed at. Doubling them measured
/// clearly worse (-53 Elo), so the first set was too generous. Halving looked
/// like a gain in a 160-game run but did not reproduce over 240 games at a
/// longer control (0.0 +/- 38), so treat this as "no worse, and safer" rather
/// than a tuned improvement. Ten candidates were tested at +/-50 error bars,
/// which is enough for one to look significant by chance.
const PASSED_PAWN_BY_RANK: [i32; 8] = [0, 3, 5, 10, 20, 35, 60, 0];

/// Every weight the cached score depends on. Read once per probe and folded
/// into the key, so changing one by `setoption` -- which `examples/texel` does in
/// a loop -- cannot serve a score fitted with the previous value.
#[derive(Clone, Copy, PartialEq)]
struct PawnWeights {
    passed_scale: i32,
    doubled: i32,
    isolated_half_open: i32,
}

impl PawnWeights {
    #[inline]
    fn current() -> Self {
        PawnWeights {
            passed_scale: weights::get(&weights::PASSED_PAWN_SCALE),
            doubled: weights::get(&weights::DOUBLED_PAWN),
            isolated_half_open: weights::get(&weights::ISOLATED_HALF_OPEN_PAWN),
        }
    }

    #[inline]
    fn mix(self) -> u64 {
        (self.passed_scale as u64)
            .wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ (self.doubled as u64).wrapping_mul(0xA0761D6478BD642F)
            ^ (self.isolated_half_open as u64).wrapping_mul(0xE7037ED1A0B428DB)
    }
}

/// Identify a pawn structure, together with the weights it will be scored with.
///
/// Bit 0 is forced set so that no real key can be zero, which is what an unused
/// slot holds; the index is taken from the high bits so that forcing a low bit
/// does not halve the table.
#[inline]
fn key(white_pawns: u64, black_pawns: u64, w: PawnWeights) -> u64 {
    let mut x = white_pawns.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ black_pawns
            .rotate_left(32)
            .wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x ^ w.mix()) | 1
}

/// The pawn-only evaluation, from white's point of view.
///
/// Everything cached here must depend on **nothing but the two pawn bitboards**
/// and the weights folded into the key. King safety is deliberately not here: it
/// reads the king squares and the heavy material left on the board, so it is not
/// a function of the pawn structure alone.
pub fn passed_pawns(cb: &Chessboard) -> i32 {
    let w = PawnWeights::current();
    let k = key(cb.white_pawns, cb.black_pawns, w);
    let index = (k >> 32) as usize & MASK;

    TABLE.with(|table| {
        let slot = &table[index];
        let entry = slot.get();
        if entry.key == k {
            record_hit();
            return entry.score;
        }
        record_miss();
        let score = compute(cb.white_pawns, cb.black_pawns, w);
        slot.set(Entry { key: k, score });
        score
    })
}

/// A pawn is passed when no enemy pawn stands on its file or either adjacent
/// file anywhere ahead of it, so nothing can block or capture it on the way.
fn compute(white_pawns: u64, black_pawns: u64, w: PawnWeights) -> i32 {
    let scale = w.passed_scale;
    let mut score = structure(black_pawns, white_pawns, w) - structure(white_pawns, black_pawns, w);

    let mut pawns = white_pawns;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let rank = square / 8;
        if rank < 7 && (black_pawns & blocking_mask(square, true)) == 0 {
            score += PASSED_PAWN_BY_RANK[rank] * scale / 100;
        }
    }

    let mut pawns = black_pawns;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let rank = square / 8;
        if rank > 0 && (white_pawns & blocking_mask(square, false)) == 0 {
            score -= PASSED_PAWN_BY_RANK[7 - rank] * scale / 100;
        }
    }

    score
}

/// Structural penalties for one side, as a positive number to be subtracted.
///
/// **Doubled**: every pawn beyond the first on a file. **Isolated on a half-open
/// file**: a pawn with no friendly pawn on either neighbouring file, on a file no
/// enemy pawn stands on. The split matters -- plain isolation measured *harmless*
/// (a positive coefficient); the damage is the half-open file, where no pawn can
/// ever defend it and an enemy rook already bears on it.
fn structure(pawns: u64, enemy: u64, w: PawnWeights) -> i32 {
    if w.doubled == 0 && w.isolated_half_open == 0 {
        return 0;
    }
    let mut penalty = 0;
    for file in 0..8 {
        let mask = FILE_MASKS[file];
        let count = (pawns & mask).count_ones() as i32;
        if count == 0 {
            continue;
        }
        penalty += (count - 1) * w.doubled;
        let mut neighbours = 0u64;
        if file > 0 {
            neighbours |= FILE_MASKS[file - 1];
        }
        if file < 7 {
            neighbours |= FILE_MASKS[file + 1];
        }
        if pawns & neighbours == 0 && enemy & mask == 0 {
            penalty += count * w.isolated_half_open;
        }
    }
    penalty
}

/// The counts the two structural weights multiply, white minus black. Used to
/// prove the Rust agrees with `analysis/features.py`, which is what the fitted
/// weights were fitted against.
pub fn structure_counts(cb: &Chessboard) -> (i32, i32) {
    let one = PawnWeights { passed_scale: 0, doubled: 1, isolated_half_open: 0 };
    let two = PawnWeights { passed_scale: 0, doubled: 0, isolated_half_open: 1 };
    (
        structure(cb.white_pawns, cb.black_pawns, one) - structure(cb.black_pawns, cb.white_pawns, one),
        structure(cb.white_pawns, cb.black_pawns, two) - structure(cb.black_pawns, cb.white_pawns, two),
    )
}

/// The squares an enemy pawn would have to occupy to stop this one: the pawn's
/// own file plus its neighbours, on every rank ahead of it.
fn blocking_mask(square: usize, white: bool) -> u64 {
    let file = square % 8;
    let rank = square / 8;

    let mut files = FILE_MASKS[file];
    if file > 0 {
        files |= FILE_MASKS[file - 1];
    }
    if file < 7 {
        files |= FILE_MASKS[file + 1];
    }

    let ahead = if white {
        u64::MAX << ((rank + 1) * 8)
    } else {
        u64::MAX >> ((8 - rank) * 8)
    };

    files & ahead
}

/// The uncached value, for tests that need to prove the cache changes nothing.
pub fn uncached(cb: &Chessboard) -> i32 {
    compute(cb.white_pawns, cb.black_pawns, PawnWeights::current())
}
