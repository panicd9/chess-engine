//! Zobrist hashing: a 64-bit key identifying a position.
//!
//! Each (piece, square) pair, each castling right, the file of an en passant
//! target and the side to move get a fixed random number; a position's key is
//! the XOR of the ones that apply. Equal keys mean equal positions (barring a
//! collision, which at 64 bits is negligible), which is what makes repetition
//! detection cheap. The same keys are what a transposition table would index by.

use crate::chessboard::{Chessboard, Color};
use crate::piece::ColoredPiece;

/// xorshift64*, used only to fill the tables below at compile time. Any
/// generator with good bit dispersion works; the values just have to be fixed.
const fn next(seed: u64) -> u64 {
    let mut x = seed;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

/// `[piece][square]`, indexed by `ColoredPiece as usize`. `Empty` is index 12
/// and is never used, but keeping it makes indexing branch-free.
const PIECE_SQUARE: [[u64; 64]; 13] = {
    let mut table = [[0u64; 64]; 13];
    let mut seed = 0x9E37_79B9_7F4A_7C15;
    let mut piece = 0;
    while piece < 13 {
        let mut square = 0;
        while square < 64 {
            seed = next(seed);
            table[piece][square] = seed;
            square += 1;
        }
        piece += 1;
    }
    table
};

/// One key per castling right, in the order white king, white queen, black
/// king, black queen.
const CASTLING: [u64; 4] = {
    let mut table = [0u64; 4];
    let mut seed = next(0xDEAD_BEEF_CAFE_F00D);
    let mut i = 0;
    while i < 4 {
        seed = next(seed);
        table[i] = seed;
        i += 1;
    }
    table
};

/// One key per en passant *file*. Only the file matters: the rank is implied by
/// the side to move.
const EN_PASSANT_FILE: [u64; 8] = {
    let mut table = [0u64; 8];
    let mut seed = next(0x0123_4567_89AB_CDEF);
    let mut i = 0;
    while i < 8 {
        seed = next(seed);
        table[i] = seed;
        i += 1;
    }
    table
};

const BLACK_TO_MOVE: u64 = next(0xFEED_FACE_DEAD_C0DE);

/// The key for a position.
///
/// Deliberately ignores the halfmove clock and fullmove counter: two positions
/// that differ only in those are the same position for repetition purposes.
pub fn hash(cb: &Chessboard) -> u64 {
    let mut key = 0u64;

    for (square, &piece) in cb.piece_square.iter().enumerate() {
        if !matches!(piece, ColoredPiece::Empty) {
            key ^= PIECE_SQUARE[piece as usize][square];
        }
    }

    for (index, allowed) in [
        cb.white_can_castle_king_side,
        cb.white_can_castle_queen_side,
        cb.black_can_castle_king_side,
        cb.black_can_castle_queen_side,
    ]
    .into_iter()
    .enumerate()
    {
        if allowed {
            key ^= CASTLING[index];
        }
    }

    if cb.en_passant != 0 {
        key ^= EN_PASSANT_FILE[(cb.en_passant.trailing_zeros() % 8) as usize];
    }

    if cb.side_to_move == Color::Black {
        key ^= BLACK_TO_MOVE;
    }

    key
}
