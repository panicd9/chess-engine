#[derive(Debug, Clone, Copy)]
pub enum Piece {
    Pawn,
    Knight,
    Rook,
    Bishop,
    Queen,
    King,
}

pub enum ColoredPiece {
    White(Piece),
    Black(Piece),
}

pub enum PromotionPiece {
    Queen,
    Knight,
    Bishop,
    Rook,
}