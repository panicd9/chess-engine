use crate::chessboard::{
    file_masks::{FILE_A, FILE_H}, rank_masks::{RANK_2, RANK_7}, Bitboard, Chessboard
};

pub const PAWNS_MOVES_CAPACITY: usize = 24;

#[rustfmt::skip]
static WHITE_PAWN_ATTACKS: [Bitboard; 56] = [
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x20000, 0x50000, 0xa0000, 0x140000, 0x280000, 0x500000, 0xa00000, 0x400000,
    0x2000000, 0x5000000, 0xa000000, 0x14000000, 0x28000000, 0x50000000, 0xa0000000, 0x40000000,
    0x200000000, 0x500000000, 0xa00000000, 0x1400000000, 0x2800000000, 0x5000000000, 0xa000000000, 0x4000000000,
    0x20000000000, 0x50000000000, 0xa0000000000, 0x140000000000, 0x280000000000, 0x500000000000, 0xa00000000000, 0x400000000000,
    0x2000000000000, 0x5000000000000, 0xa000000000000, 0x14000000000000, 0x28000000000000, 0x50000000000000, 0xa0000000000000, 0x40000000000000,
    0x200000000000000, 0x500000000000000, 0xa000000000000000, 0x1400000000000000, 0x2800000000000000, 0x5000000000000000, 0xa000000000000000, 0x4000000000000000,
];

#[rustfmt::skip]
static WHITE_PAWN_FORWARD_MOVES: [Bitboard; 56] = [
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x10000, 0x20000, 0x40000, 0x80000, 0x100000, 0x200000, 0x400000, 0x800000,
    0x1000000, 0x2000000, 0x4000000, 0x8000000, 0x10000000, 0x20000000, 0x40000000, 0x80000000,
    0x100000000, 0x200000000, 0x400000000, 0x800000000, 0x1000000000, 0x2000000000, 0x4000000000, 0x8000000000,
    0x10000000000, 0x20000000000, 0x40000000000, 0x80000000000, 0x100000000000, 0x200000000000, 0x400000000000, 0x800000000000,
    0x1000000000000, 0x2000000000000, 0x4000000000000, 0x8000000000000, 0x10000000000000, 0x20000000000000, 0x40000000000000, 0x80000000000000,
    0x100000000000000, 0x200000000000000, 0x400000000000000, 0x800000000000000, 0x1000000000000000, 0x2000000000000000, 0x4000000000000000, 0x8000000000000000,
];

#[rustfmt::skip]
static BLACK_PAWN_ATTACKS: [Bitboard; 56] = [
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x2, 0x5, 0xa, 0x14, 0x28, 0x50, 0xa0, 0x40,
    0x200, 0x500, 0xa00, 0x1400, 0x2800, 0x5000, 0xa000, 0x4000,
    0x20000, 0x50000, 0xa0000, 0x140000, 0x280000, 0x500000, 0xa00000, 0x400000,
    0x2000000, 0x5000000, 0xa000000, 0x14000000, 0x28000000, 0x50000000, 0xa0000000, 0x40000000,
    0x200000000, 0x500000000, 0xa00000000, 0x1400000000, 0x2800000000, 0x5000000000, 0xa000000000, 0x4000000000,
    0x20000000000, 0x50000000000, 0xa0000000000, 0x140000000000, 0x280000000000, 0x500000000000, 0xa00000000000, 0x400000000000,
];

#[rustfmt::skip]
static BLACK_PAWN_MOVES: [Bitboard; 56] = [
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80,
    0x100, 0x200, 0x400, 0x800, 0x1000, 0x2000, 0x4000, 0x8000,
    0x10000, 0x20000, 0x40000, 0x80000, 0x100000, 0x200000, 0x400000, 0x800000,
    0x1000000, 0x2000000, 0x4000000, 0x8000000, 0x10000000, 0x20000000, 0x40000000, 0x80000000,
    0x100000000, 0x200000000, 0x400000000, 0x800000000, 0x1000000000, 0x2000000000, 0x4000000000, 0x8000000000,
    0x10000000000, 0x20000000000, 0x40000000000, 0x80000000000, 0x100000000000, 0x200000000000, 0x400000000000, 0x800000000000,
];

pub fn white_pawns_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(PAWNS_MOVES_CAPACITY);

    // TODO: BENCHMARK THIS
    // let en_passant_square = cb.get_en_passant_bitboard();
    // if en_passant_square != 0 {
    //     // Handle en passant captures separately
    //     let mut remaining_pawns = pawns;
    //     while remaining_pawns != 0 {
    //         let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
    //         let square = single_pawn.trailing_zeros() as usize;

    //         // En passant capture
    //         let en_passant_mask = en_passant_square & WHITE_PAWN_ATTACKS[square];
    //         if en_passant_mask != 0 {
    //             let new_position = cb.make_white_pawn_en_passant_capture(single_pawn, en_passant_mask);
    //             new_positions.push(new_position);
    //         }

    //         remaining_pawns &= remaining_pawns - 1;
    //     }
    // }
    // // Handle all other moves

    let en_passant_square = cb.get_en_passant_bitboard();

    let mut remaining_pawns = cb.white_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        let square = single_pawn.trailing_zeros() as usize;

        // Forward moves
        let forward = WHITE_PAWN_FORWARD_MOVES[square] & !cb.get_occupancy();
        if forward != 0 {
            let new_position = cb.make_white_pawn_forward_move(single_pawn, forward);
            new_positions.push(new_position);

            // Double forward move (only from the second rank)
            if (single_pawn & RANK_2) != 0 {
                let double_forward = (single_pawn << 16) & !cb.get_occupancy();
                if double_forward != 0 {
                    let new_double_position =
                        cb.make_white_pawn_double_forward_move(single_pawn, double_forward);
                    new_positions.push(new_double_position);
                }
            }
        }

        // Attack moves
        let attacks = WHITE_PAWN_ATTACKS[square] & cb.get_black_occupancy();
        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_white_pawn_capture_move(single_pawn, single_attack);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1;
        }

        // En passant capture
        if en_passant_square != 0 {
            let en_passant_mask = en_passant_square & WHITE_PAWN_ATTACKS[square];
            if en_passant_mask != 0 {
                let new_position = cb.make_white_pawn_en_passant_capture(single_pawn, en_passant_mask);
                new_positions.push(new_position);
            }
        }

        remaining_pawns &= remaining_pawns - 1;
    }

    new_positions
}

pub fn black_pawns_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(PAWNS_MOVES_CAPACITY);

    let en_passant_square = cb.get_en_passant_bitboard();

    let mut remaining_pawns = cb.black_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        let square = single_pawn.trailing_zeros() as usize;

        // Forward moves
        let forward = BLACK_PAWN_MOVES[square] & !cb.get_occupancy();
        if forward != 0 {
            let new_position = cb.make_black_pawn_forward_move(single_pawn, forward);
            new_positions.push(new_position);

            // Double forward move (only from the seventh rank)
            if (single_pawn & RANK_7) != 0 {
                let double_forward = (single_pawn >> 16) & !cb.get_occupancy();
                if double_forward != 0 {
                    let new_double_position =
                        cb.make_black_pawn_double_forward_move(single_pawn, double_forward);
                    new_positions.push(new_double_position);
                }
            }
        }

        // Attack moves
        let attacks = BLACK_PAWN_ATTACKS[square] & cb.get_white_occupancy();
        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_black_pawn_capture_move(single_pawn, single_attack);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1;
        }

        // En passant capture
        if en_passant_square != 0 {
            let en_passant_mask = en_passant_square & BLACK_PAWN_ATTACKS[square];
            if en_passant_mask != 0 {
                let new_position = cb.make_black_pawn_en_passant_capture(single_pawn, en_passant_mask);
                new_positions.push(new_position);
            }
        }

        remaining_pawns &= remaining_pawns - 1;
    }

    new_positions
}