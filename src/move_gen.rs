// use crate::chessboard::*;

use move_gen_bishop::{black_bishops_legal_moves, black_bishops_pseudolegal_moves, white_bishops_legal_moves, white_bishops_pseudolegal_moves};
use move_gen_king::{black_king_legal_moves, black_king_pseudolegal_moves, white_king_legal_moves, white_king_pseudolegal_moves};
use move_gen_knight::{black_knights_legal_moves, black_knights_pseudolegal_moves, white_knights_legal_moves, white_knights_pseudolegal_moves};
use move_gen_pawn::{black_pawns_legal_moves, black_pawns_pseudolegal_moves, white_pawns_legal_moves, white_pawns_pseudolegal_moves};
use move_gen_queen::{black_queens_legal_moves, black_queens_pseudolegal_moves, white_queens_legal_moves, white_queens_pseudolegal_moves};
use move_gen_rook::{black_rooks_legal_moves, black_rooks_pseudolegal_moves, white_rooks_legal_moves, white_rooks_pseudolegal_moves};

use crate::chessboard::Chessboard;

pub mod move_gen_rook;
pub mod move_gen_pawn;
pub mod move_gen_knight;
pub mod move_gen_bishop;
pub mod move_gen_queen;
pub mod move_gen_king;
pub mod check_and_make_move;

// pub fn generate_rook_moves(rook: u64, occupied: u64) -> u64 {
//     let square = rook.trailing_zeros() as usize;

//     // Precompute rank and file masks
//     let (rank_mask, file_mask) = precompute_rook_masks(square);

//     // Isolate blocking pieces on the same rank and file
//     let blockers_rank = occupied & rank_mask;
//     let blockers_file = occupied & file_mask;

//     // Calculate attacks
//     let rank_attack = calculate_sliding_attacks(rook, blockers_rank, rank_mask);
//     let file_attack = calculate_sliding_attacks(rook, blockers_file, file_mask);

//     rank_attack | file_attack
// }

// pub(crate) const fn precompute_rook_masks(square: usize) -> (u64, u64) {
//     let rank_mask = RANK_MASKS[square / 8]; // Rank of the square
//     let file_mask = FILE_MASKS[square % 8]; // File of the square
//     (rank_mask, file_mask)
// }

// fn calculate_sliding_attacks(rook: u64, blockers: u64, mask: u64) -> u64 {
//     let forward = blockers & (mask & !(mask - rook));
//     let backward = blockers & (mask & (rook - 1));
//     let forward_attack = (forward - (rook << 1)) & mask;
//     let backward_attack = (rook - backward) & mask;

//     forward_attack | backward_attack
// }

pub fn white_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(100);

    new_positions.append(&mut white_pawns_pseudolegal_moves(cb));
    new_positions.append(&mut white_knights_pseudolegal_moves(cb));
    new_positions.append(&mut white_bishops_pseudolegal_moves(cb));
    new_positions.append(&mut white_rooks_pseudolegal_moves(cb));
    new_positions.append(&mut white_queens_pseudolegal_moves(cb));
    new_positions.append(&mut white_king_pseudolegal_moves(cb));

    new_positions
}

pub fn black_pseudolegal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(100);

    new_positions.append(&mut black_pawns_pseudolegal_moves(cb));
    new_positions.append(&mut black_knights_pseudolegal_moves(cb));
    new_positions.append(&mut black_bishops_pseudolegal_moves(cb));
    new_positions.append(&mut black_rooks_pseudolegal_moves(cb));
    new_positions.append(&mut black_queens_pseudolegal_moves(cb));
    new_positions.append(&mut black_king_pseudolegal_moves(cb));

    new_positions
}

pub fn white_legal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(50);

    new_positions.append(&mut white_pawns_legal_moves(cb));
    new_positions.append(&mut white_knights_legal_moves(cb));
    new_positions.append(&mut white_bishops_legal_moves(cb));
    new_positions.append(&mut white_rooks_legal_moves(cb));
    new_positions.append(&mut white_queens_legal_moves(cb));
    new_positions.append(&mut white_king_legal_moves(cb));

    new_positions
}

pub fn black_legal_moves(cb: &Chessboard) -> Vec<Chessboard> {
    let mut new_positions = Vec::with_capacity(50);

    new_positions.append(&mut black_pawns_legal_moves(cb));
    new_positions.append(&mut black_knights_legal_moves(cb));
    new_positions.append(&mut black_bishops_legal_moves(cb));
    new_positions.append(&mut black_rooks_legal_moves(cb));
    new_positions.append(&mut black_queens_legal_moves(cb));
    new_positions.append(&mut black_king_legal_moves(cb));
    
    new_positions
}