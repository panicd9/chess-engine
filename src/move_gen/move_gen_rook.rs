use crate::{chessboard::{Bitboard, Chessboard, FILE_MASKS, RANK_MASKS}, display::display_board, utils::{calculate_sliding_attacks, reverse_bits}};

// use super::{Bitboard, Chessboard, FILE_MASKS, RANK_MASKS};
// use crate::r#move::Move;
// use crate::piece::Piece;
// use crate::utils::*;
// use crate::display::*;

const ROOKS_MOVES_CAPACITY: usize = 15;

// Generate rook attacks on a rank using Hyperbola Quintessence.
fn single_rook_rank_attacks(occupancy: u64, rook: u64) -> u64 {
    let rank = (rook.trailing_zeros() / 8) as usize;
    let mask = RANK_MASKS[rank];
    let rank_occupancy = occupancy & mask;

    let mut forward = rank_occupancy.wrapping_sub(rook.wrapping_mul(2));
    let reverse = reverse_bits(rank_occupancy).wrapping_sub(reverse_bits(rook).wrapping_mul(2));

    forward ^= reverse_bits(reverse);
    forward & mask
}

// Generate rook attacks on a file using Hyperbola Quintessence.
fn single_rook_file_attacks(occupancy: u64, rook: u64) -> u64 {
    let file = (rook.trailing_zeros() % 8) as usize;
    let mask = FILE_MASKS[file];
    let file_occupancy = occupancy & mask;

    let mut forward = file_occupancy.wrapping_sub(rook.wrapping_mul(2)) ;
    let reverse = reverse_bits(file_occupancy).wrapping_sub(reverse_bits(rook).wrapping_mul(2));

    forward ^= reverse_bits(reverse);
    forward & mask
}

// Generate rook attacks combining both rank and file attacks.
pub fn single_rook_attacks(occupancy: u64, rook: u64) -> u64 {
    single_rook_rank_attacks(occupancy, rook) | single_rook_file_attacks(occupancy, rook)
}

pub fn all_rooks_attacks(occupancy: Bitboard, rooks: Bitboard) -> Bitboard {
    let mut total_attacks = 0;
    let mut remaining_rooks = rooks;

    while remaining_rooks != 0 {
        let single_rook = remaining_rooks & remaining_rooks.wrapping_neg(); // Extract LSB (single rook)
        let square = single_rook.trailing_zeros();

        // Rank calculation
        let rank = square / 8;
        let rank_mask = RANK_MASKS[rank as usize];
        let rank_occupancy = occupancy & rank_mask;
        total_attacks |= calculate_sliding_attacks(rank_mask, single_rook, rank_occupancy);

        // File calculation
        let file = square % 8;
        let file_mask = FILE_MASKS[file as usize];
        let file_occupancy = occupancy & file_mask;
        total_attacks |= calculate_sliding_attacks(file_mask, single_rook, file_occupancy);

        // Remove the processed rook
        remaining_rooks &= remaining_rooks - 1;
    }

    total_attacks
}


pub fn white_rooks_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(ROOKS_MOVES_CAPACITY);
    let mut remaining_rooks = cb.white_rooks;

    while remaining_rooks != 0 {
        let single_rook_bitboard = remaining_rooks & remaining_rooks.wrapping_neg(); // Get the least significant rook
        let occupancy = cb.get_occupancy();
        let attacks = single_rook_attacks(occupancy, single_rook_bitboard) & !cb.get_white_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            // let mov = Move::new(cb, Piece::Rook, single_rook_bitboard, single_attack_bitboard);
            let new_position = cb.make_white_rook_move(single_rook_bitboard, single_attack_bitboard);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }
        
        remaining_rooks &= remaining_rooks - 1; // Remove the least significant rook
    }

    new_positions
}

pub fn black_rooks_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(ROOKS_MOVES_CAPACITY);
    let mut remaining_rooks = cb.black_rooks;

    while remaining_rooks != 0 {
        let single_rook_bitboard = remaining_rooks & remaining_rooks.wrapping_neg(); // Get the least significant rook
        let occupancy = cb.get_occupancy();
        let attacks = single_rook_attacks(occupancy, single_rook_bitboard) & !cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            // let mov = Move::new(cb, Piece::Rook, single_rook_bitboard, single_attack_bitboard);
            let new_position = cb.make_black_rook_move(single_rook_bitboard, single_attack_bitboard);
            new_positions.push(new_position);
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }
        
        remaining_rooks &= remaining_rooks - 1; // Remove the least significant rook
    }

    new_positions
}

pub fn white_rooks_legal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(ROOKS_MOVES_CAPACITY);
    let mut remaining_rooks = cb.white_rooks;

    while remaining_rooks != 0 {
        let single_rook_bitboard = remaining_rooks & remaining_rooks.wrapping_neg(); // Get the least significant rook
        let occupancy = cb.get_occupancy();
        let attacks = single_rook_attacks(occupancy, single_rook_bitboard) & !cb.get_white_occupancy();
        
        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            // let mov = Move::new(cb, Piece::Rook, single_rook_bitboard, single_attack_bitboard);
            let new_position = cb.make_white_rook_move(single_rook_bitboard, single_attack_bitboard);
            if !new_position.is_white_king_under_attack() {
                new_positions.push(new_position);
            }
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }
        
        remaining_rooks &= remaining_rooks - 1; // Remove the least significant rook
    }

    new_positions
}

pub fn black_rooks_legal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(ROOKS_MOVES_CAPACITY);
    let mut remaining_rooks = cb.black_rooks;

    while remaining_rooks != 0 {
        let single_rook_bitboard = remaining_rooks & remaining_rooks.wrapping_neg(); // Get the least significant rook
        let occupancy = cb.get_occupancy();
        let attacks = single_rook_attacks(occupancy, single_rook_bitboard) & !cb.get_black_occupancy();
        
        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            let new_position = cb.make_black_rook_move(single_rook_bitboard, single_attack_bitboard);
            if !new_position.is_black_king_under_attack() {
                new_positions.push(new_position);
            }
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
        }

        remaining_rooks &= remaining_rooks - 1; // Remove the least significant rook
    }

    new_positions
}


#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn test_rooks_pseudolegal_moves() {
    //     let rooks_bitboard = 1 << 35; // D5
    //     let occupancy = 1 << 35; // No pieces on the board except rook on D5
    //     let calculated_rook_moves = rooks_pseudo_legal_moves(occupancy, rooks_bitboard);
        
    //     let expected = 0x80808f708080808; // Expected attacks

    //     assert_eq!(calculated_rook_move.len(), expected);

    //     assert_eq!(calculated_rook_attacks, expected);
    // }

    #[test]
    fn test_rook_attacks_empty_board() {
        let rook_square = 35; // D5
        let occupancy = 1 << 35; // No pieces on the board except rook on D5
        let expected = 0x80808f708080808; // Expected attacks
        let calculated_rook_attacks = single_rook_attacks(occupancy, rook_square);

        assert_eq!(calculated_rook_attacks, expected);
    }

    #[test]
    fn test_rook_attacks_with_blockers() {
        let rook_square =  35; // D5
        let occupancy = 0x8008100000800; // Blockers on A5, H5, D2, D7
        let expected = 0x808f708080800; // Expected attacks
        let calculated_rook_attacks = single_rook_attacks(occupancy, rook_square);

        assert_eq!(calculated_rook_attacks, expected);
    }

    // #[test]
    // fn test_multiple_rooks() {
    //     let rooks_bitboard = (1 << 35) | (1 << 0); // Rooks on D5 and A1
    //     let occupancy = (1 << 27) | (1 << 63); // Blockers on D4 and H8
    //     let expected = 0x80808F708080808 | 0x1010101010100FE; // Combined attacks for D5 and A1
    //     let calculated_rook_attacks = all_rooks_attacks(occupancy, rooks_bitboard);
    //     assert_eq!(calculated_rook_attacks, expected);
    // }

    // #[test]
    // fn test_rook_on_edge_square() {
    //     let rooks_bitboard = 1 << 63; // Rook on H8
    //     let occupancy = 0; // No pieces on the board
    //     let expected = 0x80808080808080FE; // Expected attacks for a rook on H8
    //     let calculated_rook_attacks = all_rooks_attacks(occupancy, rooks_bitboard);
    //     assert_eq!(calculated_rook_attacks, expected);
    // }

    // #[test]
    // fn test_rook_blocked_on_same_rank_and_file() {
    //     let rooks_bitboard = 1 << 35; // Rook on D5
    //     let occupancy = 0x808000000080808; // Blockers on D1, D8, A5, H5
    //     let expected = 0x808700708000000; // Expected attacks for a rook on D5 with these blockers
    //     let calculated_rook_attacks = all_rooks_attacks(occupancy, rooks_bitboard);
    //     assert_eq!(calculated_rook_attacks, expected);
    // }

    // #[test]
    // fn test_rook_attacks_corner_square() {
    //     let occupancy = 0; // No pieces on the board
    //     let square = 0; // a1
    //     let expected = 0x01010101010101FE; // Expected attacks
    //     // assert_eq!(rook_attacks(occupancy, square), expected);
    // }

    // #[test]
    // fn test_rook_attacks_full_blocked() {
    //     let occupancy = 0xFFFFFFFFFFFFFFFF; // Fully blocked board
    //     let square = 36; // e5
    //     let expected = 0x0010101010101010; // Expected attacks
    //     // assert_eq!(rook_attacks(occupancy, square), expected);
    // }
}