use crate::{
    chessboard::Chessboard,
    display::display_board_string,
    move_gen::{black_legal_moves, white_legal_moves},
    piece::{ColoredPiece, Piece},
    piece_square_tables::*,
};
use crate::piece_square_tables::eval;

pub fn evaluate(cb: &Chessboard) -> i32 {
    eval(&cb.piece_square, &EVAL_TABLES)
}
