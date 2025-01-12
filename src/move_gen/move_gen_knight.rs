use crate::chessboard::{Bitboard, Chessboard, Square};

pub const KNIGHTS_MOVES_CAPACITY: usize = 16;

// Define the knight move lookup table for each square on an 8x8 chessboard
#[rustfmt::skip]
static KNIGHT_ATTACKS: [Bitboard; 64] = [
    0x20400, 0x50800, 0xa1100, 0x142200, 0x284400, 0x508800, 0xa01000, 0x402000,
    0x2040004, 0x5080008, 0xa110411, 0x14220822, 0x28441044, 0x50882088, 0xa0104010, 0x40208020,
    0x204000402, 0x508000805, 0xa1100110a, 0x1422002214, 0x2844004428, 0x5088008850, 0xa0100010a0, 0x4020002040,
    0x20400040200, 0x50800080500, 0xa1100110a00, 0x142200221400, 0x284400442800, 0x508800885000, 0xa0100010a000, 0x402000204000,
    0x2040004020000, 0x5080008050000, 0xa1100110a0000, 0x14220022140000, 0x28440044280000, 0x50880088500000, 0xa0100010a00000, 0x40200020400000,
    0x204000402000000, 0x508000805000000, 0xa1100110a000000, 0x1422002214000000, 0x2844004428000000, 0x5088008850000000, 0xa0100010a0000000, 0x4020002040000000,
    0x400040200000000, 0x800080500000000, 0x1100110a00000000, 0x2200221400000000, 0x4400442800000000, 0x8800885000000000, 0x100010a000000000, 0x2000204000000000,
    0x4020000000000, 0x8050000000000, 0x110a0000000000, 0x22140000000000, 0x44280000000000, 0x88500000000000, 0x10a00000000000, 0x20400000000000
];

// Function to get the knight attacks for a given square
#[inline(always)]
fn knight_attacks(square: Square) -> Bitboard {
    // Return the pre-calculated attack pattern for the given square
    KNIGHT_ATTACKS[square]
}

fn knight_attacks_from_single_knight_bitboard(knight: Bitboard) -> Bitboard {
    // Get the square of the knight
    let square = knight.trailing_zeros() as usize;

    // Get the attack pattern for the knight
    knight_attacks(square)
}

fn white_knights_pseudolegal_moves(cb: &Chessboard, knights: Bitboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(KNIGHTS_MOVES_CAPACITY);
    let mut remaining_knights = knights;

    while remaining_knights != 0 {
        let single_knight_bitboard = remaining_knights & remaining_knights.wrapping_neg(); // Get the least significant knight
        let attacks = knight_attacks_from_single_knight_bitboard(single_knight_bitboard) ^ cb.get_white_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_white_knight_move(single_knight_bitboard, single_attack_bitboard);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }
        
        remaining_knights &= remaining_knights - 1; // Remove the least significant knight
    }

    new_positions
}

fn black_knights_pseudolegal_moves(cb: &Chessboard, knights: Bitboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(KNIGHTS_MOVES_CAPACITY);
    let mut remaining_knights = knights;

    while remaining_knights != 0 {
        let single_knight_bitboard = remaining_knights & remaining_knights.wrapping_neg(); // Get the least significant knight
        let attacks = knight_attacks_from_single_knight_bitboard(single_knight_bitboard) ^ cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_black_knight_move(single_knight_bitboard, single_attack_bitboard);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }
        
        remaining_knights &= remaining_knights - 1; // Remove the least significant knight
    }

    new_positions
}