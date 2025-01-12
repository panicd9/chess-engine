use crate::{chessboard::{Bitboard, Chessboard}, utils::calculate_sliding_attacks};

use super::{move_gen_bishop::single_bishop_attacks, move_gen_rook::single_rook_attacks};

const QUEEN_MOVES_CAPACITY: usize = 42;

// Generate single queen attacks by combining rook and bishop attacks.
fn single_queen_attacks(occupancy: u64, queen: u64) -> u64 {
    let rook_attacks = single_rook_attacks(occupancy, queen);
    let bishop_attacks = single_bishop_attacks(occupancy, queen);
    rook_attacks | bishop_attacks
}

// Generate all queen attacks on the board.
fn all_queens_attacks(occupancy: u64, queens: u64) -> u64 {
    let mut combined_attacks = 0;
    let mut remaining_queens = queens;

    while remaining_queens != 0 {
        let single_queen = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB (single queen)
        combined_attacks |= single_queen_attacks(occupancy, single_queen);
        remaining_queens &= remaining_queens - 1; // Remove LSB
    }

    combined_attacks
}

// Generate pseudolegal moves for white queens.
fn white_queens_pseudolegal_moves(cb: &Chessboard, queens: Bitboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(QUEEN_MOVES_CAPACITY);
    let mut remaining_queens = queens;

    while remaining_queens != 0 {
        let single_queen_bitboard = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB
        let occupancy = cb.get_occupancy();
        let attacks = single_queen_attacks(occupancy, single_queen_bitboard) ^ cb.get_white_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_white_queen_move(single_queen_bitboard, single_attack_bitboard);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1; // Remove LSB
        }

        remaining_queens &= remaining_queens - 1; // Remove LSB
    }

    new_positions
}

// Generate pseudolegal moves for black queens.
fn black_queens_pseudolegal_moves(cb: &Chessboard, queens: Bitboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(QUEEN_MOVES_CAPACITY);
    let mut remaining_queens = queens;

    while remaining_queens != 0 {
        let single_queen_bitboard = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB
        let occupancy = cb.get_occupancy();
        let attacks = single_queen_attacks(occupancy, single_queen_bitboard) ^ cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_black_queen_move(single_queen_bitboard, single_attack_bitboard);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1; // Remove LSB
        }

        remaining_queens &= remaining_queens - 1; // Remove LSB
    }

    new_positions
}
