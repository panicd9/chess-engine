// use crate::chessboard::*;

use move_gen_bishop::{black_bishops_legal_moves_into, white_bishops_legal_moves_into};
use move_gen_king::{black_king_legal_moves_into, white_king_legal_moves_into};
use move_gen_knight::{black_knights_legal_moves_into, white_knights_legal_moves_into};
use move_gen_pawn::{black_pawns_legal_moves_into, white_pawns_legal_moves_into};
use move_gen_queen::{black_queens_legal_moves_into, white_queens_legal_moves_into};
use move_gen_rook::{black_rooks_legal_moves_into, white_rooks_legal_moves_into};

use crate::{chessboard::{Chessboard, Color}, move_list::Move};
use move_gen_king::king_attacks;

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

/// Does this side have a legal move at all?
///
/// Asked only at a quiescence leaf that produced no captures, to tell a quiet
/// position from stalemate -- a stalemate is a draw whatever the evaluation
/// says. The king is tried first because it is the cheapest set to build (a
/// table lookup, no sliding attacks) and in almost every position it has
/// somewhere legal to go, so the answer is usually one make-and-test. The full
/// generator is only reached when the king is boxed in, which is also the only
/// case where the answer might be "no".
pub fn has_any_legal_move(cb: &Chessboard, is_white: bool) -> bool {
    let (king, own) = if is_white {
        (cb.white_king, cb.get_white_occupancy())
    } else {
        (cb.black_king, cb.get_black_occupancy())
    };

    let mut targets = king_attacks(king) & !own;
    while targets != 0 {
        let to = targets & targets.wrapping_neg();
        let moved = if is_white {
            cb.make_white_king_move(king, to)
        } else {
            cb.make_black_king_move(king, to)
        };
        let leaves_king_attacked = if is_white {
            moved.chessboard.is_white_king_under_attack()
        } else {
            moved.chessboard.is_black_king_under_attack()
        };
        if !leaves_king_attacked {
            return true;
        }
        targets &= targets - 1;
    }

    // The king cannot move. Castling needs an empty, unattacked square next to
    // the king, so it would have been found above; everything else has to be
    // generated.
    let mut buf = Vec::with_capacity(32);
    if is_white {
        white_legal_moves_into(cb, &mut buf);
    } else {
        black_legal_moves_into(cb, &mut buf);
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
    let targets = !cb.get_white_occupancy();
    white_pawns_legal_moves_into(cb, out);
    white_knights_legal_moves_into(cb, out, targets);
    white_bishops_legal_moves_into(cb, out, targets);
    white_rooks_legal_moves_into(cb, out, targets);
    white_queens_legal_moves_into(cb, out, targets);
    white_king_legal_moves_into(cb, out, targets);
}

/// PROTOTYPE: captures only for the non-pawn pieces; pawns still generate
/// everything, so this is a lower bound on what a real capture generator saves.
pub fn white_captures_into(cb: &Chessboard, out: &mut Vec<Move>) {
    let targets = cb.get_black_occupancy();
    move_gen_pawn::white_pawn_captures_into(cb, out);
    white_knights_legal_moves_into(cb, out, targets);
    white_bishops_legal_moves_into(cb, out, targets);
    white_rooks_legal_moves_into(cb, out, targets);
    white_queens_legal_moves_into(cb, out, targets);
    white_king_legal_moves_into(cb, out, targets);
}

pub fn black_legal_moves(cb: &Chessboard) -> Vec<Move> {
    let mut new_positions = Vec::with_capacity(64);
    black_legal_moves_into(cb, &mut new_positions);
    new_positions
}

pub fn black_legal_moves_into(cb: &Chessboard, out: &mut Vec<Move>) {
    let targets = !cb.get_black_occupancy();
    black_pawns_legal_moves_into(cb, out);
    black_knights_legal_moves_into(cb, out, targets);
    black_bishops_legal_moves_into(cb, out, targets);
    black_rooks_legal_moves_into(cb, out, targets);
    black_queens_legal_moves_into(cb, out, targets);
    black_king_legal_moves_into(cb, out, targets);
}

pub fn black_captures_into(cb: &Chessboard, out: &mut Vec<Move>) {
    let targets = cb.get_white_occupancy();
    move_gen_pawn::black_pawn_captures_into(cb, out);
    black_knights_legal_moves_into(cb, out, targets);
    black_bishops_legal_moves_into(cb, out, targets);
    black_rooks_legal_moves_into(cb, out, targets);
    black_queens_legal_moves_into(cb, out, targets);
    black_king_legal_moves_into(cb, out, targets);
}