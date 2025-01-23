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
    for square in 0..64 {
        let square_bitboard = 1 << square;
        let piece = get_piece_at_square(cb, square_bitboard);
        if let Some(piece) = piece {
            match piece {
                ColoredPiece::White(piece) => match piece {
                    Piece::Pawn => piece_square_value += WHITE_PAWN_TABLE[square],
                    Piece::Knight => piece_square_value += WHITE_KNIGHT_TABLE[square],
                    Piece::Bishop => piece_square_value += WHITE_BISHOP_TABLE[square],
                    Piece::Rook => piece_square_value += WHITE_ROOK_TABLE[square],
                    Piece::Queen => piece_square_value += WHITE_QUEEN_TABLE[square],
                    Piece::King => piece_square_value += WHITE_KING_MIDGAME_TABLE[square],
                },
                ColoredPiece::Black(piece) => match piece {
                    Piece::Pawn => piece_square_value -= BLACK_PAWN_TABLE[square],
                    Piece::Knight => piece_square_value -= BLACK_KNIGHT_TABLE[square],
                    Piece::Bishop => piece_square_value -= BLACK_BISHOP_TABLE[square],
                    Piece::Rook => piece_square_value -= BLACK_ROOK_TABLE[square],
                    Piece::Queen => piece_square_value -= BLACK_QUEEN_TABLE[square],
                    Piece::King => piece_square_value -= BLACK_KING_MIDGAME_TABLE[square],
                }
            }
        }
    }

    material + piece_square_value
}