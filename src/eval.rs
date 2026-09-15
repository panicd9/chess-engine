//! Position evaluation, from white's point of view.
//!
//! The bulk of the score is the tapered piece-square evaluation in
//! [`crate::piece_square_tables`], which already blends material and placement
//! between opening and endgame. Added on top are the things those tables
//! cannot express, because they depend on the position as a whole rather than
//! on where one piece stands:
//!
//! - **mobility** -- how many squares a side's pieces actually control
//! - **king safety** -- whether the pawns in front of the king are still there
//! - **passed pawns** -- pawns with nothing left to stop them queening
//! - **what the pieces let a passed pawn do** -- the kings' distance to it, and
//!   whether its path is free
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

    /// How far along a passed pawn's path the pieces let it go. See
    /// [`super::passed_pawn_pieces`]; these scale Stockfish 15's classical
    /// terms, whose shapes are kept and whose sizes are refitted.
    ///
    /// `PASSED_KING_THEM` and `PASSED_KING_US` are in quarter centipawns per
    /// square of king distance to the stop square, per unit of the rank weight.
    /// `PASSED_FREE_PATH` is a percentage of Stockfish's free-path bonus, and
    /// `PASSED_PATH_OFFSET` is subtracted from that bonus's 0..41 grade first,
    /// so that a pawn whose path is taken can score below the tables' average
    /// rather than only a free one above it.
    ///
    /// Fitted by `examples/texel` with `only=` these four plus
    /// `PassedPawnScale`, from Stockfish's shapes scaled to our pawn (9,4,45,17),
    /// and cross-validated each way:
    ///
    /// ```text
    /// trained on       Scale  Them  Us  Free  Offset
    /// big3               120     7   2    25      16
    /// quiet-labeled       60     8   1    21      19
    /// ```
    ///
    /// The four agree; `PassedPawnScale` splits either side of its adopted 84 as
    /// it did in the six-weight fit, so it stays. Against the term switched off,
    /// these values cut the error by 0.85% on big3 and 2.5% on quiet-labeled,
    /// and by the same amounts on each as the held-out set.
    pub static PASSED_KING_THEM: AtomicI32 = AtomicI32::new(8);
    pub static PASSED_KING_US: AtomicI32 = AtomicI32::new(2);
    pub static PASSED_FREE_PATH: AtomicI32 = AtomicI32::new(23);
    pub static PASSED_PATH_OFFSET: AtomicI32 = AtomicI32::new(17);

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
            "passedkingthem" => &PASSED_KING_THEM,
            "passedkingus" => &PASSED_KING_US,
            "passedfreepath" => &PASSED_FREE_PATH,
            "passedpathoffset" => &PASSED_PATH_OFFSET,
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
    let attacks = PieceAttacks::new(cb);
    eval(&cb.piece_square, &EVAL_TABLES)
        + mobility(&attacks)
        + king_safety(cb)
        + crate::pawn_hash::passed_pawns(cb)
        + passed_pawn_pieces(cb, &attacks)
}

/// The squares each side's knights, bishops, rooks and queens attack, computed
/// once per evaluation: mobility counts them, and the passed-pawn term needs
/// their union. Computing them twice cost up to 15% of search time in endgames.
struct PieceAttacks {
    white: [u64; 4],
    black: [u64; 4],
}

impl PieceAttacks {
    #[inline]
    fn new(cb: &Chessboard) -> Self {
        PieceAttacks {
            white: [
                cb.white_knights_attacks(),
                cb.white_bishops_attacks(),
                cb.white_rooks_attacks(),
                cb.white_queens_attacks(),
            ],
            black: [
                cb.black_knights_attacks(),
                cb.black_bishops_attacks(),
                cb.black_rooks_attacks(),
                cb.black_queens_attacks(),
            ],
        }
    }
}

/// Every square from `b` southwards, `b` included.
#[inline]
fn fill_south(mut b: u64) -> u64 {
    b |= b >> 8;
    b |= b >> 16;
    b |= b >> 32;
    b
}

/// Every square from `b` northwards, `b` included.
#[inline]
fn fill_north(mut b: u64) -> u64 {
    b |= b << 8;
    b |= b << 16;
    b |= b << 32;
    b
}

#[inline]
fn with_neighbour_files(b: u64) -> u64 {
    b | ((b << 1) & !FILE_MASKS[0]) | ((b >> 1) & !FILE_MASKS[7])
}

#[inline]
fn distance(a: usize, b: usize) -> i32 {
    let files = (a % 8).abs_diff(b % 8);
    let ranks = (a / 8).abs_diff(b / 8);
    files.max(ranks) as i32
}

/// The part of a passed pawn's value that depends on the pieces rather than the
/// pawns, which is why it cannot live in [`crate::pawn_hash`].
///
/// The piece-square tables pay a pawn on the seventh rank ~150cp in the endgame
/// and the pawn hash adds a rank bonus, whether or not anything stops it. That
/// is an average over pawns that queen and pawns that are lost, and it threw
/// away a won game: in `gCd8UcfI` the engine sacrificed into an endgame it
/// scored +394 because of two pawns on the seventh, with an enemy knight
/// covering both queening squares and the enemy king next to one of them.
/// Stockfish scores it 0.00. `2qYroOWA` is the same blindness from the other
/// side: a blockaded enemy pawn on d7 was worth 280cp to us, so the winning
/// blockade looked equal and the engine took a perpetual.
///
/// The shapes are Stockfish 15's `Evaluation::passed()`, for pawns on the fourth
/// rank and beyond, weighted by `w = 5 * relative_rank - 13`:
///
/// - **King proximity** (endgame only): the enemy king's distance to the stop
///   square is a bonus, our own king's distance to it and to the square beyond
///   (2:1) a penalty, each capped at 5.
/// - **Free path** (both phases): when the stop square is empty, a bonus by how
///   much of the pawn's path the enemy controls -- none of the span (36), only
///   squares our pawns defend (30), none of the pawn's own file (17), not the
///   stop square (7), or the stop square itself (0) -- plus 5 when we defend the
///   stop square or have a rook or queen behind the pawn. An enemy rook or queen
///   behind the pawn makes the whole span count as controlled.
///
/// Stockfish's constants are in its own pawn units (208cp in the endgame against
/// our 94), so only the shapes carry over; the weights are fitted.
fn passed_pawn_pieces(cb: &Chessboard, attacks: &PieceAttacks) -> i32 {
    // The fourth rank and beyond, from each side's own point of view.
    const WHITE_ADVANCED: u64 = 0x00FF_FFFF_FF00_0000;
    const BLACK_ADVANCED: u64 = 0x0000_00FF_FFFF_FF00;

    // Passed: no enemy pawn ahead on its own or a neighbouring file. The same
    // test as the pawn hash's `blocking_mask`, done with fills.
    let white_passed = cb.white_pawns
        & WHITE_ADVANCED
        & !with_neighbour_files(fill_south(cb.black_pawns >> 8));
    let black_passed = cb.black_pawns
        & BLACK_ADVANCED
        & !with_neighbour_files(fill_north(cb.white_pawns << 8));
    if white_passed | black_passed == 0 {
        return 0;
    }

    let (king_them, king_us, free_path, offset) = (
        weights::get(&weights::PASSED_KING_THEM),
        weights::get(&weights::PASSED_KING_US),
        weights::get(&weights::PASSED_FREE_PATH),
        weights::get(&weights::PASSED_PATH_OFFSET),
    );

    let white_pieces = cb.white_pawns | cb.white_knights | cb.white_bishops
        | cb.white_rooks | cb.white_queens | cb.white_king;
    let black_pieces = cb.black_pawns | cb.black_knights | cb.black_bishops
        | cb.black_rooks | cb.black_queens | cb.black_king;
    let occupied = white_pieces | black_pieces;
    let heavy = cb.white_rooks | cb.black_rooks | cb.white_queens | cb.black_queens;

    let white = Side {
        king: cb.white_king.trailing_zeros() as usize,
        pieces: white_pieces,
        pawn_attacks: cb.white_pawns_attacks(),
        attacks: attacks.white.iter().fold(cb.white_pawns_attacks() | cb.white_king_attacks(), |a, b| a | b),
    };
    let black = Side {
        king: cb.black_king.trailing_zeros() as usize,
        pieces: black_pieces,
        pawn_attacks: cb.black_pawn_attacks(),
        attacks: attacks.black.iter().fold(cb.black_pawn_attacks() | cb.black_king_attacks(), |a, b| a | b),
    };

    // Accumulated white minus black before any division, so that a mirrored
    // position scores exactly the negation.
    let (mut endgame_quarters, mut both) = (0i32, 0i32);
    let mut pawns = white_passed;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let (k, f) = passer(square, true, &white, &black, occupied, heavy, king_them, king_us, offset);
        endgame_quarters += k;
        both += f;
    }
    let mut pawns = black_passed;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let (k, f) = passer(square, false, &black, &white, occupied, heavy, king_them, king_us, offset);
        endgame_quarters -= k;
        both -= f;
    }

    let phase = ((cb.white_knights | cb.black_knights | cb.white_bishops | cb.black_bishops)
        .count_ones()
        + 2 * (cb.white_rooks | cb.black_rooks).count_ones()
        + 4 * (cb.white_queens | cb.black_queens).count_ones())
    .min(24) as i32;

    endgame_quarters * (24 - phase) / 96 + both * free_path / 100
}

struct Side {
    king: usize,
    pieces: u64,
    pawn_attacks: u64,
    attacks: u64,
}

/// One passed pawn on its fourth rank or beyond: (king proximity in quarter
/// centipawns, endgame only; free-path grade less the offset, times the rank
/// weight, both phases).
#[inline]
#[allow(clippy::too_many_arguments)]
fn passer(
    square: usize,
    white: bool,
    us: &Side,
    them: &Side,
    occupied: u64,
    heavy: u64,
    king_them: i32,
    king_us: i32,
    offset: i32,
) -> (i32, i32) {
    let rank = if white { square / 8 } else { 7 - square / 8 } as i32;
    let w = 5 * rank - 13;
    let stop = if white { square + 8 } else { square - 8 };
    let stop_bb = 1u64 << stop;

    let mut proximity = king_them * distance(them.king, stop).min(5) - king_us * distance(us.king, stop).min(5);
    if rank != 6 {
        // Short of the seventh, the square after the stop square matters too.
        let beyond = if white { stop + 8 } else { stop - 8 };
        proximity -= king_us * distance(us.king, beyond).min(5) / 2;
    }

    // A pawn whose stop square is occupied grades 0, the same as one whose stop
    // square the enemy controls.
    let mut grade = 0;
    if occupied & stop_bb == 0 {
        let pawn = 1u64 << square;
        let (to_queen, behind) = if white {
            (fill_north(pawn << 8), fill_south(pawn >> 8))
        } else {
            (fill_south(pawn >> 8), fill_north(pawn << 8))
        };
        let heavy_behind = behind & heavy;
        let mut unsafe_squares = with_neighbour_files(to_queen);
        if heavy_behind & them.pieces == 0 {
            unsafe_squares &= them.attacks | them.pieces;
        }
        let mut k = if unsafe_squares == 0 {
            36
        } else if unsafe_squares & !us.pawn_attacks == 0 {
            30
        } else if unsafe_squares & to_queen == 0 {
            17
        } else if unsafe_squares & stop_bb == 0 {
            7
        } else {
            0
        };
        if heavy_behind & us.pieces != 0 || us.attacks & stop_bb != 0 {
            k += 5;
        }
        grade = k;
    }
    (proximity * w, (grade - offset) * w)
}

/// Squares attacked by each side's pieces, weighted by piece type.
///
/// Counts attacked squares rather than legal moves, which is cheaper and close
/// enough: a piece that eyes many squares is usually the more active one.
fn mobility(attacks: &PieceAttacks) -> i32 {
    let weight = [
        weights::get(&weights::KNIGHT_MOBILITY),
        weights::get(&weights::BISHOP_MOBILITY),
        weights::get(&weights::ROOK_MOBILITY),
        weights::get(&weights::QUEEN_MOBILITY),
    ];
    let side = |a: &[u64; 4]| -> i32 {
        a.iter().zip(weight).map(|(squares, w)| squares.count_ones() as i32 * w).sum()
    };
    side(&attacks.white) - side(&attacks.black)
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

