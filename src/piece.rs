use crate::chessboard::Color;

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,

    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum ColoredPiece {
    WhitePawn = 0,
    BlackPawn = 1,

    WhiteKnight = 2,
    BlackKnight = 3,

    WhiteBishop = 4,
    BlackBishop = 5,

    WhiteRook = 6,
    BlackRook = 7,

    WhiteQueen = 8,
    BlackQueen = 9,

    WhiteKing = 10,
    BlackKing = 11,

    Empty = 12,
}

impl ColoredPiece {
    pub fn to_piece(self) -> Piece {
        match self {
            ColoredPiece::WhitePawn | ColoredPiece::BlackPawn => Piece::Pawn,
            ColoredPiece::WhiteKnight | ColoredPiece::BlackKnight => Piece::Knight,
            ColoredPiece::WhiteBishop | ColoredPiece::BlackBishop => Piece::Bishop,
            ColoredPiece::WhiteRook | ColoredPiece::BlackRook => Piece::Rook,
            ColoredPiece::WhiteQueen | ColoredPiece::BlackQueen => Piece::Queen,
            ColoredPiece::WhiteKing | ColoredPiece::BlackKing => Piece::King,
            ColoredPiece::Empty => Piece::None,
        }
    }

    /// Returns the color of the piece.
    /// For `Empty`, returns `None`.
    pub fn color(self) -> Option<Color> {
        match self {
            ColoredPiece::Empty => None,
            _ => {
                // We encode white pieces with even numbers and black with odd.
                if (self as usize) % 2 == 0 {
                    Some(Color::White)
                } else {
                    Some(Color::Black)
                }
            }
        }
    }
}

pub enum PromotionPiece {
    Queen,
    Knight,
    Bishop,
    Rook,
}
