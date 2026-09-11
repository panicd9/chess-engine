//! Position evaluation, from white's point of view.
//!
//! The bulk of the score is the tapered piece-square evaluation in
//! [`crate::piece_square_tables`], which already blends material and placement
//! between opening and endgame. Added on top are three things those tables
//! cannot express, because they depend on the position as a whole rather than
//! on where one piece stands:
//!
//! - **mobility** -- how many squares a side's pieces actually control
//! - **king safety** -- whether the pawns in front of the king are still there
//! - **passed pawns** -- pawns with nothing left to stop them queening
//!
//! Every weight here is a hand-picked guess in centipawns, not a tuned value.

use crate::chessboard::{Chessboard, FILE_MASKS};
use crate::piece_square_tables::{eval, EVAL_TABLES};

/// The weights these terms use, all in centipawns.
///
/// Overridable at runtime so they can be tuned by playing matches rather than
/// by rebuilding for each candidate: see the `EvalWeight` UCI options. The
/// defaults are hand-picked starting points, not tuned values.
pub mod weights {
    use std::sync::atomic::{AtomicI32, Ordering};

    /// Per square of mobility, by piece. The weight falls as the piece gets
    /// bigger, which is the opposite of what it looks like it should be: a queen
    /// already attacks many squares from anywhere, so counting each of them at
    /// the knight's rate would swamp the rest of the evaluation and reward
    /// shuffling her into the open. The term as a whole is worth +58.7 +/- 26.1
    /// Elo over 400 games against setting all four to zero; these particular
    /// numbers are still hand-picked and have never been tuned against
    /// alternatives.
    pub static KNIGHT_MOBILITY: AtomicI32 = AtomicI32::new(4);
    pub static BISHOP_MOBILITY: AtomicI32 = AtomicI32::new(4);
    pub static ROOK_MOBILITY: AtomicI32 = AtomicI32::new(2);
    pub static QUEEN_MOBILITY: AtomicI32 = AtomicI32::new(1);

    /// Per missing pawn of the three in front of a castled king.
    pub static MISSING_SHIELD_PAWN: AtomicI32 = AtomicI32::new(12);

    /// Scales the passed pawn bonus, as a percentage. 100 leaves the by-rank
    /// table below unchanged.
    pub static PASSED_PAWN_SCALE: AtomicI32 = AtomicI32::new(100);

    /// Set a weight by name. Unknown names are ignored, as UCI requires.
    /// Returns whether the name was recognised.
    pub fn set(name: &str, value: i32) -> bool {
        // Case-insensitive: GUIs are inconsistent about how they echo names.
        let target = match name.to_ascii_lowercase().as_str() {
            "knightmobility" => &KNIGHT_MOBILITY,
            "bishopmobility" => &BISHOP_MOBILITY,
            "rookmobility" => &ROOK_MOBILITY,
            "queenmobility" => &QUEEN_MOBILITY,
            "kingshield" => &MISSING_SHIELD_PAWN,
            "passedpawnscale" => &PASSED_PAWN_SCALE,
            _ => return false,
        };
        target.store(value, Ordering::Relaxed);
        true
    }

    #[inline]
    pub fn get(w: &AtomicI32) -> i32 {
        w.load(Ordering::Relaxed)
    }
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

pub fn evaluate(cb: &Chessboard) -> i32 {
    eval(&cb.piece_square, &EVAL_TABLES) + mobility(cb) + king_safety(cb) + passed_pawns(cb)
}

/// Squares attacked by each side's pieces, weighted by piece type.
///
/// Counts attacked squares rather than legal moves, which is cheaper and close
/// enough: a piece that eyes many squares is usually the more active one.
fn mobility(cb: &Chessboard) -> i32 {
    let (knight, bishop, rook, queen) = (
        weights::get(&weights::KNIGHT_MOBILITY),
        weights::get(&weights::BISHOP_MOBILITY),
        weights::get(&weights::ROOK_MOBILITY),
        weights::get(&weights::QUEEN_MOBILITY),
    );
    let white = cb.white_knights_attacks().count_ones() as i32 * knight
        + cb.white_bishops_attacks().count_ones() as i32 * bishop
        + cb.white_rooks_attacks().count_ones() as i32 * rook
        + cb.white_queens_attacks().count_ones() as i32 * queen;
    let black = cb.black_knights_attacks().count_ones() as i32 * knight
        + cb.black_bishops_attacks().count_ones() as i32 * bishop
        + cb.black_rooks_attacks().count_ones() as i32 * rook
        + cb.black_queens_attacks().count_ones() as i32 * queen;
    white - black
}

/// Penalise a king whose pawn cover has gone.
///
/// Only the king's own file and its neighbours are considered, and only while
/// there is still enough material for an attack -- in an endgame the king
/// wants to be active, not hidden.
fn king_safety(cb: &Chessboard) -> i32 {
    let heavy_material = (cb.white_queens | cb.black_queens | cb.white_rooks | cb.black_rooks)
        .count_ones();
    if heavy_material < 2 {
        return 0; // Endgame: the shield no longer matters.
    }

    let shield = |king: u64, pawns: u64, ahead: fn(u64) -> u64| -> i32 {
        if king == 0 {
            return 0;
        }
        let file = (king.trailing_zeros() % 8) as usize;
        let mut files = FILE_MASKS[file];
        if file > 0 {
            files |= FILE_MASKS[file - 1];
        }
        if file < 7 {
            files |= FILE_MASKS[file + 1];
        }
        // Pawns standing on the three files, ahead of the king.
        let cover = pawns & files & ahead(king);
        let missing = 3i32 - cover.count_ones().min(3) as i32;
        -missing * weights::get(&weights::MISSING_SHIELD_PAWN)
    };

    // "Ahead" is up the board for white, down for black.
    let white = shield(cb.white_king, cb.white_pawns, |k| {
        let rank = k.trailing_zeros() / 8;
        if rank >= 7 { 0 } else { u64::MAX << ((rank + 1) * 8) }
    });
    let black = shield(cb.black_king, cb.black_pawns, |k| {
        let rank = k.trailing_zeros() / 8;
        if rank == 0 { 0 } else { u64::MAX >> ((8 - rank) * 8) }
    });
    white - black
}

/// A pawn is passed when no enemy pawn stands on its file or either adjacent
/// file anywhere ahead of it, so nothing can block or capture it on the way.
fn passed_pawns(cb: &Chessboard) -> i32 {
    let mut score = 0;
    let scale = weights::get(&weights::PASSED_PAWN_SCALE);

    let mut pawns = cb.white_pawns;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let rank = square / 8;
        if rank < 7 && (cb.black_pawns & blocking_mask(square, true)) == 0 {
            score += PASSED_PAWN_BY_RANK[rank] * scale / 100;
        }
    }

    let mut pawns = cb.black_pawns;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let rank = square / 8;
        if rank > 0 && (cb.white_pawns & blocking_mask(square, false)) == 0 {
            score -= PASSED_PAWN_BY_RANK[7 - rank] * scale / 100;
        }
    }

    score
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
