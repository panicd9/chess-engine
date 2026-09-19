use crate::{chessboard::{Chessboard, Color, SingletonBitboard}, piece::ColoredPiece, utils::flip};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref EVAL_TABLES: EvalTables = EvalTables::new();
}

/// Midgame piece values for Pawn, Knight, Bishop, Rook, Queen, King.
///
/// **This is the engine's single source of truth for what a piece is worth.**
/// The tables below are these values plus a per-square offset, so anything
/// elsewhere that needs a piece value -- delta pruning's queen, the drawish
/// material test -- must read it from here rather than write its own copy. Two
/// hand-written copies had already drifted from it before this was made public.
pub const MG_VALUE: [i32; 6] = [101, 299, 318, 427, 1019, 0];
// const MG_VALUE: [i32; 6] = [82, 339, 365, 477, 1025, 0];
/// Endgame piece values. See [`MG_VALUE`] on why these are public.
pub const EG_VALUE: [i32; 6] = [137, 319, 337, 579, 1020, 0];

const MG_PAWN_TABLE: [i32; 64] = [
       0,    0,    0,    0,    0,    0,    0,    0,
      85,  101,   74,  107,   89,   93,    8,   -1,
      -9,   -7,   27,   39,   39,   59,   51,    6,
     -38,  -24,  -17,  -13,    4,    0,   -3,  -23,
     -55,  -33,  -29,  -15,  -14,  -17,  -17,  -36,
     -53,  -35,  -31,  -29,  -14,  -19,    1,  -30,
     -51,  -35,  -34,  -42,  -23,   -4,   11,  -36,
       0,    0,    0,    0,    0,    0,    0,    0,
];

const EG_PAWN_TABLE: [i32; 64] = [
       0,    0,    0,    0,    0,    0,    0,    0,
      45,   27,   65,   38,   42,   18,   72,   74,
      42,   62,   41,   27,   26,   12,   39,   34,
       6,    0,  -15,  -25,  -29,  -28,  -15,  -15,
      -8,  -14,  -27,  -36,  -36,  -33,  -27,  -30,
     -17,  -17,  -28,  -19,  -26,  -28,  -28,  -35,
     -13,  -11,  -20,  -12,  -12,  -22,  -28,  -31,
       0,    0,    0,    0,    0,    0,    0,    0,
];

const MG_KNIGHT_TABLE: [i32; 64] = [
    -168, -133,  -41,   15,    4,  -81, -105, -102,
     -23,    0,   11,   27,   38,   52,   -7,   35,
      -9,   28,   50,   61,   70,   87,   51,   22,
      -1,   11,   28,   54,   30,   59,   19,   42,
     -13,    4,   20,   22,   29,   22,   30,    7,
     -20,   -9,    3,   13,   28,   10,   16,  -11,
     -38,  -24,  -10,    4,    2,    6,   -4,   -7,
     -72,  -23,  -34,  -21,  -15,   -1,  -21,  -41,
];

const EG_KNIGHT_TABLE: [i32; 64] = [
     -37,  -11,   -1,  -14,   11,   -1,   -1,  -85,
     -10,    1,    4,    7,   -1,   -2,   -4,  -31,
      11,    3,    9,   11,    9,   -4,   -8,  -15,
       7,   23,   28,   27,   30,   20,   19,    1,
      15,   15,   29,   25,   33,   22,   11,    1,
     -13,    5,    7,   17,   18,    1,   -4,   -4,
      -9,    0,   -1,    2,    5,    3,  -21,  -12,
     -39,  -38,  -11,   -4,   -7,  -17,  -14,  -28,
];

const MG_BISHOP_TABLE: [i32; 64] = [
     -13,  -61,  -30,  -90,  -59,  -75,  -39,  -59,
      -4,   -1,  -11,  -26,   -5,    1,   16,  -15,
      -3,   17,   21,   28,   31,   49,   51,   27,
     -13,    7,   12,   39,   24,   33,    7,   -3,
     -15,   -2,    1,   27,   21,    3,   -2,    4,
       0,    6,   11,    8,   13,    8,    5,   16,
       8,    7,   18,   -2,    5,   17,   23,    8,
     -14,   10,   -7,  -10,   -9,  -10,   15,    2,
];

const EG_BISHOP_TABLE: [i32; 64] = [
       4,   10,    7,   18,   10,   12,   14,   -5,
     -16,    0,   -3,   12,   -4,    2,   -2,  -12,
      11,    2,    1,   -3,    1,    0,   -2,    2,
      -4,    7,    9,   12,    5,    5,   11,    3,
      -2,   10,   13,    0,    9,    9,   10,  -19,
       1,    2,    6,    6,   11,    9,   -1,  -11,
     -11,  -11,  -15,    2,    0,   -4,   -3,  -18,
     -19,   -3,  -12,   -5,   -7,   -2,  -22,  -30,
];

const MG_ROOK_TABLE: [i32; 64] = [
      -4,   25,   -9,    0,   27,   30,   58,   59,
      -4,   -3,   12,   42,   16,   48,   32,   50,
     -10,    7,    1,    6,   26,   46,   97,   44,
     -27,   -2,  -15,   -9,   -5,    8,   20,   14,
     -40,  -37,  -25,  -23,  -17,  -30,   -6,  -18,
     -36,  -36,  -29,  -26,  -20,  -15,   16,  -11,
     -44,  -31,  -20,  -20,  -13,   -7,   10,  -27,
     -16,  -17,  -10,   -3,    2,   -4,    2,  -14,
];

const EG_ROOK_TABLE: [i32; 64] = [
      16,    8,   26,   16,   11,   13,    3,    5,
      14,   19,   22,    6,   12,    5,    2,    0,
      13,   12,    9,    8,   -1,  -10,  -14,   -5,
      15,    4,   19,    8,   -5,   -5,   -8,   -9,
       7,    9,   10,    6,    2,    6,   -3,   -1,
      -1,    3,   -6,   -4,   -6,  -16,  -31,  -25,
      -7,   -8,   -5,   -7,  -17,  -20,  -28,  -18,
      -2,   -4,   -3,   -7,  -14,   -6,  -16,  -15,
];

const MG_QUEEN_TABLE: [i32; 64] = [
     -64,  -30,  -52,  -17,  -33,  -25,   86,   13,
      -4,  -21,  -17,  -42,  -33,   10,   14,   52,
       2,  -17,   -5,  -14,   10,   32,   60,   29,
     -12,  -11,  -16,  -19,   -7,    4,   15,   21,
      -5,  -15,  -21,  -14,  -13,   -9,    6,   13,
      -4,   -1,   -7,   -8,   -7,    7,   18,   12,
       1,   -2,   11,   16,   11,   23,   23,   37,
       1,   -6,    2,   13,    8,   -5,   16,    3,
];

const EG_QUEEN_TABLE: [i32; 64] = [
      45,    1,   60,   34,   46,   35,  -51,    6,
       0,    6,   39,   72,   79,   11,   -2,   -1,
      -5,   14,   30,   48,   29,    7,  -34,    5,
      10,    6,   22,   35,   33,   29,   23,  -10,
     -10,   15,   20,   34,   36,   24,    2,    4,
     -20,  -14,    1,    2,   17,   -9,  -23,  -11,
     -31,  -34,  -43,  -35,  -24,  -55,  -63,  -97,
     -32,  -33,  -38,  -27,  -39,  -35,  -79,  -27,
];

const MG_KING_TABLE: [i32; 64] = [
     -18,    2,    9,  -83,  -65,    5,   38,   52,
     -80,  -31, -102,    9,   29,   -1,   74,   56,
    -140,    3,  -42,  -55,  -25,   57,   63,   -2,
     -38,   -7,  -81, -148, -137,  -93,  -97, -130,
     -83,  -45, -110, -120, -133,  -75,  -84, -139,
     -32,   -3,  -55,  -79,  -71,  -55,  -20,  -63,
      54,   11,   -5,  -38,  -37,  -24,   24,   31,
      44,   59,   35,  -63,   -4,  -35,   38,   40,
];

const EG_KING_TABLE: [i32; 64] = [
     -83,  -47,  -25,   12,   -7,    3,   21, -104,
      -4,   22,   39,   27,   24,   42,   34,   11,
      27,   23,   48,   57,   63,   57,   45,   19,
       8,   32,   49,   65,   67,   64,   56,   33,
       0,   17,   46,   56,   59,   44,   34,   26,
     -18,    0,   18,   32,   32,   25,    6,    0,
     -41,  -16,   -5,    3,    9,    3,  -13,  -30,
     -84,  -59,  -43,  -22,  -41,  -26,  -52,  -82,
];

//
// Group the piece–square tables into arrays for easier access.
// The index into these arrays corresponds to the piece type (0 for Pawn, …, 5 for King).
//
pub const MG_PESTO: [&[i32; 64]; 6] = [
    &MG_PAWN_TABLE,
    &MG_KNIGHT_TABLE,
    &MG_BISHOP_TABLE,
    &MG_ROOK_TABLE,
    &MG_QUEEN_TABLE,
    &MG_KING_TABLE,
];

pub const EG_PESTO: [&[i32; 64]; 6] = [
    &EG_PAWN_TABLE,
    &EG_KNIGHT_TABLE,
    &EG_BISHOP_TABLE,
    &EG_ROOK_TABLE,
    &EG_QUEEN_TABLE,
    &EG_KING_TABLE,
];

/// Game phase increments for each piece code (using the order of our evaluation table).
///
/// We use 12 entries (white then black) corresponding to:
/// WhitePawn, BlackPawn, WhiteKnight, BlackKnight, …, WhiteKing, BlackKing.
const GAMEPHASE_INC: [i32; 12] = [
    0, 0, // Pawn
    1, 1, // Knight
    1, 1, // Bishop
    2, 2, // Rook
    4, 4, // Queen
    0, 0, // King
];

/// Evaluation tables for midgame and endgame scores.
/// The arrays are indexed by the piece code (0..11) and then by square (0..63).
pub struct EvalTables {
    mg: [[i32; 64]; 12],
    eg: [[i32; 64]; 12],
}

impl EvalTables {
    fn new() -> Self {
        let mut mg = [[0; 64]; 12];
        let mut eg = [[0; 64]; 12];

        // For each piece type (Pawn .. King, i.e. indices 0..5)
        for piece in 0..6 {
            // For white, the index is 2*piece; for black, 2*piece+1.
            let white_idx = 2 * piece;
            let black_idx = 2 * piece + 1;
            for sq in 0..64 {
                mg[white_idx][sq] = MG_VALUE[piece] + MG_PESTO[piece][flip(sq)];
                eg[white_idx][sq] = EG_VALUE[piece] + EG_PESTO[piece][flip(sq)];

                mg[black_idx][sq] = MG_VALUE[piece] + MG_PESTO[piece][sq];
                eg[black_idx][sq] = EG_VALUE[piece] + EG_PESTO[piece][sq];
            }
        }

        Self { mg, eg }
    }
}

/// Evaluate the board using a tapered evaluation scheme.
///
/// * `board` is a 64-element array of `ColoredPiece`.
/// * `side_to_move` is the color for which we want the evaluation.
///
/// The function accumulates midgame and endgame score differences (adding for pieces
/// belonging to `side_to_move` and subtracting for the opponent) and interpolates between
/// these scores based on the “game phase.”
pub fn eval(board: &[ColoredPiece; 64], tables: &EvalTables) -> i32 {
    let (mg_diff, eg_diff, phase) = board.iter().enumerate().fold(
        (0, 0, 0),
        |(mg_acc, eg_acc, phase_acc), (sq, &cp)| {
            if cp == ColoredPiece::Empty {
                (mg_acc, eg_acc, phase_acc)
            } else {
                let idx = cp as usize; // Directly use the encoded value.
                let sign = match cp.color() {
                    Some(Color::White) => 1,
                    Some(Color::Black) => -1,
                    None => 0,
                };
                (
                    mg_acc + sign * tables.mg[idx][sq],
                    eg_acc + sign * tables.eg[idx][sq],
                    phase_acc + GAMEPHASE_INC[idx],
                )
            }
        },
    );

    let phase = phase.min(24); // Cap the phase to 24.
    (mg_diff * phase + eg_diff * (24 - phase)) / 24
}
// // Piece-square tables for positional evaluation
// pub const WHITE_PAWN_TABLE: [i64; 64] = [
//     0,   0,   0,   0,   0,   0,   0,   0,
//     5,  10,  10, -20, -20,  10,  10,   5,
//     5,  -5, -10,   0,   0, -10,  -5,   5,
//     0,   0,   0,  20,  20,   0,   0,   0,
//     5,   5,  10,  25,  25,  10,   5,   5,
//     10,  10,  20,  30,  30,  20,  10,  10,
//     50,  50,  50,  50,  50,  50,  50,  50,
//     0,   0,   0,   0,   0,   0,   0,   0,
// ];

// pub const BLACK_PAWN_TABLE: [i64; 64] = [
//     0,   0,   0,   0,   0,   0,   0,   0, 
//     50,  50,  50,  50,  50,  50,  50,  50,
//     10,  10,  20,  30,  30,  20,  10,  10,
//     5,   5,  10,  25,  25,  10,   5,   5,
//     0,   0,   0,  20,  20,   0,   0,   0,
//     5,  -5, -10,   0,   0, -10,  -5,   5,
//     5,  10,  10, -20, -20,  10,  10,   5,
//     0,   0,   0,   0,   0,   0,   0,   0,
// ];

// pub const WHITE_KNIGHT_TABLE: [i64; 64] = [
//     -50, -40, -30, -30, -30, -30, -40, -50,
//     -40, -20,   0,   5,   5,   0, -20, -40,
//     -30,   0,  10,  15,  15,  10,   0, -30,
//     -30,   5,  15,  20,  20,  15,   5, -30,
//     -30,   0,  15,  20,  20,  15,   0, -30,
//     -30,   5,  10,  15,  15,  10,   5, -30,
//     -40, -20,   0,   0,   0,   0, -20, -40,
//     -50, -40, -30, -30, -30, -30, -40, -50,
// ];

// pub const BLACK_KNIGHT_TABLE: [i64; 64] = [
//     -50, -40, -30, -30, -30, -30, -40, -50,
//     -40, -20,   0,   0,   0,   0, -20, -40,
//     -30,   0,  10,  15,  15,  10,   0, -30,
//     -30,   5,  15,  20,  20,  15,   5, -30,
//     -30,   0,  15,  20,  20,  15,   0, -30,
//     -30,   5,  10,  15,  15,  10,   5, -30,
//     -40, -20,   0,   5,   5,   0, -20, -40,
//     -50, -40, -30, -30, -30, -30, -40, -50,
// ];

// pub const WHITE_BISHOP_TABLE: [i64; 64] = [
//     -20, -10, -10, -10, -10, -10, -10, -20,
//     -10,   5,   0,   0,   0,   0,   5, -10,
//     -10,  10,  10,  10,  10,  10,  10, -10,
//     -10,   0,  10,  10,  10,  10,   0, -10,
//     -10,   5,   5,  10,  10,   5,   5, -10,
//     -10,   0,   5,  10,  10,   5,   0, -10,
//     -10,   0,   0,   0,   0,   0,   0, -10,
//     -20, -10, -10, -10, -10, -10, -10, -20,
// ];

// pub const BLACK_BISHOP_TABLE: [i64; 64] = [
//     -20, -10, -10, -10, -10, -10, -10, -20,
//     -10,   0,   0,   0,   0,   0,   0, -10,
//     -10,   0,   5,  10,  10,   5,   0, -10,
//     -10,   5,   5,  10,  10,   5,   5, -10,
//     -10,   0,  10,  10,  10,  10,   0, -10,
//     -10,  10,  10,  10,  10,  10,  10, -10,
//     -10,   5,   0,   0,   0,   0,   5, -10,
//     -20, -10, -10, -10, -10, -10, -10, -20,
// ];

// pub const WHITE_ROOK_TABLE: [i64; 64] = [
//      0,  0,  0,  5,  5,  0,  0,  0,
//      5, 10, 10, 10, 10, 10, 10,  5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//      0,  0,  0,  5,  5,  0,  0,  0,
// ];

// pub const BLACK_ROOK_TABLE: [i64; 64] = [
//      0,  0,  0,  0,  0,  0,  0,  0,
//      5, 10, 10, 10, 10, 10, 10,  5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//     -5,  0,  0,  0,  0,  0,  0, -5,
//     0,  0,  0,  5,  5,  0,  0,  0,
// ];

// pub const WHITE_QUEEN_TABLE: [i64; 64] = [
//     -20,-10,-10, -5, -5,-10,-10,-20,
//     -10,  0,  5,  0,  0,  0,  0,-10,
//     -10,  5,  5,  5,  5,  5,  0,-10,
//      0,  0,  5,  5,  5,  5,  0, -5,
//     -5,  0,  5,  5,  5,  5,  0, -5,
//     -10,  0,  5,  5,  5,  5,  0,-10,
//     -10,  0,  0,  0,  0,  0,  0,-10,
//     -20,-10,-10, -5, -5,-10,-10,-20,
// ];


// pub const BLACK_QUEEN_TABLE: [i64; 64] = [
//     -20,-10,-10, -5, -5,-10,-10,-20,
//     -10,  0,  0,  0,  0,  0,  0,-10,
//     -10,  0,  5,  5,  5,  5,  0,-10,
//     -5,  0,  5,  5,  5,  5,  0, -5,
//      0,  0,  5,  5,  5,  5,  0, -5,
//     -10,  5,  5,  5,  5,  5,  0,-10,
//     -10,  0,  5,  0,  0,  0,  0,-10,
//     -20,-10,-10, -5, -5,-10,-10,-20
// ];

// pub const WHITE_KING_MIDGAME_TABLE: [i64; 64] = [
//     20, 30, 10,  0,  0, 10, 30, 20,
//     20, 20,  0,  0,  0,  0, 20, 20,
//     -10,-20,-20,-20,-20,-20,-20,-10,
//     -20,-30,-30,-40,-40,-30,-30,-20,
//     -30,-40,-40,-50,-50,-40,-40,-30,
//     -30,-40,-40,-50,-50,-40,-40,-30,
//     -30,-40,-40,-50,-50,-40,-40,-30,
//     -30,-40,-40,-50,-50,-40,-40,-30,
// ];

// pub const BLACK_KING_MIDGAME_TABLE: [i64; 64] = [
//     -30,-40,-40,-50,-50,-40,-40,-30,
//     -30,-40,-40,-50,-50,-40,-40,-30,
//     -30,-40,-40,-50,-50,-40,-40,-30,
//     -30,-40,-40,-50,-50,-40,-40,-30,
//     -20,-30,-30,-40,-40,-30,-30,-20,
//     -10,-20,-20,-20,-20,-20,-20,-10,
//      20, 20,  0,  0,  0,  0, 20, 20,
//      20, 30, 10,  0,  0, 10, 30, 20
// ];

// pub const WHITE_KING_ENDGAME_TABLE: [i64; 64] = [
//     -50,-30,-30,-30,-30,-30,-30,-50,
//     -30,-30,  0,  0,  0,  0,-30,-30,
//     -30,-10, 20, 30, 30, 20,-10,-30,
//     -30,-10, 30, 40, 40, 30,-10,-30,
//     -30,-10, 30, 40, 40, 30,-10,-30,
//     -30,-10, 20, 30, 30, 20,-10,-30,
//     -30,-20,-10,  0,  0,-10,-20,-30,
//     -50,-40,-30,-20,-20,-30,-40,-50,
// ];

// pub const BLACK_KING_ENDGAME_TABLE: [i64; 64] = [
//     -50,-40,-30,-20,-20,-30,-40,-50,
//     -30,-20,-10,  0,  0,-10,-20,-30,
//     -30,-10, 20, 30, 30, 20,-10,-30,
//     -30,-10, 30, 40, 40, 30,-10,-30,
//     -30,-10, 30, 40, 40, 30,-10,-30,
//     -30,-10, 20, 30, 30, 20,-10,-30,
//     -30,-30,  0,  0,  0,  0,-30,-30,
//     -50,-30,-30,-30,-30,-30,-30,-50
// ];

// pub fn get_piece_at_square(cb: &Chessboard, square_bitboard: SingletonBitboard) -> Option<ColoredPiece> {
//     // Define all piece mappings in a compact and maintainable structure
//     let piece_mappings = [
//         (cb.white_pawns, ColoredPiece::WhitePawn),
//         (cb.white_knights, ColoredPiece::WhiteKnight),
//         (cb.white_rooks, ColoredPiece::WhiteRook),
//         (cb.white_bishops, ColoredPiece::WhiteBishop),
//         (cb.white_queens, ColoredPiece::WhiteQueen),
//         (cb.white_king, ColoredPiece::WhiteKing),
//         (cb.black_pawns, ColoredPiece::BlackPawn),
//         (cb.black_knights, ColoredPiece::BlackKnight),
//         (cb.black_rooks, ColoredPiece::BlackRook),
//         (cb.black_bishops, ColoredPiece::BlackBishop),
//         (cb.black_queens, ColoredPiece::BlackQueen),
//         (cb.black_king, ColoredPiece::BlackKing),
//     ];

//     // Iterate over the mappings and find the first match
//     for (bitboard, piece) in piece_mappings {
//         if bitboard & square_bitboard != 0 {
//             return Some(piece);
//         }
//     }

//     // If no piece matches, return None
//     None
// }

// // Base piece values
// pub const PAWN_VALUE: i64   = 100;
// pub const KNIGHT_VALUE: i64 = 300;
// pub const BISHOP_VALUE: i64 = 300;
// pub const ROOK_VALUE: i64   = 500;
// pub const QUEEN_VALUE: i64  = 900;
// pub const KING_VALUE: i64   = 20000;

// //////////////////////////////////////////////////////////////
// // A helper to add an offset to every element of a table
// //////////////////////////////////////////////////////////////

// // The `const fn` below creates a new array with the given offset added.
// const fn add_offset<const N: usize>(table: [i64; N], offset: i64) -> [i64; N] {
//     let mut new_table = [0; N];
//     let mut i = 0;
//     while i < N {
//         new_table[i] = table[i] + offset;
//         i += 1;
//     }
//     new_table
// }

// // Now, you can define new constant arrays that include the base piece value.
// // For example, for pawns:

// pub const WHITE_PAWN_TABLE_WITH_VALUE: [i64; 64] = add_offset(WHITE_PAWN_TABLE, PAWN_VALUE);
// pub const BLACK_PAWN_TABLE_WITH_VALUE: [i64; 64] = add_offset(BLACK_PAWN_TABLE, PAWN_VALUE);

// // Similarly, for knights and bishops:
// pub const WHITE_KNIGHT_TABLE_WITH_VALUE: [i64; 64] = add_offset(WHITE_KNIGHT_TABLE, KNIGHT_VALUE);
// pub const BLACK_KNIGHT_TABLE_WITH_VALUE: [i64; 64] = add_offset(BLACK_KNIGHT_TABLE, KNIGHT_VALUE);

// pub const WHITE_BISHOP_TABLE_WITH_VALUE: [i64; 64] = add_offset(WHITE_BISHOP_TABLE, BISHOP_VALUE);
// pub const BLACK_BISHOP_TABLE_WITH_VALUE: [i64; 64] = add_offset(BLACK_BISHOP_TABLE, BISHOP_VALUE);

// // For rooks:
// pub const WHITE_ROOK_TABLE_WITH_VALUE: [i64; 64] = add_offset(WHITE_ROOK_TABLE, ROOK_VALUE);
// pub const BLACK_ROOK_TABLE_WITH_VALUE: [i64; 64] = add_offset(BLACK_ROOK_TABLE, ROOK_VALUE);

// // For queens:
// pub const WHITE_QUEEN_TABLE_WITH_VALUE: [i64; 64] = add_offset(WHITE_QUEEN_TABLE, QUEEN_VALUE);
// pub const BLACK_QUEEN_TABLE_WITH_VALUE: [i64; 64] = add_offset(BLACK_QUEEN_TABLE, QUEEN_VALUE);

// // For kings, you might have both midgame and endgame tables:
// pub const WHITE_KING_MIDGAME_TABLE_WITH_VALUE: [i64; 64] =
//     add_offset(WHITE_KING_MIDGAME_TABLE, KING_VALUE);
// pub const BLACK_KING_MIDGAME_TABLE_WITH_VALUE: [i64; 64] =
//     add_offset(BLACK_KING_MIDGAME_TABLE, KING_VALUE);

// pub const WHITE_KING_ENDGAME_TABLE_WITH_VALUE: [i64; 64] =
//     add_offset(WHITE_KING_ENDGAME_TABLE, KING_VALUE);
// pub const BLACK_KING_ENDGAME_TABLE_WITH_VALUE: [i64; 64] =
//     add_offset(BLACK_KING_ENDGAME_TABLE, KING_VALUE);
