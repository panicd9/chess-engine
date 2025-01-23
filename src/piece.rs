#[derive(Debug, Clone, Copy)]
pub enum Piece {
    Pawn,
    Knight,
    Rook,
    Bishop,
    Queen,
    King,
}

#[derive(Debug, Clone, Copy)]
pub enum ColoredPiece {
    WhitePawn,
    WhiteKnight,
    WhiteRook,
    WhiteBishop,
    WhiteQueen,
    WhiteKing,

    BlackPawn,
    BlackKnight,
    BlackRook,
    BlackBishop,
    BlackQueen,
    BlackKing,

    Empty,
}

pub enum PromotionPiece {
    Queen,
    Knight,
    Bishop,
    Rook,
}