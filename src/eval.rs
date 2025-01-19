use crate::{
    chessboard::Chessboard, display::display_board_string, move_gen::{black_legal_moves, white_legal_moves}
};

pub const QUEEN_VALUE: i64 = 9;
pub const ROOK_VALUE: i64 = 5;
pub const BISHOP_VALUE: i64 = 3;
pub const KNIGHT_VALUE: i64 = 3;
pub const PAWN_VALUE: i64 = 1;

pub fn evaluate(cb: &Chessboard) -> i64 {
    let material = (cb.white_queens.count_ones() as i64 - cb.black_queens.count_ones() as i64)
        * QUEEN_VALUE
        + (cb.white_rooks.count_ones() as i64 - cb.black_rooks.count_ones() as i64) * ROOK_VALUE
        + (cb.white_bishops.count_ones() as i64 - cb.black_bishops.count_ones() as i64)
            * BISHOP_VALUE
        + (cb.white_knights.count_ones() as i64 - cb.black_knights.count_ones() as i64)
            * KNIGHT_VALUE
        + (cb.white_pawns.count_ones() as i64 - cb.black_pawns.count_ones() as i64) * PAWN_VALUE;
    material
}