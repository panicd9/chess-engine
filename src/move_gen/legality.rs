//! Legality decided before the move is made.
//!
//! The generator used to answer "is this move legal?" by making it and asking
//! `is_*_king_under_attack` on the result. That test reads back all twelve
//! bitboards `make_*` has just written, and measures at 20-24 ns per move
//! against 16-19 ns to build the board in the first place -- better than half
//! of move generation went on proving moves legal, and on a tactical position
//! a fifth of the boards built were thrown away again.
//!
//! What replaces it is three masks, computed once per position:
//!
//! * `check_mask` -- where a piece that is not the king may land. Everything
//!   when the king is not in check; the checker, plus the squares between it
//!   and the king, on a single check; nothing at all on a double check, where
//!   only the king may move.
//! * `pinned` -- own pieces that are the only thing between the king and an
//!   enemy slider. Such a piece may move only along that line, which
//!   `LINE[king][square]` spells out. A pinned knight therefore has no moves,
//!   which falls out of the mask rather than needing a case.
//! * `danger` -- every square the enemy attacks, computed with our own king
//!   lifted off the board so a slider's ray does not stop on the square the
//!   king is about to leave. A king move to a danger square is illegal and
//!   every other king move is legal, so the king needs no make-and-test
//!   either. Castling reads the same mask instead of calling
//!   `all_*_attacks()` once per side.
//!
//! En passant is the one move this does not decide. It takes two pieces off
//! one rank, so it can discover a check no mask here describes, and it can
//! answer a check by capturing the checking pawn, which `check_mask` would
//! reject. It keeps the make-and-test, which is always right and is asked at
//! most once per pawn per position.

use crate::chessboard::{Bitboard, Chessboard, SquareIndex};

use super::move_gen_bishop::{all_bishops_attacks, single_bishop_attacks};
use super::move_gen_king::king_attacks;
use super::move_gen_knight::knight_attacks_from_single_knight_bitboard;
use super::move_gen_pawn::{BLACK_PAWN_ATTACKS, WHITE_PAWN_ATTACKS};
use super::move_gen_queen::all_queens_attacks;
use super::move_gen_rook::{all_rooks_attacks, single_rook_attacks};

const DIRECTIONS: [(i32, i32); 8] =
    [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

/// Every square reachable from `from` in one direction, `from` excluded.
const fn ray(from: usize, file_step: i32, rank_step: i32) -> Bitboard {
    let mut file = (from % 8) as i32 + file_step;
    let mut rank = (from / 8) as i32 + rank_step;
    let mut squares = 0u64;
    while file >= 0 && file < 8 && rank >= 0 && rank < 8 {
        squares |= 1u64 << (rank * 8 + file) as usize;
        file += file_step;
        rank += rank_step;
    }
    squares
}

/// The squares strictly between two aligned squares; 0 when they do not share
/// a rank, file or diagonal, and 0 when they are adjacent.
const fn build_between() -> [[Bitboard; 64]; 64] {
    let mut table = [[0u64; 64]; 64];
    let mut from = 0;
    while from < 64 {
        let mut direction = 0;
        while direction < 8 {
            let (file_step, rank_step) = DIRECTIONS[direction];
            let mut file = (from % 8) as i32 + file_step;
            let mut rank = (from / 8) as i32 + rank_step;
            let mut walked = 0u64;
            while file >= 0 && file < 8 && rank >= 0 && rank < 8 {
                let to = (rank * 8 + file) as usize;
                table[from][to] = walked;
                walked |= 1u64 << to;
                file += file_step;
                rank += rank_step;
            }
            direction += 1;
        }
        from += 1;
    }
    table
}

/// The whole rank, file or diagonal through two aligned squares, both ends
/// included; 0 when they are not aligned.
const fn build_line() -> [[Bitboard; 64]; 64] {
    let mut table = [[0u64; 64]; 64];
    let mut from = 0;
    while from < 64 {
        let mut direction = 0;
        while direction < 8 {
            let (file_step, rank_step) = DIRECTIONS[direction];
            let whole = ray(from, file_step, rank_step)
                | ray(from, -file_step, -rank_step)
                | (1u64 << from);
            let mut file = (from % 8) as i32 + file_step;
            let mut rank = (from / 8) as i32 + rank_step;
            while file >= 0 && file < 8 && rank >= 0 && rank < 8 {
                table[from][(rank * 8 + file) as usize] = whole;
                file += file_step;
                rank += rank_step;
            }
            direction += 1;
        }
        from += 1;
    }
    table
}

/// 32 KB each, built at compile time. `const`, not `lazy_static` or a
/// `OnceLock`: a lazily initialised table costs a check on every read, and the
/// pawn cache lost 46% of search speed to exactly that.
pub static BETWEEN: [[Bitboard; 64]; 64] = build_between();
pub static LINE: [[Bitboard; 64]; 64] = build_line();

/// What the side to move may do without leaving its king attacked.
pub struct Legality {
    /// Where a piece other than the king may land. `!0` when not in check.
    pub check_mask: Bitboard,
    /// Own pieces that may not leave the line they share with the king.
    pub pinned: Bitboard,
    /// Squares the king may not move to, our own king not blocking.
    pub danger: Bitboard,
    pub king_square: SquareIndex,
    /// How many pieces give check. Two means only the king may move.
    pub checkers: u32,
}

impl Legality {
    /// The squares a piece standing on `square` may land on. Both terms are
    /// `!0` in the common case -- not in check, nothing pinned -- so this is
    /// two ands and a predictable branch.
    #[inline(always)]
    pub fn allowed(&self, piece: Bitboard, square: SquareIndex) -> Bitboard {
        if piece & self.pinned != 0 {
            self.check_mask & LINE[self.king_square][square]
        } else {
            self.check_mask
        }
    }

    /// Only the king may move out of a double check, so the other generators
    /// can be skipped outright rather than run with an empty mask.
    #[inline(always)]
    pub fn double_check(&self) -> bool {
        self.checkers > 1
    }

    #[inline(always)]
    pub fn in_check(&self) -> bool {
        self.checkers > 0
    }
}

pub fn white_legality(cb: &Chessboard) -> Legality {
    let king = cb.white_king;
    let king_square = king.trailing_zeros() as usize;
    let occupancy = cb.get_occupancy();
    let own = cb.get_white_occupancy();

    let diagonal = cb.black_bishops | cb.black_queens;
    let straight = cb.black_rooks | cb.black_queens;

    let checkers = (knight_attacks_from_single_knight_bitboard(king) & cb.black_knights)
        | (WHITE_PAWN_ATTACKS[king_square] & cb.black_pawns)
        | (single_bishop_attacks(occupancy, king) & diagonal)
        | (single_rook_attacks(occupancy, king) & straight);

    let pinned = pinned_pieces(king, king_square, occupancy, own, diagonal, straight);

    let without_king = occupancy ^ king;
    let danger = cb.black_pawn_attacks()
        | cb.black_knights_attacks()
        | all_bishops_attacks(without_king, cb.black_bishops)
        | all_rooks_attacks(without_king, cb.black_rooks)
        | all_queens_attacks(without_king, cb.black_queens)
        | king_attacks(cb.black_king);

    Legality {
        check_mask: check_mask(king_square, checkers),
        pinned,
        danger,
        king_square,
        checkers: checkers.count_ones(),
    }
}

pub fn black_legality(cb: &Chessboard) -> Legality {
    let king = cb.black_king;
    let king_square = king.trailing_zeros() as usize;
    let occupancy = cb.get_occupancy();
    let own = cb.get_black_occupancy();

    let diagonal = cb.white_bishops | cb.white_queens;
    let straight = cb.white_rooks | cb.white_queens;

    let checkers = (knight_attacks_from_single_knight_bitboard(king) & cb.white_knights)
        | (BLACK_PAWN_ATTACKS[king_square] & cb.white_pawns)
        | (single_bishop_attacks(occupancy, king) & diagonal)
        | (single_rook_attacks(occupancy, king) & straight);

    let pinned = pinned_pieces(king, king_square, occupancy, own, diagonal, straight);

    let without_king = occupancy ^ king;
    let danger = cb.white_pawns_attacks()
        | cb.white_knights_attacks()
        | all_bishops_attacks(without_king, cb.white_bishops)
        | all_rooks_attacks(without_king, cb.white_rooks)
        | all_queens_attacks(without_king, cb.white_queens)
        | king_attacks(cb.white_king);

    Legality {
        check_mask: check_mask(king_square, checkers),
        pinned,
        danger,
        king_square,
        checkers: checkers.count_ones(),
    }
}

#[inline]
fn check_mask(king_square: SquareIndex, checkers: Bitboard) -> Bitboard {
    match checkers.count_ones() {
        // Not in check: anywhere.
        0 => !0,
        // Take the checker, or stand in its way. `BETWEEN` is empty for a
        // knight or a pawn, which leaves capturing it as the only answer that
        // is not a king move.
        1 => checkers | BETWEEN[king_square][checkers.trailing_zeros() as usize],
        // Double check: no piece but the king can answer two attackers at once.
        _ => 0,
    }
}

/// Own pieces that stand alone between the king and an enemy slider.
///
/// Looking out from the king over an occupancy that holds the enemy and our
/// own king only, each ray passes through our pieces and stops on the nearest
/// enemy piece. When that piece is a slider of the right kind, whatever of
/// ours lies between is pinned -- provided there is exactly one such piece,
/// since two blockers pin neither.
#[inline]
fn pinned_pieces(
    king: Bitboard,
    king_square: SquareIndex,
    occupancy: Bitboard,
    own: Bitboard,
    diagonal: Bitboard,
    straight: Bitboard,
) -> Bitboard {
    let see_through_own = (occupancy & !own) | king;
    let mut snipers = (single_bishop_attacks(see_through_own, king) & diagonal)
        | (single_rook_attacks(see_through_own, king) & straight);

    let mut pinned = 0;
    while snipers != 0 {
        let sniper = snipers & snipers.wrapping_neg();
        let blockers = BETWEEN[king_square][sniper.trailing_zeros() as usize] & occupancy;
        if blockers.count_ones() == 1 && blockers & own != 0 {
            pinned |= blockers;
        }
        snipers &= snipers - 1;
    }
    pinned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::squares::{Square, SquareBitboard};

    #[test]
    fn between_is_exclusive_and_only_on_a_line() {
        // a1..h1 along a rank.
        assert_eq!(BETWEEN[0][3], (1 << 1) | (1 << 2));
        // Adjacent squares have nothing between them.
        assert_eq!(BETWEEN[0][1], 0);
        // Squares that do not share a line have no entry.
        assert_eq!(BETWEEN[0][10], 0);
        // A diagonal: a1 to d4 leaves b2 and c3.
        assert_eq!(BETWEEN[0][27], (1 << 9) | (1 << 18));
        // Symmetric.
        for a in 0..64 {
            for b in 0..64 {
                assert_eq!(BETWEEN[a][b], BETWEEN[b][a], "between {a} {b}");
            }
        }
    }

    #[test]
    fn line_is_the_whole_line_through_both_squares() {
        // The first rank, all eight squares.
        assert_eq!(LINE[0][3], 0xff);
        // The a1-h8 diagonal.
        assert_eq!(LINE[0][27], 0x8040201008040201);
        // Unaligned squares have no line.
        assert_eq!(LINE[0][10], 0);
        // A square is on the line of anything aligned with it.
        for a in 0..64 {
            for b in 0..64 {
                assert_eq!(LINE[a][b], LINE[b][a], "line {a} {b}");
                if LINE[a][b] != 0 {
                    assert!(LINE[a][b] & (1 << a) != 0);
                    assert!(LINE[a][b] & (1 << b) != 0);
                    assert_eq!(BETWEEN[a][b] & LINE[a][b], BETWEEN[a][b]);
                }
            }
        }
    }

    #[test]
    fn finds_the_pinned_piece() {
        // White knight on e2 pinned to e1 by a rook on e8.
        let cb = Chessboard::from_fen("4r2k/8/8/8/8/8/4N3/4K3 w - - 0 1").unwrap();
        let legality = white_legality(&cb);
        assert_eq!(legality.pinned, SquareBitboard::E2 as u64);
        assert!(!legality.in_check());
        // It may only move along the e-file, and a knight never can.
        let allowed = legality.allowed(SquareBitboard::E2 as u64, Square::E2 as usize);
        assert_eq!(allowed, LINE[Square::E1 as usize][Square::E2 as usize]);
        assert_eq!(allowed & knight_attacks_from_single_knight_bitboard(SquareBitboard::E2 as u64), 0);
    }

    #[test]
    fn two_blockers_pin_neither() {
        let cb = Chessboard::from_fen("4r2k/8/8/8/8/4N3/4B3/4K3 w - - 0 1").unwrap();
        assert_eq!(white_legality(&cb).pinned, 0);
    }

    #[test]
    fn check_mask_is_the_checker_and_the_squares_before_it() {
        // Rook on e8 checking a king on e1 down an empty file.
        let cb = Chessboard::from_fen("4r2k/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let legality = white_legality(&cb);
        assert_eq!(legality.checkers, 1);
        let e_file_between: u64 = (1..7).map(|rank| 1u64 << (rank * 8 + 4)).sum();
        assert_eq!(legality.check_mask, e_file_between | (SquareBitboard::E8 as u64));
    }

    #[test]
    fn a_knight_check_can_only_be_captured() {
        let cb = Chessboard::from_fen("7k/8/8/8/8/5n2/8/4K3 w - - 0 1").unwrap();
        let legality = white_legality(&cb);
        assert_eq!(legality.checkers, 1);
        assert_eq!(legality.check_mask, SquareBitboard::F3 as u64);
    }

    #[test]
    fn double_check_leaves_nothing_but_king_moves() {
        let cb = Chessboard::from_fen("4r2k/8/8/8/8/5n2/8/4K3 w - - 0 1").unwrap();
        let legality = white_legality(&cb);
        assert_eq!(legality.checkers, 2);
        assert!(legality.double_check());
        assert_eq!(legality.check_mask, 0);
    }

    #[test]
    fn danger_reaches_past_the_king_it_is_checking() {
        // A king on e4, checked down the file by a rook on e8. Stepping to e3
        // does not escape, and a plain attack map says it does: the rook's ray
        // stops on the square the king is about to leave. This is what
        // make-and-test got right, and the reason `danger` lifts the king off
        // the board before asking.
        let cb = Chessboard::from_fen("4r2k/8/8/8/4K3/8/8/8 w - - 0 1").unwrap();
        let legality = white_legality(&cb);
        assert!(legality.danger & (SquareBitboard::E3 as u64) != 0, "e3 is still on the rook's line");
        assert!(legality.danger & (SquareBitboard::E5 as u64) != 0);
        assert!(legality.danger & (SquareBitboard::E4 as u64) != 0);
        assert!(legality.danger & (SquareBitboard::D4 as u64) == 0);
        assert!(legality.danger & (SquareBitboard::F4 as u64) == 0);

        // And the generator agrees: no king move stays on the e-file.
        let mut moves = Vec::new();
        super::super::white_legal_moves_into(&cb, &mut moves);
        assert!(!moves.is_empty());
        for m in &moves {
            assert_eq!(m.chessboard.white_king & LINE[Square::E4 as usize][Square::E8 as usize], 0);
        }
    }
}
