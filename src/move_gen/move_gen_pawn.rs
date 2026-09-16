use crate::{chessboard::{
    file_masks::{FILE_A, FILE_H},
    rank_masks::{RANK_1, RANK_2, RANK_7, RANK_8},
    Bitboard, Chessboard, SingletonBitboard,
}, move_list::Move};

use super::legality::Legality;

pub const PAWNS_MOVES_CAPACITY: usize = 20;

#[rustfmt::skip]
pub static WHITE_PAWN_ATTACKS: [Bitboard; 64] = [
    0x200, 0x500, 0xa00, 0x1400, 0x2800, 0x5000, 0xa000, 0x4000,
    0x20000, 0x50000, 0xa0000, 0x140000, 0x280000, 0x500000, 0xa00000, 0x400000,
    0x2000000, 0x5000000, 0xa000000, 0x14000000, 0x28000000, 0x50000000, 0xa0000000, 0x40000000,
    0x200000000, 0x500000000, 0xa00000000, 0x1400000000, 0x2800000000, 0x5000000000, 0xa000000000, 0x4000000000,
    0x20000000000, 0x50000000000, 0xa0000000000, 0x140000000000, 0x280000000000, 0x500000000000, 0xa00000000000, 0x400000000000,
    0x2000000000000, 0x5000000000000, 0xa000000000000, 0x14000000000000, 0x28000000000000, 0x50000000000000, 0xa0000000000000, 0x40000000000000,
    0x200000000000000, 0x500000000000000, 0xa00000000000000, 0x1400000000000000, 0x2800000000000000, 0x5000000000000000, 0xa000000000000000, 0x4000000000000000,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
];

#[rustfmt::skip]
pub static WHITE_PAWN_FORWARD_MOVES: [Bitboard; 56] = [
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x10000, 0x20000, 0x40000, 0x80000, 0x100000, 0x200000, 0x400000, 0x800000,
    0x1000000, 0x2000000, 0x4000000, 0x8000000, 0x10000000, 0x20000000, 0x40000000, 0x80000000,
    0x100000000, 0x200000000, 0x400000000, 0x800000000, 0x1000000000, 0x2000000000, 0x4000000000, 0x8000000000,
    0x10000000000, 0x20000000000, 0x40000000000, 0x80000000000, 0x100000000000, 0x200000000000, 0x400000000000, 0x800000000000,
    0x1000000000000, 0x2000000000000, 0x4000000000000, 0x8000000000000, 0x10000000000000, 0x20000000000000, 0x40000000000000, 0x80000000000000,
    0x100000000000000, 0x200000000000000, 0x400000000000000, 0x800000000000000, 0x1000000000000000, 0x2000000000000000, 0x4000000000000000, 0x8000000000000000,
];

#[rustfmt::skip]
pub static BLACK_PAWN_ATTACKS: [Bitboard; 64] = [
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x2, 0x5, 0xa, 0x14, 0x28, 0x50, 0xa0, 0x40,
    0x200, 0x500, 0xa00, 0x1400, 0x2800, 0x5000, 0xa000, 0x4000,
    0x20000, 0x50000, 0xa0000, 0x140000, 0x280000, 0x500000, 0xa00000, 0x400000,
    0x2000000, 0x5000000, 0xa000000, 0x14000000, 0x28000000, 0x50000000, 0xa0000000, 0x40000000,
    0x200000000, 0x500000000, 0xa00000000, 0x1400000000, 0x2800000000, 0x5000000000, 0xa000000000, 0x4000000000,
    0x20000000000, 0x50000000000, 0xa0000000000, 0x140000000000, 0x280000000000, 0x500000000000, 0xa00000000000, 0x400000000000,
    0x2000000000000, 0x5000000000000, 0xa000000000000, 0x14000000000000, 0x28000000000000, 0x50000000000000, 0xa0000000000000, 0x40000000000000,
];

#[rustfmt::skip]
pub static BLACK_PAWN_FORWARD_MOVES: [Bitboard; 56] = [
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80,
    0x100, 0x200, 0x400, 0x800, 0x1000, 0x2000, 0x4000, 0x8000,
    0x10000, 0x20000, 0x40000, 0x80000, 0x100000, 0x200000, 0x400000, 0x800000,
    0x1000000, 0x2000000, 0x4000000, 0x8000000, 0x10000000, 0x20000000, 0x40000000, 0x80000000,
    0x100000000, 0x200000000, 0x400000000, 0x800000000, 0x1000000000, 0x2000000000, 0x4000000000, 0x8000000000,
    0x10000000000, 0x20000000000, 0x40000000000, 0x80000000000, 0x100000000000, 0x200000000000, 0x400000000000, 0x800000000000,
];

pub fn single_white_pawn_attacks(pawn: SingletonBitboard) -> Bitboard {
    WHITE_PAWN_ATTACKS[pawn.trailing_zeros() as usize]
}

pub fn single_black_pawn_attacks(pawn: SingletonBitboard) -> Bitboard {
    BLACK_PAWN_ATTACKS[pawn.trailing_zeros() as usize]
}

pub fn white_pawns_legal_moves_into(cb: &Chessboard, moves: &mut Vec<Move>, legality: &Legality) {

    let en_passant_square = cb.get_en_passant_bitboard();
    let occupancy = cb.get_occupancy();
    let black_occupancy = cb.get_black_occupancy();

    let mut remaining_pawns = cb.white_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        let square = single_pawn.trailing_zeros() as usize;
        // Where this pawn may land without leaving the king attacked: the
        // check mask, narrowed to the pin line when it is pinned.
        let allowed = legality.allowed(single_pawn, square);
        // Forward moves
        let forward = WHITE_PAWN_FORWARD_MOVES[square] & !occupancy;
        if forward != 0 {
            // Not promotion
            if forward & RANK_8 == 0 {
                if forward & allowed != 0 {
                    moves.push(cb.make_white_pawn_forward_move(single_pawn, forward));
                }

                // Double forward move (only from the second rank). The square
                // it steps over still has to be empty -- that is `forward` --
                // but it does not have to be one the pawn may stop on.
                if (single_pawn & RANK_2) != 0 {
                    let double_forward = (single_pawn << 16) & !occupancy;
                    if double_forward & allowed != 0 {
                        moves.push(
                            cb.make_white_pawn_double_forward_move(single_pawn, double_forward),
                        );
                    }
                }
                // Promotion
            } else if forward & allowed != 0 {
                // All four promotions stand or fall together: they differ only
                // in the piece left on the square, and no attack on our own
                // king can depend on which piece that is.
                for new_move in cb.make_all_white_pawn_promotion_moves(single_pawn, forward) {
                    moves.push(new_move);
                }
            }
        }

        // Attack moves
        let attacks = WHITE_PAWN_ATTACKS[square] & black_occupancy & allowed;
        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack & RANK_8 == 0 {
                moves.push(cb.make_white_pawn_capture_move(single_pawn, single_attack));
            } else {
                for new_move in
                    cb.make_all_white_pawn_capture_promotion_moves(single_pawn, single_attack)
                {
                    moves.push(new_move);
                }
            }

            remaining_attacks &= remaining_attacks - 1;
        }

        // En passant is the one move the masks cannot decide. It takes two
        // pieces off one rank, so it can uncover a rook that no pin describes,
        // and in check it can answer by capturing the checking pawn, which
        // `check_mask` would reject. Make it and look, as everything used to.
        if en_passant_square != 0 {
            let en_passant_mask = en_passant_square & WHITE_PAWN_ATTACKS[square];
            if en_passant_mask != 0 {
                let new_move =
                    cb.make_white_pawn_en_passant_capture(single_pawn, en_passant_mask);
                if !new_move.chessboard.is_white_king_under_attack() {
                    moves.push(new_move);
                }
            }
        }

        remaining_pawns &= remaining_pawns - 1;
        // println!("remaining pawns: {}", remaining_pawns);
    }
}

pub fn black_pawns_legal_moves_into(cb: &Chessboard, moves: &mut Vec<Move>, legality: &Legality) {

    let en_passant_square = cb.get_en_passant_bitboard();
    let occupancy = cb.get_occupancy();
    let white_occupancy = cb.get_white_occupancy();

    let mut remaining_pawns = cb.black_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        let square = single_pawn.trailing_zeros() as usize;
        let allowed = legality.allowed(single_pawn, square);

        // Forward moves
        let forward = BLACK_PAWN_FORWARD_MOVES[square] & !occupancy;
        if forward != 0 {
            // Not promotion
            if forward & RANK_1 == 0 {
                if forward & allowed != 0 {
                    moves.push(cb.make_black_pawn_forward_move(single_pawn, forward));
                }

                // Double forward move (only from the seventh rank)
                if (single_pawn & RANK_7) != 0 {
                    let double_forward = (single_pawn >> 16) & !occupancy;
                    if double_forward & allowed != 0 {
                        moves.push(
                            cb.make_black_pawn_double_forward_move(single_pawn, double_forward),
                        );
                    }
                }
            } else if forward & allowed != 0 {
                // Promotion. All four differ only in the piece left behind.
                for new_move in cb.make_all_black_pawn_promotion_moves(single_pawn, forward) {
                    moves.push(new_move);
                }
            }
        }

        // Attack moves
        let attacks = BLACK_PAWN_ATTACKS[square] & white_occupancy & allowed;
        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack & RANK_1 == 0 {
                moves.push(cb.make_black_pawn_capture_move(single_pawn, single_attack));
            } else {
                // Capture promotion
                for new_move in
                    cb.make_all_black_pawn_capture_promotion_moves(single_pawn, single_attack)
                {
                    moves.push(new_move);
                }
            }

            remaining_attacks &= remaining_attacks - 1;
        }

        // En passant capture
        if en_passant_square != 0 {
            let en_passant_mask = en_passant_square & BLACK_PAWN_ATTACKS[square];
            if en_passant_mask != 0 {
                let new_move =
                    cb.make_black_pawn_en_passant_capture(single_pawn, en_passant_mask);
                if !new_move.chessboard.is_black_king_under_attack() {
                    moves.push(new_move);
                }
            }
        }
        remaining_pawns &= remaining_pawns - 1;
    }

}


/// The pawn moves the capture stage does not produce: the plain pushes.
///
/// Promotion pushes are absent on purpose. They score 6-9, at or above the
/// threshold quiescence keeps, so `*_pawn_captures_into` produces them and
/// generating them here as well would search each one twice. Together the two
/// functions are exactly `*_pawns_legal_moves_into`, which
/// `capture_and_quiet_stages_partition_the_full_generator` checks.
pub fn white_pawn_quiets_into(cb: &Chessboard, moves: &mut Vec<Move>, legality: &Legality) {
    let occupancy = cb.get_occupancy();

    let mut remaining_pawns = cb.white_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        let square = single_pawn.trailing_zeros() as usize;
        let allowed = legality.allowed(single_pawn, square);

        let forward = WHITE_PAWN_FORWARD_MOVES[square] & !occupancy;
        if forward != 0 && forward & RANK_8 == 0 {
            if forward & allowed != 0 {
                moves.push(cb.make_white_pawn_forward_move(single_pawn, forward));
            }

            if (single_pawn & RANK_2) != 0 {
                let double_forward = (single_pawn << 16) & !occupancy;
                if double_forward & allowed != 0 {
                    moves.push(cb.make_white_pawn_double_forward_move(single_pawn, double_forward));
                }
            }
        }

        remaining_pawns &= remaining_pawns - 1;
    }
}

pub fn black_pawn_quiets_into(cb: &Chessboard, moves: &mut Vec<Move>, legality: &Legality) {
    let occupancy = cb.get_occupancy();

    let mut remaining_pawns = cb.black_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        let square = single_pawn.trailing_zeros() as usize;
        let allowed = legality.allowed(single_pawn, square);

        let forward = BLACK_PAWN_FORWARD_MOVES[square] & !occupancy;
        if forward != 0 && forward & RANK_1 == 0 {
            if forward & allowed != 0 {
                moves.push(cb.make_black_pawn_forward_move(single_pawn, forward));
            }

            if (single_pawn & RANK_7) != 0 {
                let double_forward = (single_pawn >> 16) & !occupancy;
                if double_forward & allowed != 0 {
                    moves.push(cb.make_black_pawn_double_forward_move(single_pawn, double_forward));
                }
            }
        }

        remaining_pawns &= remaining_pawns - 1;
    }
}

/// The pawn moves quiescence keeps -- captures, capture-promotions
/// and en passant. Quiet pushes (including quiet promotions) score below the
/// capture threshold and are discarded, so they are never generated.
pub fn white_pawn_captures_into(cb: &Chessboard, moves: &mut Vec<Move>, legality: &Legality) {
    let en_passant_square = cb.get_en_passant_bitboard();
    let black_occupancy = cb.get_black_occupancy();
    let occupancy = cb.get_occupancy();

    let mut remaining_pawns = cb.white_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        let square = single_pawn.trailing_zeros() as usize;
        let allowed = legality.allowed(single_pawn, square);

        // A promotion push is quiet but decisive, and all four pieces score at
        // or above the capture threshold, so the old filter kept them.
        if single_pawn & RANK_7 != 0 {
            let forward = WHITE_PAWN_FORWARD_MOVES[square] & !occupancy;
            if forward & allowed != 0 {
                for new_move in cb.make_all_white_pawn_promotion_moves(single_pawn, forward) {
                    moves.push(new_move);
                }
            }
        }

        let mut remaining_attacks = WHITE_PAWN_ATTACKS[square] & black_occupancy & allowed;
        while remaining_attacks != 0 {
            let single_attack = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack & RANK_8 == 0 {
                moves.push(cb.make_white_pawn_capture_move(single_pawn, single_attack));
            } else {
                for new_move in cb.make_all_white_pawn_capture_promotion_moves(single_pawn, single_attack) {
                    moves.push(new_move);
                }
            }
            remaining_attacks &= remaining_attacks - 1;
        }

        if en_passant_square != 0 {
            let en_passant_mask = en_passant_square & WHITE_PAWN_ATTACKS[square];
            if en_passant_mask != 0 {
                let new_move = cb.make_white_pawn_en_passant_capture(single_pawn, en_passant_mask);
                if !new_move.chessboard.is_white_king_under_attack() {
                    moves.push(new_move);
                }
            }
        }

        remaining_pawns &= remaining_pawns - 1;
    }
}

pub fn black_pawn_captures_into(cb: &Chessboard, moves: &mut Vec<Move>, legality: &Legality) {
    let en_passant_square = cb.get_en_passant_bitboard();
    let white_occupancy = cb.get_white_occupancy();
    let occupancy = cb.get_occupancy();

    let mut remaining_pawns = cb.black_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        let square = single_pawn.trailing_zeros() as usize;
        let allowed = legality.allowed(single_pawn, square);

        if single_pawn & RANK_2 != 0 {
            let forward = BLACK_PAWN_FORWARD_MOVES[square] & !occupancy;
            if forward & allowed != 0 {
                for new_move in cb.make_all_black_pawn_promotion_moves(single_pawn, forward) {
                    moves.push(new_move);
                }
            }
        }

        let mut remaining_attacks = BLACK_PAWN_ATTACKS[square] & white_occupancy & allowed;
        while remaining_attacks != 0 {
            let single_attack = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack & RANK_1 == 0 {
                moves.push(cb.make_black_pawn_capture_move(single_pawn, single_attack));
            } else {
                for new_move in cb.make_all_black_pawn_capture_promotion_moves(single_pawn, single_attack) {
                    moves.push(new_move);
                }
            }
            remaining_attacks &= remaining_attacks - 1;
        }

        if en_passant_square != 0 {
            let en_passant_mask = en_passant_square & BLACK_PAWN_ATTACKS[square];
            if en_passant_mask != 0 {
                let new_move = cb.make_black_pawn_en_passant_capture(single_pawn, en_passant_mask);
                if !new_move.chessboard.is_black_king_under_attack() {
                    moves.push(new_move);
                }
            }
        }

        remaining_pawns &= remaining_pawns - 1;
    }
}
