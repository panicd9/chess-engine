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
/// by rebuilding for each candidate: see the `EvalWeight` UCI options.
///
/// **These are fitted values, not guesses.** They come from a Texel fit against
/// two independently sourced public corpora -- each held out against the other,
/// since neither has game ids -- and were then measured over the board:
/// **+17.1 +/- 7.6 Elo, LOS 100%, over 5000 games at 10+0.1** against the
/// hand-picked set they replaced. Do not adjust one by eye; the six were fitted
/// and measured together.
pub mod weights {
    use std::sync::atomic::{AtomicI32, Ordering};

    /// Per square of mobility, by piece. The term as a whole is worth
    /// +58.7 +/- 26.1 Elo over 400 games against setting all four to zero.
    ///
    /// These were 4/4/2/1, on the reasoning that the weight should fall as the
    /// piece gets bigger because a queen already attacks many squares from
    /// anywhere, so counting each at the knight's rate would swamp the rest of
    /// the evaluation and reward shuffling her into the open. **The fit
    /// disagrees and the board agreed with the fit**: it is close to flat, and
    /// the rook -- which gains the most from a line opening -- is now the
    /// highest. The reasoning was sound and the conclusion was wrong.
    pub static KNIGHT_MOBILITY: AtomicI32 = AtomicI32::new(3);
    pub static BISHOP_MOBILITY: AtomicI32 = AtomicI32::new(3);
    pub static ROOK_MOBILITY: AtomicI32 = AtomicI32::new(4);
    pub static QUEEN_MOBILITY: AtomicI32 = AtomicI32::new(3);

    /// Per missing pawn of the three in front of a castled king.
    ///
    /// Was 12. Both corpora pushed it down hard and independently, which is the
    /// most surprising part of the fit: counting missing pawns is a crude proxy
    /// for king safety, and overpaying for it makes the engine hold pawns in
    /// front of its king that are worth more elsewhere.
    pub static MISSING_SHIELD_PAWN: AtomicI32 = AtomicI32::new(5);

    /// Penalty per pawn beyond the first on a file, and per isolated pawn on a
    /// file with no enemy pawn.
    ///
    /// **Measured, at two time controls, against the same engine with both at 0:**
    ///
    /// ```text
    /// 10+0.1   1812 games   depth ~10    +31.0 +/- 12.6
    /// 40+0.4    700 games   depth ~11.8  +36.4 +/- 19.1
    /// ```
    ///
    /// Fitted by `examples/texel` against game results, cross-validated each way
    /// over big3 and quiet-labeled (19/14 and 16/10; these are the consensus).
    /// The short-control run was stopped early on a favourable reading, so its
    /// magnitude is biased upward; the sign is not in doubt at either control.
    ///
    /// They were tried once before at 25 and 19, read off a regression against
    /// Stockfish's evaluation, and measured +3.8 +/- 17.4 at identical conditions
    /// -- a difference of +27 +/- 22 from this. Two things changed: the weights
    /// now come from game results, and the terms live behind `crate::pawn_hash`,
    /// which computes them only on a miss instead of costing 10-12% of
    /// `evaluate()`. Do not adjust one by eye.
    ///
    /// They are the two features that survive a change of corpus: top-ranked
    /// against both zurichess `quiet-labeled` and our own game positions, at both
    /// oracle depths. Nothing else in that study does.
    pub static DOUBLED_PAWN: AtomicI32 = AtomicI32::new(18);
    pub static ISOLATED_HALF_OPEN_PAWN: AtomicI32 = AtomicI32::new(12);

    /// Scales the passed pawn bonus, as a percentage. 100 leaves the by-rank
    /// table below unchanged.
    ///
    /// Was 100. The cut is small and it agrees with an independent measurement:
    /// regressing `evaluate()` against Stockfish 19 over 66k quiet positions
    /// found the passed-pawn term over-generous by ~16cp per pawn, on top of
    /// what the piece-square tables already pay an advanced pawn.
    pub static PASSED_PAWN_SCALE: AtomicI32 = AtomicI32::new(84);

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
            "doubledpawn" => &DOUBLED_PAWN,
            "isolatedhalfopenpawn" => &ISOLATED_HALF_OPEN_PAWN,
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

pub fn evaluate(cb: &Chessboard) -> i32 {
    eval(&cb.piece_square, &EVAL_TABLES) + mobility(cb) + king_safety(cb) + crate::pawn_hash::passed_pawns(cb)
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

