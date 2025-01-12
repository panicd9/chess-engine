// use crate::chessboard::*;

mod move_gen_pawn;
mod move_gen_rook;
mod move_gen_knight;
mod move_gen_bishop;
mod move_gen_queen;
mod move_gen_king;


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