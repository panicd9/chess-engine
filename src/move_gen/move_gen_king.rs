use crate::chessboard::{Bitboard, Chessboard, Square};

pub const KING_MOVES_CAPACITY: usize = 4;

// Define the king move lookup table for each square on an 8x8 chessboard
#[rustfmt::skip]
static KING_ATTACKS: [Bitboard; 64] = [
    0x302, 0x705, 0xe0a, 0x1c14, 0x3828, 0x7050, 0xe0a0, 0xc040,
    0x30203, 0x70507, 0xe0a0e, 0x1c141c, 0x382838, 0x705070, 0xe0a0e0, 0xc040c0,
    0x3020300, 0x7050700, 0xe0a0e00, 0x1c141c00, 0x38283800, 0x70507000, 0xe0a0e000, 0xc040c000,
    0x302030000, 0x705070000, 0xe0a0e0000, 0x1c141c0000, 0x3828380000, 0x7050700000, 0xe0a0e00000, 0xc040c00000,
    0x30203000000, 0x70507000000, 0xe0a0e000000, 0x1c141c000000, 0x382838000000, 0x705070000000, 0xe0a0e0000000, 0xc040c0000000,
    0x3020300000000, 0x7050700000000, 0xe0a0e00000000, 0x1c141c00000000, 0x38283800000000, 0x70507000000000, 0xe0a0e000000000, 0xc040c000000000,
    0x302030000000000, 0x705070000000000, 0xe0a0e0000000000, 0x1c141c0000000000, 0x3828380000000000, 0x7050700000000000, 0xe0a0e00000000000, 0xc040c00000000000,
    0x203000000000000, 0x507000000000000, 0xa0e000000000000, 0x141c000000000000, 0x2838000000000000, 0x5070000000000000, 0xa0e0000000000000, 0x40c0000000000000,
];

// Function to get the king attacks for a given square
#[inline(always)]
fn king_attacks(square: Square) -> Bitboard {
    // Return the pre-calculated attack pattern for the given square
    KING_ATTACKS[square]
}

fn king_attacks_from_single_king_bitboard(king: Bitboard) -> Bitboard {
    // Get the square of the king
    let square = king.trailing_zeros() as usize;

    // Get the attack pattern for the king
    king_attacks(square)
}

fn white_kings_pseudolegal_moves(cb: &Chessboard, king: Bitboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(KING_MOVES_CAPACITY);
    
    let attacks = king_attacks_from_single_king_bitboard(king) ^ cb.get_white_occupancy();

    let mut remaining_attacks = attacks;
    while remaining_attacks != 0 {
        let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
        let new_position = cb.make_white_king_move(king, single_attack_bitboard);
        new_positions.push(new_position);
        remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
    }

    new_positions
}

fn black_kings_pseudolegal_moves(cb: &Chessboard, king: Bitboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(KING_MOVES_CAPACITY);
    
    let attacks = king_attacks_from_single_king_bitboard(king) ^ cb.get_black_occupancy();

    let mut remaining_attacks = attacks;
    while remaining_attacks != 0 {
        let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
        let new_position = cb.make_black_king_move(king, single_attack_bitboard);
        new_positions.push(new_position);
        remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
    }

    new_positions
}