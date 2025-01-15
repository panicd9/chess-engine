use crate::{chessboard::{Bitboard, Chessboard}, utils::{calculate_sliding_attacks, reverse_bits, ANTI_DIAGONAL_MASKS, DIAGONAL_MASKS}};

const BISHOPS_MOVES_CAPACITY: usize = 14;

// Generate bishop attacks on a diagonal using Hyperbola Quintessence.
fn single_bishop_diagonal_attacks(occupancy: u64, bishop: u64) -> u64 {
    let mask = DIAGONAL_MASKS[bishop.trailing_zeros() as usize];
    let diagonal_occupancy = occupancy & mask;

    let mut forward = diagonal_occupancy.wrapping_sub(2 * bishop);
    let reverse = reverse_bits(diagonal_occupancy).wrapping_sub(2 * reverse_bits(bishop));

    forward ^= reverse_bits(reverse);
    forward & mask
}

// Generate bishop attacks on an anti-diagonal using Hyperbola Quintessence.
fn single_bishop_anti_diagonal_attacks(occupancy: u64, bishop: u64) -> u64 {
    let mask = ANTI_DIAGONAL_MASKS[bishop.trailing_zeros() as usize];
    let anti_diagonal_occupancy = occupancy & mask;

    let mut forward = anti_diagonal_occupancy.wrapping_sub(2 * bishop);
    let reverse = reverse_bits(anti_diagonal_occupancy).wrapping_sub(2 * reverse_bits(bishop));

    forward ^= reverse_bits(reverse);
    forward & mask
}

// Generate bishop attacks combining both diagonals and anti-diagonals.
pub fn single_bishop_attacks(occupancy: u64, bishop: u64) -> u64 {
    single_bishop_diagonal_attacks(occupancy, bishop) | single_bishop_anti_diagonal_attacks(occupancy, bishop)
}

pub fn all_bishops_attacks(occupancy: u64, bishops: u64) -> u64 {
    let mut diagonal_attacks = 0;
    let mut remaining_bishops = bishops;

    while remaining_bishops != 0 {
        let single_bishop = remaining_bishops & remaining_bishops.wrapping_neg(); // Extract LSB (single bishop)
        let square_index = single_bishop.trailing_zeros(); // Get square index (0–63)

        // Main diagonal attack
        let main_diagonal_mask = DIAGONAL_MASKS[square_index as usize];
        let main_diagonal_occupancy = occupancy & main_diagonal_mask;
        diagonal_attacks |= calculate_sliding_attacks(main_diagonal_mask, single_bishop, main_diagonal_occupancy);

        // Anti-diagonal attack
        let anti_diagonal_mask = ANTI_DIAGONAL_MASKS[square_index as usize];
        let anti_diagonal_occupancy = occupancy & anti_diagonal_mask;
        diagonal_attacks |= calculate_sliding_attacks(anti_diagonal_mask, single_bishop, anti_diagonal_occupancy);


        remaining_bishops &= remaining_bishops - 1; // Remove LSB
    }

    diagonal_attacks
}

pub fn white_bishops_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(BISHOPS_MOVES_CAPACITY);
    let mut remaining_bishops = cb.white_bishops;

    while remaining_bishops != 0 {
        let single_bishop_bitboard = remaining_bishops & remaining_bishops.wrapping_neg(); // Get the least significant bishop
        let occupancy = cb.get_occupancy();
        let attacks = single_bishop_attacks(occupancy, single_bishop_bitboard) ^ cb.get_white_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_white_bishop_move(single_bishop_bitboard, single_attack_bitboard);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }
        
        remaining_bishops &= remaining_bishops - 1; // Remove the least significant bishop
    }

    new_positions
}

pub fn black_bishops_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(BISHOPS_MOVES_CAPACITY);
    let mut remaining_bishops = cb.black_bishops;

    while remaining_bishops != 0 {
        let single_bishop_bitboard = remaining_bishops & remaining_bishops.wrapping_neg(); // Get the least significant bishop
        let occupancy = cb.get_occupancy();
        let attacks = single_bishop_attacks(occupancy, single_bishop_bitboard) ^ cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_black_bishop_move(single_bishop_bitboard, single_attack_bitboard);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }
        
        remaining_bishops &= remaining_bishops - 1; // Remove the least significant bishop
    }

    new_positions
}

pub fn white_bishops_legal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(BISHOPS_MOVES_CAPACITY);
    let mut remaining_bishops = cb.white_bishops;

    while remaining_bishops != 0 {
        let single_bishop_bitboard = remaining_bishops & remaining_bishops.wrapping_neg(); // Get the least significant bishop
        let occupancy = cb.get_occupancy();
        let attacks = single_bishop_attacks(occupancy, single_bishop_bitboard) ^ cb.get_white_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_white_bishop_move(single_bishop_bitboard, single_attack_bitboard);
            if !new_position.is_white_king_under_attack() {
                new_positions.push(new_position);
            }
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }
        
        remaining_bishops &= remaining_bishops - 1; // Remove the least significant bishop
    }

    new_positions
}

pub fn black_bishops_legal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(BISHOPS_MOVES_CAPACITY);
    let mut remaining_bishops = cb.black_bishops;

    while remaining_bishops != 0 {
        let single_bishop_bitboard = remaining_bishops & remaining_bishops.wrapping_neg(); // Get the least significant bishop
        let occupancy = cb.get_occupancy();
        let attacks = single_bishop_attacks(occupancy, single_bishop_bitboard) ^ cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_black_bishop_move(single_bishop_bitboard, single_attack_bitboard);
            if !new_position.is_black_king_under_attack() {
                new_positions.push(new_position);
            }
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }

        remaining_bishops &= remaining_bishops - 1; // Remove the least significant bishop
    }

    new_positions
}
