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
pub const MG_VALUE: [i32; 6] = [94, 296, 306, 418, 1017, 0];
// const MG_VALUE: [i32; 6] = [82, 339, 365, 477, 1025, 0];
/// Endgame piece values. See [`MG_VALUE`] on why these are public.
pub const EG_VALUE: [i32; 6] = [125, 325, 343, 583, 1036, 0];

const MG_PAWN_TABLE: [i32; 64] = [
       0,    0,    0,    0,    0,    0,    0,    0,
      62,   80,   37,   88,   57,   57,  -17,  -42,
       0,   -8,   15,   19,   41,   61,   61,   27,
     -25,  -19,  -15,  -10,   11,    6,    3,  -13,
     -41,  -28,  -26,   -7,   -7,  -11,  -11,  -28,
     -39,  -31,  -26,  -26,   -9,  -12,    8,  -19,
     -38,  -31,  -30,  -38,  -18,    4,   16,  -27,
       0,    0,    0,    0,    0,    0,    0,    0,
];

const EG_PAWN_TABLE: [i32; 64] = [
       0,    0,    0,    0,    0,    0,    0,    0,
       9,   -7,   32,    7,   24,  -19,   43,   39,
      32,   56,   38,   27,   13,    9,   35,   26,
      11,    6,   -7,  -20,  -20,  -17,   -5,   -6,
       3,   -2,  -14,  -26,  -26,  -23,  -17,  -19,
      -8,   -6,  -17,   -7,  -14,  -14,  -21,  -24,
      -2,   -1,   -7,   -5,   -4,  -12,  -18,  -20,
       0,    0,    0,    0,    0,    0,    0,    0,
];

const MG_KNIGHT_TABLE: [i32; 64] = [
    -128, -143,  -67,    9,  -17,  -88,  -97, -110,
     -46,  -15,   31,   26,   33,   41,   28,   58,
     -34,   31,   49,   69,   64,   87,   39,    1,
       6,    9,   22,   57,   30,   66,   22,   41,
      -9,    5,   23,   23,   30,   18,   38,    8,
     -15,   -5,    6,   14,   27,   10,   15,   -9,
     -38,  -24,   -1,    6,    2,   16,    2,   -2,
     -81,  -21,  -25,  -18,   -8,    1,  -15,  -22,
];

const EG_KNIGHT_TABLE: [i32; 64] = [
     -82,   -6,   -4,  -20,   16,   31,   17,  -84,
     -10,   20,   -8,    3,    0,   15,    4,  -42,
      21,    4,    8,    4,    4,   -1,    0,    2,
      14,   25,   27,   25,   28,   18,   20,   -2,
      15,    8,   27,   24,   33,   28,    6,    0,
     -10,    8,    6,   16,   25,    2,    0,    1,
      -9,   -1,   -7,    2,    6,   -8,  -19,  -11,
     -29,  -40,   -6,   -7,  -11,  -16,  -12,  -51,
];

const MG_BISHOP_TABLE: [i32; 64] = [
      30,  -76,  -15, -108,  -68,  -99,  -34,  -62,
     -13,    4,  -15,  -40,   24,    2,   -2,  -13,
     -10,   22,   15,   23,   30,   48,   47,   15,
     -16,    3,   14,   33,   16,   31,   10,  -19,
     -14,  -12,   -1,   30,   22,    3,   -5,   -3,
       3,    1,    6,   13,   14,   11,    4,    9,
      10,    7,   20,   -1,    7,   19,   24,    8,
     -13,    7,   -6,   -8,    6,   -8,   27,    7,
];

const EG_BISHOP_TABLE: [i32; 64] = [
       7,   16,    7,   16,    8,   10,   11,   -1,
      -5,    4,   -7,   22,  -19,   -1,    3,   -4,
       9,    2,   -2,   -7,   -5,   -4,   -7,    4,
      -5,    6,   11,   15,   10,    4,    4,   14,
       9,   11,   10,    4,    6,    1,   16,  -10,
       0,    2,    6,    3,   12,    8,    4,    1,
     -17,  -15,  -12,    2,   -1,   -5,   -8,  -12,
     -18,    0,   -7,   -3,   -7,    0,  -34,  -29,
];

const MG_ROOK_TABLE: [i32; 64] = [
      -1,   14,  -12,    4,    3,   39,   86,   63,
      -4,   -9,    5,   32,   14,   43,   34,   18,
     -19,   14,   -8,   16,   30,   47,   74,   38,
     -32,    5,  -22,  -14,  -10,   -2,   19,   12,
     -40,  -46,  -19,  -16,  -10,  -27,   -4,  -10,
     -25,  -32,  -31,  -25,  -20,  -14,   18,   -1,
     -39,  -27,  -18,  -24,  -18,   -2,    5,  -19,
     -11,  -13,   -8,   -3,    3,    1,    3,  -11,
];

const EG_ROOK_TABLE: [i32; 64] = [
      15,    6,   29,   13,   13,   17,   -5,   13,
      14,   21,   18,    5,    7,    7,    6,   17,
      17,    6,   13,    8,   -3,   -5,    1,    1,
      15,    5,   14,   11,   -5,    3,   -9,   -8,
       9,   11,   10,    4,   -8,   -1,   -4,    1,
      -3,    4,   -3,   -7,   -3,  -19,  -30,  -29,
      -5,   -9,  -11,   -5,  -13,  -19,  -20,  -23,
       2,   -4,   -5,   -8,  -16,   -4,  -18,  -13,
];

const MG_QUEEN_TABLE: [i32; 64] = [
     -57,  -56,  -85,  -35,  -28,  -18,   89,   -4,
      -2,  -17,  -19,  -53,  -53,    3,    8,   41,
       8,   -8,   -6,  -31,   23,   41,   56,   15,
     -17,   -1,  -19,  -17,   -6,   -4,    9,   21,
      -4,  -14,  -22,  -17,  -13,  -14,   11,   16,
       0,    1,    1,   -3,   -4,   14,   15,   13,
      21,   -1,   16,   20,   13,   26,   22,   37,
       4,   -7,    4,   17,   13,    7,   14,    4,
];

const EG_QUEEN_TABLE: [i32; 64] = [
      53,   12,   67,   35,   28,   33,  -36,    8,
       0,    0,   55,   92,  106,   22,  -15,   13,
     -10,    0,   26,   66,   13,   10,  -25,   23,
       0,   -1,   12,   34,   28,   44,   26,   10,
     -15,   -9,   29,   36,   31,   37,    2,   12,
     -26,  -22,   -9,   -8,   15,  -29,  -22,  -15,
     -72,  -25,  -51,  -50,  -26,  -51,  -77,  -94,
     -36,  -23,  -32,  -31,  -48,  -46,  -69,  -29,
];

const MG_KING_TABLE: [i32; 64] = [
     -16,   16,   17,  -95,  -51,   -3,   54,   63,
     -96,  -52,  -82,   23,   33,  -10,   79,   60,
    -127,   15,  -15,  -57,  -33,   70,   48,    1,
     -19,  -11, -104, -135, -114,  -69,  -81, -104,
    -115,  -53,  -70, -115, -116,  -61,  -69, -168,
       6,   -1,  -55,  -71,  -49,  -47,  -21,  -49,
      62,   16,   -7,  -31,  -35,  -24,   25,   25,
      52,   55,   34,  -64,   -8,  -35,   31,   36,
];

const EG_KING_TABLE: [i32; 64] = [
     -82,  -45,  -36,    3,   14,   -1,   39,  -91,
      -1,   29,   35,   27,   19,   48,   44,    0,
      30,   24,   40,   54,   71,   53,   52,   13,
       5,   38,   51,   63,   68,   57,   54,   22,
       7,   24,   37,   52,   60,   39,   30,   33,
     -28,    0,   21,   28,   28,   23,    2,    0,
     -37,  -17,   -3,    3,   11,    4,  -14,  -26,
     -95,  -57,  -43,  -18,  -37,  -25,  -52,  -81,
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
