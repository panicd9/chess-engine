// use crate::chessboard::*;

use move_gen_bishop::{black_bishops_legal_moves_into, white_bishops_legal_moves_into};
use move_gen_king::{black_king_legal_moves_into, white_king_legal_moves_into};
use move_gen_knight::{black_knights_legal_moves_into, white_knights_legal_moves_into};
use move_gen_pawn::{black_pawns_legal_moves_into, white_pawns_legal_moves_into};
use move_gen_queen::{black_queens_legal_moves_into, white_queens_legal_moves_into};
use move_gen_rook::{black_rooks_legal_moves_into, white_rooks_legal_moves_into};

use crate::{chessboard::{Chessboard, Color}, move_list::Move};
use legality::Legality;
use legality::{black_legality, white_legality};
use move_gen_king::{black_castling_into, king_attacks, white_castling_into};

pub mod move_gen_rook;
pub mod move_gen_pawn;
pub mod move_gen_knight;
pub mod move_gen_bishop;
pub mod move_gen_queen;
pub mod move_gen_king;
pub mod check_and_make_move;
pub mod legality;

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

/// Does this side have a legal move at all?
///
/// Asked only at a quiescence leaf that produced no captures, to tell a quiet
/// position from stalemate -- a stalemate is a draw whatever the evaluation
/// says. The king is tried first because `danger` answers for all of its moves
/// at once, and in almost every position it has somewhere legal to go. The
/// full generator is only reached when the king is boxed in, which is also the
/// only case where the answer might be "no".
pub fn has_any_legal_move(cb: &Chessboard, is_white: bool) -> bool {
    let legality = if is_white { white_legality(cb) } else { black_legality(cb) };
    has_any_legal_move_with(cb, is_white, &legality)
}

pub fn has_any_legal_move_with(cb: &Chessboard, is_white: bool, legality: &Legality) -> bool {
    let (king, own) = if is_white {
        (cb.white_king, cb.get_white_occupancy())
    } else {
        (cb.black_king, cb.get_black_occupancy())
    };

    if king_attacks(king) & !own & !legality.danger != 0 {
        return true;
    }

    // The king cannot move. Castling needs an empty, unattacked square next to
    // the king, so it would have been found above; everything else has to be
    // generated.
    let mut buf = Vec::with_capacity(32);
    if is_white {
        white_legal_moves_with(cb, &mut buf, legality);
    } else {
        black_legal_moves_with(cb, &mut buf, legality);
    }
    !buf.is_empty()
}

pub fn legal_moves(cb: &Chessboard) -> Vec<Move> {
    let side_to_move = cb.side_to_move;
    if side_to_move == Color::White {
        white_legal_moves(cb)
    } else {
        black_legal_moves(cb)
    }
}

pub fn white_legal_moves(cb: &Chessboard) -> Vec<Move> {
    let mut new_positions = Vec::with_capacity(64);
    white_legal_moves_into(cb, &mut new_positions);
    new_positions
}

pub fn white_legal_moves_into(cb: &Chessboard, out: &mut Vec<Move>) {
    white_legal_moves_with(cb, out, &white_legality(cb));
}

/// Every legal move, in the order the generators have always produced them:
/// pawns, knights, bishops, rooks, queens, king, castling. Move ordering
/// breaks ties by position in the list, so the order is part of how the search
/// behaves and is not to be rearranged casually.
pub fn white_legal_moves_with(cb: &Chessboard, out: &mut Vec<Move>, legality: &Legality) {
    let targets = !cb.get_white_occupancy();
    // Nothing but the king can answer two checks at once, so on a double check
    // the other five generators would run only to be masked down to nothing.
    if !legality.double_check() {
        white_pawns_legal_moves_into(cb, out, legality);
        white_knights_legal_moves_into(cb, out, targets, legality);
        white_bishops_legal_moves_into(cb, out, targets, legality);
        white_rooks_legal_moves_into(cb, out, targets, legality);
        white_queens_legal_moves_into(cb, out, targets, legality);
    }
    white_king_legal_moves_into(cb, out, targets, legality);
    white_castling_into(cb, out, legality);
}

/// The noisy stage: captures, en passant and every promotion -- the moves
/// scoring at or above the threshold quiescence retains, and the ones a cutoff
/// usually comes from.
pub fn white_captures_with(cb: &Chessboard, out: &mut Vec<Move>, legality: &Legality) {
    let targets = cb.get_black_occupancy();
    // Only the king can answer a double check; see `white_legal_moves_with`.
    if !legality.double_check() {
        move_gen_pawn::white_pawn_captures_into(cb, out, legality);
        white_knights_legal_moves_into(cb, out, targets, legality);
        white_bishops_legal_moves_into(cb, out, targets, legality);
        white_rooks_legal_moves_into(cb, out, targets, legality);
        white_queens_legal_moves_into(cb, out, targets, legality);
    }
    white_king_legal_moves_into(cb, out, targets, legality);
}

/// The quiet stage: everything the noisy stage does not produce. A promotion
/// push is noisy and belongs there; castling is quiet and belongs here. The
/// two together are exactly `white_legal_moves_with`, though not in that order --
/// `capture_and_quiet_stages_partition_the_full_generator` holds them to it.
pub fn white_quiets_with(cb: &Chessboard, out: &mut Vec<Move>, legality: &Legality) {
    let targets = !cb.get_occupancy();
    // Only the king can answer a double check; see `white_legal_moves_with`.
    if !legality.double_check() {
        move_gen_pawn::white_pawn_quiets_into(cb, out, legality);
        white_knights_legal_moves_into(cb, out, targets, legality);
        white_bishops_legal_moves_into(cb, out, targets, legality);
        white_rooks_legal_moves_into(cb, out, targets, legality);
        white_queens_legal_moves_into(cb, out, targets, legality);
    }
    white_king_legal_moves_into(cb, out, targets, legality);
    white_castling_into(cb, out, legality);
}

pub fn white_captures_into(cb: &Chessboard, out: &mut Vec<Move>) {
    white_captures_with(cb, out, &white_legality(cb));
}

pub fn black_legal_moves(cb: &Chessboard) -> Vec<Move> {
    let mut new_positions = Vec::with_capacity(64);
    black_legal_moves_into(cb, &mut new_positions);
    new_positions
}

pub fn black_legal_moves_into(cb: &Chessboard, out: &mut Vec<Move>) {
    black_legal_moves_with(cb, out, &black_legality(cb));
}

/// Every legal move, in the order the generators have always produced them:
/// pawns, knights, bishops, rooks, queens, king, castling. Move ordering
/// breaks ties by position in the list, so the order is part of how the search
/// behaves and is not to be rearranged casually.
pub fn black_legal_moves_with(cb: &Chessboard, out: &mut Vec<Move>, legality: &Legality) {
    let targets = !cb.get_black_occupancy();
    // Nothing but the king can answer two checks at once, so on a double check
    // the other five generators would run only to be masked down to nothing.
    if !legality.double_check() {
        black_pawns_legal_moves_into(cb, out, legality);
        black_knights_legal_moves_into(cb, out, targets, legality);
        black_bishops_legal_moves_into(cb, out, targets, legality);
        black_rooks_legal_moves_into(cb, out, targets, legality);
        black_queens_legal_moves_into(cb, out, targets, legality);
    }
    black_king_legal_moves_into(cb, out, targets, legality);
    black_castling_into(cb, out, legality);
}

/// The noisy stage: captures, en passant and every promotion -- the moves
/// scoring at or above the threshold quiescence retains, and the ones a cutoff
/// usually comes from.
pub fn black_captures_with(cb: &Chessboard, out: &mut Vec<Move>, legality: &Legality) {
    let targets = cb.get_white_occupancy();
    // Only the king can answer a double check; see `black_legal_moves_with`.
    if !legality.double_check() {
        move_gen_pawn::black_pawn_captures_into(cb, out, legality);
        black_knights_legal_moves_into(cb, out, targets, legality);
        black_bishops_legal_moves_into(cb, out, targets, legality);
        black_rooks_legal_moves_into(cb, out, targets, legality);
        black_queens_legal_moves_into(cb, out, targets, legality);
    }
    black_king_legal_moves_into(cb, out, targets, legality);
}

/// The quiet stage: everything the noisy stage does not produce. A promotion
/// push is noisy and belongs there; castling is quiet and belongs here. The
/// two together are exactly `black_legal_moves_with`, though not in that order --
/// `capture_and_quiet_stages_partition_the_full_generator` holds them to it.
pub fn black_quiets_with(cb: &Chessboard, out: &mut Vec<Move>, legality: &Legality) {
    let targets = !cb.get_occupancy();
    // Only the king can answer a double check; see `black_legal_moves_with`.
    if !legality.double_check() {
        move_gen_pawn::black_pawn_quiets_into(cb, out, legality);
        black_knights_legal_moves_into(cb, out, targets, legality);
        black_bishops_legal_moves_into(cb, out, targets, legality);
        black_rooks_legal_moves_into(cb, out, targets, legality);
        black_queens_legal_moves_into(cb, out, targets, legality);
    }
    black_king_legal_moves_into(cb, out, targets, legality);
    black_castling_into(cb, out, legality);
}

pub fn black_captures_into(cb: &Chessboard, out: &mut Vec<Move>) {
    black_captures_with(cb, out, &black_legality(cb));
}

