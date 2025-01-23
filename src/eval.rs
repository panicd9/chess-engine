use crate::{
    chessboard::Chessboard, display::display_board_string, move_gen::{black_legal_moves, white_legal_moves}, piece::{ColoredPiece, Piece}, piece_square_tables::{get_piece_at_square, BLACK_BISHOP_TABLE, BLACK_KING_MIDGAME_TABLE, BLACK_KNIGHT_TABLE, BLACK_PAWN_TABLE, BLACK_QUEEN_TABLE, BLACK_ROOK_TABLE, WHITE_BISHOP_TABLE, WHITE_KING_MIDGAME_TABLE, WHITE_KNIGHT_TABLE, WHITE_PAWN_TABLE, WHITE_QUEEN_TABLE, WHITE_ROOK_TABLE}
};

pub const QUEEN_VALUE: i64 = 900;
pub const ROOK_VALUE: i64 = 500;
pub const BISHOP_VALUE: i64 = 300;
pub const KNIGHT_VALUE: i64 = 300;
pub const PAWN_VALUE: i64 = 100;
pub const KING_VALUE: i64 = 20000;

pub fn evaluate(cb: &Chessboard) -> i64 {
    let material = 
        (cb.white_pawns.count_ones() as i64 - cb.black_pawns.count_ones() as i64) * PAWN_VALUE
        + (cb.white_knights.count_ones() as i64 - cb.black_knights.count_ones() as i64) * KNIGHT_VALUE
        + (cb.white_bishops.count_ones() as i64 - cb.black_bishops.count_ones() as i64) * BISHOP_VALUE
        + (cb.white_rooks.count_ones() as i64 - cb.black_rooks.count_ones() as i64) * ROOK_VALUE
        + (cb.white_queens.count_ones() as i64 - cb.black_queens.count_ones() as i64) * QUEEN_VALUE
        + (cb.white_king.count_ones() as i64 - cb.black_king.count_ones() as i64) * KING_VALUE;


    let mut piece_square_value = 0;
    for (index, piece) in cb.piece_square.iter().enumerate() {
        match piece {
            ColoredPiece::WhitePawn => piece_square_value += WHITE_PAWN_TABLE[index],
            ColoredPiece::WhiteKnight => piece_square_value += WHITE_KNIGHT_TABLE[index],
            ColoredPiece::WhiteRook => piece_square_value += WHITE_ROOK_TABLE[index],
            ColoredPiece::WhiteBishop => piece_square_value += WHITE_BISHOP_TABLE[index],
            ColoredPiece::WhiteQueen => piece_square_value += WHITE_QUEEN_TABLE[index],
            ColoredPiece::WhiteKing => piece_square_value += WHITE_KING_MIDGAME_TABLE[index],
            ColoredPiece::BlackPawn => piece_square_value -= BLACK_PAWN_TABLE[index],
            ColoredPiece::BlackKnight => piece_square_value -= BLACK_KNIGHT_TABLE[index],
            ColoredPiece::BlackRook => piece_square_value -= BLACK_ROOK_TABLE[index],
            ColoredPiece::BlackBishop => piece_square_value -= BLACK_BISHOP_TABLE[index],
            ColoredPiece::BlackQueen => piece_square_value -= BLACK_QUEEN_TABLE[index],
            ColoredPiece::BlackKing => piece_square_value -= BLACK_KING_MIDGAME_TABLE[index],
            ColoredPiece::Empty => {},
        }
    }

    material + piece_square_value
}