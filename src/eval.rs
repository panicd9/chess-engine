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

/// Centipawns per square of mobility, per piece type. Sliders benefit most from
/// open lines, so they are weighted higher than knights.
const KNIGHT_MOBILITY: i32 = 4;
const BISHOP_MOBILITY: i32 = 4;
const ROOK_MOBILITY: i32 = 2;
const QUEEN_MOBILITY: i32 = 1;

/// Penalty for each missing pawn of the three in front of a castled king.
const MISSING_SHIELD_PAWN: i32 = 12;

/// Bonus for a passed pawn, by the rank it has reached (from its own side's
/// point of view). A pawn one step from promoting is worth close to a piece.
const PASSED_PAWN_BY_RANK: [i32; 8] = [0, 5, 10, 20, 40, 70, 120, 0];

pub fn evaluate(cb: &Chessboard) -> i32 {
    eval(&cb.piece_square, &EVAL_TABLES) + mobility(cb) + king_safety(cb) + passed_pawns(cb)
}

/// Squares attacked by each side's pieces, weighted by piece type.
///
/// Counts attacked squares rather than legal moves, which is cheaper and close
/// enough: a piece that eyes many squares is usually the more active one.
fn mobility(cb: &Chessboard) -> i32 {
    let white = cb.white_knights_attacks().count_ones() as i32 * KNIGHT_MOBILITY
        + cb.white_bishops_attacks().count_ones() as i32 * BISHOP_MOBILITY
        + cb.white_rooks_attacks().count_ones() as i32 * ROOK_MOBILITY
        + cb.white_queens_attacks().count_ones() as i32 * QUEEN_MOBILITY;
    let black = cb.black_knights_attacks().count_ones() as i32 * KNIGHT_MOBILITY
        + cb.black_bishops_attacks().count_ones() as i32 * BISHOP_MOBILITY
        + cb.black_rooks_attacks().count_ones() as i32 * ROOK_MOBILITY
        + cb.black_queens_attacks().count_ones() as i32 * QUEEN_MOBILITY;
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
        -missing * MISSING_SHIELD_PAWN
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

    let mut pawns = cb.white_pawns;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let rank = square / 8;
        if rank < 7 && (cb.black_pawns & blocking_mask(square, true)) == 0 {
            score += PASSED_PAWN_BY_RANK[rank];
        }
    }

    let mut pawns = cb.black_pawns;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let rank = square / 8;
        if rank > 0 && (cb.white_pawns & blocking_mask(square, false)) == 0 {
            score -= PASSED_PAWN_BY_RANK[7 - rank];
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
