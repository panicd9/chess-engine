use crate::{chessboard::{Bitboard, Chessboard, FILE_MASKS, RANK_MASKS}, utils::{calculate_sliding_attacks, ANTI_DIAGONAL_MASKS, DIAGONAL_MASKS}};

use super::{move_gen_bishop::{all_bishops_attacks, single_bishop_attacks}, move_gen_rook::{all_rooks_attacks, single_rook_attacks}};

const QUEEN_MOVES_CAPACITY: usize = 10;

// Generate single queen attacks by combining rook and bishop attacks.
pub fn single_queen_attacks(occupancy: u64, queen: u64) -> u64 {
    // TODO: trailing_zeros() can be called only once instead
    let rook_attacks = single_rook_attacks(occupancy, queen);
    let bishop_attacks = single_bishop_attacks(occupancy, queen);
    rook_attacks | bishop_attacks
}

pub fn all_queens_attacks(occupancy: u64, queens: u64) -> u64 {
    let mut combined_attacks = 0;
    let mut remaining_queens = queens;

    while remaining_queens != 0 {
        let single_queen = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB (single queen)
        let square_index = single_queen.trailing_zeros(); // Get square index (0–63)

        // Calculate rook-like attacks (rank and file)
        let rank_mask = RANK_MASKS[(square_index / 8) as usize];
        let rank_occupancy = occupancy & rank_mask;
        let rank_attacks = calculate_sliding_attacks(rank_mask, single_queen, rank_occupancy);

        let file_mask = FILE_MASKS[(square_index % 8) as usize];
        let file_occupancy = occupancy & file_mask;
        let file_attacks = calculate_sliding_attacks(file_mask, single_queen, file_occupancy);

        // Calculate bishop-like attacks (diagonals)
        let main_diagonal_mask = DIAGONAL_MASKS[square_index as usize];
        let main_diagonal_occupancy = occupancy & main_diagonal_mask;
        let main_diagonal_attacks = calculate_sliding_attacks(main_diagonal_mask, single_queen, main_diagonal_occupancy);

        let anti_diagonal_mask = ANTI_DIAGONAL_MASKS[square_index as usize];
        let anti_diagonal_occupancy = occupancy & anti_diagonal_mask;
        let anti_diagonal_attacks = calculate_sliding_attacks(anti_diagonal_mask, single_queen, anti_diagonal_occupancy);

        // Combine attacks
        combined_attacks |= rank_attacks | file_attacks | main_diagonal_attacks | anti_diagonal_attacks;

        // Remove the processed queen
        remaining_queens &= remaining_queens - 1;
    }

    combined_attacks
}

// Generate pseudolegal moves for white queens.
pub fn white_queens_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(QUEEN_MOVES_CAPACITY);
    let mut remaining_queens = cb.white_queens;

    while remaining_queens != 0 {
        let single_queen_bitboard = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB
        let occupancy = cb.get_occupancy();
        let attacks = single_queen_attacks(occupancy, single_queen_bitboard) & !cb.get_white_occupancy();

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
pub fn black_queens_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(QUEEN_MOVES_CAPACITY);
    let mut remaining_queens = cb.black_queens;

    while remaining_queens != 0 {
        let single_queen_bitboard = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB
        let occupancy = cb.get_occupancy();
        let attacks = single_queen_attacks(occupancy, single_queen_bitboard) & !cb.get_black_occupancy();

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

pub fn white_queens_legal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(QUEEN_MOVES_CAPACITY);
    let mut remaining_queens = cb.white_queens;

    while remaining_queens != 0 {
        let single_queen_bitboard = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB
        let occupancy = cb.get_occupancy();
        let attacks = single_queen_attacks(occupancy, single_queen_bitboard) & !cb.get_white_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_white_queen_move(single_queen_bitboard, single_attack_bitboard);
            if !new_position.is_white_king_under_attack() {
                new_positions.push(new_position);
            }
            remaining_attacks &= remaining_attacks - 1; // Remove LSB
        }

        remaining_queens &= remaining_queens - 1; // Remove LSB
    }

    new_positions
}

pub fn black_queens_legal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(QUEEN_MOVES_CAPACITY);
    let mut remaining_queens = cb.black_queens;

    while remaining_queens != 0 {
        let single_queen_bitboard = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB
        let occupancy = cb.get_occupancy();
        let attacks = single_queen_attacks(occupancy, single_queen_bitboard) & !cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_black_queen_move(single_queen_bitboard, single_attack_bitboard);
            if !new_position.is_black_king_under_attack() {
                new_positions.push(new_position);
            }
            remaining_attacks &= remaining_attacks - 1; // Remove LSB
        }

        remaining_queens &= remaining_queens - 1; // Remove LSB
    }

    new_positions
}
