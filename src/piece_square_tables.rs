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
pub const MG_VALUE: [i32; 6] = [95, 295, 301, 413, 1013, 0];
// const MG_VALUE: [i32; 6] = [82, 339, 365, 477, 1025, 0];
/// Endgame piece values. See [`MG_VALUE`] on why these are public.
pub const EG_VALUE: [i32; 6] = [122, 329, 347, 587, 1042, 0];

const MG_PAWN_TABLE: [i32; 64] = [
       0,    0,    0,    0,    0,    0,    0,    0,
      64,   91,   29,   95,   48,   55,   -6,  -58,
       0,   -7,   18,   17,   40,   62,   62,   28,
     -25,  -18,  -14,   -9,   13,    7,    4,  -14,
     -42,  -28,  -25,   -7,   -8,  -12,  -11,  -29,
     -39,  -30,  -27,  -26,   -9,  -13,    7,  -19,
     -37,  -31,  -30,  -39,  -19,    2,   15,  -28,
       0,    0,    0,    0,    0,    0,    0,    0,
];

const EG_PAWN_TABLE: [i32; 64] = [
       0,    0,    0,    0,    0,    0,    0,    0,
      -7,  -22,   30,   -2,   27,  -24,   35,   34,
      27,   51,   31,   21,    9,    7,   30,   23,
      12,    6,   -7,  -20,  -19,  -15,   -4,   -6,
       6,    1,  -10,  -22,  -22,  -19,  -13,  -15,
      -5,   -4,  -14,   -5,  -11,  -11,  -18,  -20,
       0,    1,   -5,   -3,   -3,  -10,  -16,  -17,
       0,    0,    0,    0,    0,    0,    0,    0,
];

const MG_KNIGHT_TABLE: [i32; 64] = [
    -115, -148,  -72,    8,  -22,  -99,  -99, -115,
     -40,   -3,   49,   38,   34,   56,   31,   63,
     -32,   37,   58,   76,   81,   87,   43,   -7,
       6,    6,   20,   56,   28,   63,   17,   38,
     -12,    3,   20,   20,   28,   15,   35,    6,
     -17,   -8,    3,   12,   24,    7,   12,  -12,
     -40,  -27,   -2,    4,    0,   14,    0,   -5,
     -81,  -23,  -26,  -20,  -11,    0,  -17,  -24,
];

const EG_KNIGHT_TABLE: [i32; 64] = [
     -96,   -6,   -1,  -19,   17,   35,   17,  -84,
     -12,   15,  -10,    0,   -1,   13,    2,  -44,
      21,    1,    5,    2,   -2,   -2,   -3,    3,
      15,   26,   28,   27,   28,   19,   20,   -2,
      15,    8,   27,   24,   33,   27,    5,    0,
     -10,    7,    7,   16,   25,    2,   -1,    1,
      -9,    0,   -7,    2,    6,   -8,  -18,  -12,
     -29,  -39,   -5,   -6,  -11,  -15,  -13,  -53,
];

const MG_BISHOP_TABLE: [i32; 64] = [
      34,  -77,  -13, -112,  -71, -105,  -32,  -62,
     -10,   26,   -6,  -33,   43,    5,   16,  -14,
      -9,   24,   22,   28,   29,   44,   38,   11,
     -13,    3,   15,   32,   16,   28,    8,  -21,
     -13,  -11,   -3,   29,   21,    2,   -4,   -3,
       3,    1,    7,   12,   13,   11,    4,    9,
      10,    8,   20,    0,    7,   18,   24,    8,
     -13,    8,   -6,   -7,    6,   -7,   27,    7,
];

const EG_BISHOP_TABLE: [i32; 64] = [
       6,   16,    5,   16,    8,   11,   11,   -3,
      -7,   -3,  -10,   20,  -25,   -3,   -2,   -4,
       8,    2,   -4,   -9,   -7,   -5,   -7,    4,
      -6,    5,   10,   14,    8,    3,    3,   12,
       9,    9,    9,    3,    5,    0,   14,  -10,
      -1,    2,    6,    3,   12,    8,    4,    0,
     -17,  -16,  -11,    3,   -1,   -5,   -8,  -12,
     -18,    1,   -6,   -3,   -6,    0,  -34,  -29,
];

const MG_ROOK_TABLE: [i32; 64] = [
       2,   15,  -12,    4,   -4,   39,   91,   62,
      -4,   -8,    6,   33,   15,   43,   35,   16,
     -19,   15,   -8,   15,   30,   48,   73,   39,
     -32,    4,  -20,  -13,  -11,   -2,   15,   12,
     -40,  -46,  -21,  -16,  -11,  -28,   -6,  -10,
     -24,  -32,  -31,  -25,  -20,  -14,   18,   -1,
     -37,  -26,  -16,  -23,  -16,    0,    6,  -18,
      -9,  -11,   -7,   -2,    5,    2,    5,   -9,
];

const EG_ROOK_TABLE: [i32; 64] = [
      13,    6,   28,   12,   15,   17,   -7,   13,
      14,   20,   18,    5,    7,    6,    4,   17,
      16,    5,   12,    7,   -4,   -6,   -1,    0,
      15,    4,   13,    9,   -7,    2,   -8,  -10,
       9,   10,    9,    4,   -8,   -2,   -5,    0,
      -3,    3,   -3,   -8,   -4,  -19,  -31,  -28,
      -5,  -10,  -12,   -6,  -14,  -20,  -21,  -23,
       1,   -5,   -5,   -9,  -16,   -4,  -18,  -13,
];

const MG_QUEEN_TABLE: [i32; 64] = [
     -56,  -62,  -95,  -37,  -23,  -18,   84,   -3,
      -1,  -16,  -19,  -55,  -59,    4,   10,   41,
      10,   -6,   -4,  -30,   26,   39,   55,   14,
     -15,    2,  -19,  -17,   -5,   -3,    8,   22,
      -1,  -12,  -20,  -16,  -11,  -11,   10,   18,
       3,    3,    3,    0,   -2,   15,   16,   15,
      24,    2,   19,   22,   16,   29,   25,   40,
       7,   -4,    7,   20,   16,    9,   15,    6,
];

const EG_QUEEN_TABLE: [i32; 64] = [
      53,   18,   77,   39,   24,   32,  -32,   10,
      -1,    1,   57,   97,  117,   23,  -14,   16,
     -11,    0,   24,   65,   11,   15,  -23,   25,
      -1,   -3,   14,   36,   29,   43,   28,   10,
     -15,  -10,   27,   35,   32,   34,    3,   11,
     -25,  -23,  -11,   -9,   14,  -29,  -22,  -16,
     -73,  -26,  -53,  -52,  -28,  -53,  -77,  -94,
     -36,  -23,  -32,  -33,  -48,  -46,  -67,  -30,
];

const MG_KING_TABLE: [i32; 64] = [
     -15,   23,   23,  -98,  -47,   -6,   60,   68,
    -103,  -61,  -73,   31,   36,  -13,   80,   63,
    -122,   20,   -4,  -53,  -34,   77,   44,    4,
     -11,  -10, -105, -130, -104,  -60,  -75,  -93,
    -124,  -51,  -55, -109, -110,  -55,  -66, -169,
      17,    1,  -52,  -69,  -47,  -46,  -18,  -46,
      61,   16,   -6,  -31,  -34,  -23,   26,   25,
      51,   53,   32,  -64,   -9,  -34,   30,   35,
];

const EG_KING_TABLE: [i32; 64] = [
     -80,  -45,  -36,    7,   16,    2,   39,  -88,
       2,   33,   34,   27,   20,   50,   45,    1,
      30,   24,   39,   55,   72,   53,   54,   14,
       4,   38,   52,   63,   68,   56,   55,   21,
      10,   24,   35,   52,   58,   39,   30,   35,
     -30,    0,   20,   27,   28,   24,    3,    0,
     -36,  -17,   -3,    3,   11,    5,  -14,  -26,
     -95,  -57,  -44,  -18,  -37,  -26,  -52,  -81,
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
