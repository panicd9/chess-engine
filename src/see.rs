//! Static exchange evaluation: what a capture is worth once both sides have
//! finished taking on the square.
//!
//! Quiescence searches every capture, including ones that plainly lose
//! material -- a queen taking a defended pawn, say. Playing out the whole
//! exchange statically is far cheaper than searching it, so those can be
//! recognised and skipped.
//!
//! The method is the standard swap-off: take with the least valuable attacker
//! each time, alternating sides, then work backwards deciding at each step
//! whether the side to move would actually have continued.

use crate::chessboard::{Bitboard, Chessboard, Color, SquareIndex};
use crate::move_gen::move_gen_bishop::single_bishop_attacks;
use crate::move_gen::move_gen_king::king_attacks;
use crate::move_gen::move_gen_knight::knight_attacks_from_single_knight_bitboard;
use crate::move_gen::move_gen_pawn::{single_black_pawn_attacks, single_white_pawn_attacks};
use crate::move_gen::move_gen_rook::single_rook_attacks;
use crate::piece::ColoredPiece;

/// Values used only for weighing exchanges, not for evaluation. The king is
/// effectively infinite: it can capture, but never be captured, so a sequence
/// that would expose it simply stops.
const PIECE_VALUE: [i32; 13] = [
    100, 100, // pawns
    320, 320, // knights
    330, 330, // bishops
    500, 500, // rooks
    900, 900, // queens
    10_000, 10_000, // kings
    0,   // empty
];

fn value_of(piece: ColoredPiece) -> i32 {
    PIECE_VALUE[piece as usize]
}

/// Every piece of either colour currently attacking `square`, given `occupancy`.
///
/// Recomputed as the occupancy shrinks, which is what uncovers pieces lined up
/// behind the ones that have already captured.
fn attackers_to(cb: &Chessboard, square: SquareIndex, occupancy: Bitboard) -> Bitboard {
    let target = 1u64 << square;

    // A pawn attacks this square if, from here, a pawn of the opposite colour
    // would attack it -- the relation is symmetric.
    let by_white_pawns = single_black_pawn_attacks(target) & cb.white_pawns;
    let by_black_pawns = single_white_pawn_attacks(target) & cb.black_pawns;

    let knights = knight_attacks_from_single_knight_bitboard(target)
        & (cb.white_knights | cb.black_knights);
    let kings = king_attacks(target) & (cb.white_king | cb.black_king);

    let diagonal = single_bishop_attacks(occupancy, target)
        & (cb.white_bishops | cb.black_bishops | cb.white_queens | cb.black_queens);
    let straight = single_rook_attacks(occupancy, target)
        & (cb.white_rooks | cb.black_rooks | cb.white_queens | cb.black_queens);

    (by_white_pawns | by_black_pawns | knights | kings | diagonal | straight) & occupancy
}

/// The cheapest piece of `side` among `attackers`, as a single-bit board.
fn least_valuable(cb: &Chessboard, attackers: Bitboard, side: Color) -> Option<(Bitboard, i32)> {
    let sets: [(Bitboard, i32); 6] = match side {
        Color::White => [
            (cb.white_pawns, 100),
            (cb.white_knights, 320),
            (cb.white_bishops, 330),
            (cb.white_rooks, 500),
            (cb.white_queens, 900),
            (cb.white_king, 10_000),
        ],
        Color::Black => [
            (cb.black_pawns, 100),
            (cb.black_knights, 320),
            (cb.black_bishops, 330),
            (cb.black_rooks, 500),
            (cb.black_queens, 900),
            (cb.black_king, 10_000),
        ],
    };
    for (set, value) in sets {
        let candidates = set & attackers;
        if candidates != 0 {
            return Some((candidates & candidates.wrapping_neg(), value));
        }
    }
    None
}

/// Material the side to move gains by capturing on `to` with the piece on
/// `from`, assuming both sides then take optimally.
///
/// Negative means the capture loses material.
pub fn see(cb: &Chessboard, from: SquareIndex, to: SquareIndex) -> i32 {
    let mut gain = [0i32; 32];
    let mut depth = 0;

    let mut occupancy = cb.get_occupancy();
    let mut attacker = 1u64 << from;
    let mut attacker_value = value_of(cb.piece_square[from]);
    let mut side = match cb.side_to_move {
        Color::White => Color::Black, // The opponent replies first.
        Color::Black => Color::White,
    };

    gain[0] = value_of(cb.piece_square[to]);

    loop {
        depth += 1;
        if depth >= gain.len() {
            break;
        }
        // What this side stands to gain if it takes and the exchange stops here.
        gain[depth] = attacker_value - gain[depth - 1];

        occupancy &= !attacker;
        let attackers = attackers_to(cb, to, occupancy);

        let Some((next, next_value)) = least_valuable(cb, attackers, side) else {
            break;
        };
        attacker = next;
        attacker_value = next_value;
        side = match side {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
    }

    // Work backwards: at each step the side to move only continues if doing so
    // beats standing pat.
    while depth > 1 {
        depth -= 1;
        gain[depth - 1] = -std::cmp::max(-gain[depth - 1], gain[depth]);
    }

    gain[0]
}
