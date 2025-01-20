use crate::{chessboard::SingletonBitboard, squares::SquareBitboard};

pub const DIAGONAL_MASKS: [u64; 64] = [
    0x8040201008040201,
    0x80402010080402,
    0x804020100804,
    0x8040201008,
    0x80402010,
    0x804020,
    0x8040,
    0x80,
    0x4020100804020100,
    0x8040201008040201,
    0x80402010080402,
    0x804020100804,
    0x8040201008,
    0x80402010,
    0x804020,
    0x8040,
    0x2010080402010000,
    0x4020100804020100,
    0x8040201008040201,
    0x80402010080402,
    0x804020100804,
    0x8040201008,
    0x80402010,
    0x804020,
    0x1008040201000000,
    0x2010080402010000,
    0x4020100804020100,
    0x8040201008040201,
    0x80402010080402,
    0x804020100804,
    0x8040201008,
    0x80402010,
    0x804020100000000,
    0x1008040201000000,
    0x2010080402010000,
    0x4020100804020100,
    0x8040201008040201,
    0x80402010080402,
    0x804020100804,
    0x8040201008,
    0x402010000000000,
    0x804020100000000,
    0x1008040201000000,
    0x2010080402010000,
    0x4020100804020100,
    0x8040201008040201,
    0x80402010080402,
    0x804020100804,
    0x201000000000000,
    0x402010000000000,
    0x804020100000000,
    0x1008040201000000,
    0x2010080402010000,
    0x4020100804020100,
    0x8040201008040201,
    0x80402010080402,
    0x100000000000000,
    0x201000000000000,
    0x402010000000000,
    0x804020100000000,
    0x1008040201000000,
    0x2010080402010000,
    0x4020100804020100,
    0x8040201008040201,
];

pub const ANTI_DIAGONAL_MASKS: [u64; 64] = [
    0x1,
    0x102,
    0x10204,
    0x1020408,
    0x102040810,
    0x10204081020,
    0x1020408102040,
    0x102040810204080,
    0x102,
    0x10204,
    0x1020408,
    0x102040810,
    0x10204081020,
    0x1020408102040,
    0x102040810204080,
    0x204081020408000,
    0x10204,
    0x1020408,
    0x102040810,
    0x10204081020,
    0x1020408102040,
    0x102040810204080,
    0x204081020408000,
    0x408102040800000,
    0x1020408,
    0x102040810,
    0x10204081020,
    0x1020408102040,
    0x102040810204080,
    0x204081020408000,
    0x408102040800000,
    0x810204080000000,
    0x102040810,
    0x10204081020,
    0x1020408102040,
    0x102040810204080,
    0x204081020408000,
    0x408102040800000,
    0x810204080000000,
    0x1020408000000000,
    0x10204081020,
    0x1020408102040,
    0x102040810204080,
    0x204081020408000,
    0x408102040800000,
    0x810204080000000,
    0x1020408000000000,
    0x2040800000000000,
    0x1020408102040,
    0x102040810204080,
    0x204081020408000,
    0x408102040800000,
    0x810204080000000,
    0x1020408000000000,
    0x2040800000000000,
    0x4080000000000000,
    0x102040810204080,
    0x204081020408000,
    0x408102040800000,
    0x810204080000000,
    0x1020408000000000,
    0x2040800000000000,
    0x4080000000000000,
    0x8000000000000000,
];

pub const WHITE_KING_CASTLE_EMPTY_SQUARES: u64 = 0x60;
pub const WHITE_QUEEN_CASTLE_EMPTY_SQUARES: u64 = 0xe;
pub const BLACK_KING_CASTLE_EMPTY_SQUARES: u64 = 0x6000000000000000;
pub const BLACK_QUEEN_CASTLE_EMPTY_SQUARES: u64 = 0xe00000000000000;

pub const WHITE_QUEEN_CASTLE_KING_PASSTHROUGH_SQUARES: u64 = 0x1c;
pub const BLACK_QUEEN_CASTLE_KING_PASSTHROUGH_SQUARES: u64 = 0x1c00000000000000;

pub const WHITE_KING_CASTLE_KING_PASSTHROUGH_SQUARES: u64 = 0x70;
pub const BLACK_KING_CASTLE_KING_PASSTHROUGH_SQUARES: u64 = 0x7000000000000000;

pub const WHITE_ROOK_QUEENSIDE: u64 = 0x1;
pub const WHITE_ROOK_KINGSIDE: u64 = 0x80;
pub const WHITE_ROOKS_MASK: u64 = WHITE_ROOK_QUEENSIDE | WHITE_ROOK_KINGSIDE;

pub const BLACK_ROOK_QUEENSIDE: u64 = 0x100000000000000;
pub const BLACK_ROOK_KINGSIDE: u64 = 0x8000000000000000;
pub const BLACK_ROOKS_MASK: u64 = BLACK_ROOK_QUEENSIDE | BLACK_ROOK_KINGSIDE;

pub const WHITE_KINGSIDE_ROOK: SingletonBitboard = SquareBitboard::H1 as u64;
pub const WHITE_QUEENSIDE_ROOK: SingletonBitboard = SquareBitboard::A1 as u64;
pub const BLACK_KINGSIDE_ROOK: SingletonBitboard = SquareBitboard::H8 as u64;
pub const BLACK_QUEENSIDE_ROOK: SingletonBitboard = SquareBitboard::A8 as u64;

pub fn reverse_bits(bits: u64) -> u64 {
    bits.reverse_bits()
}

/// Calculate sliding piece attacks for a specific line (rank, file, or diagonal) using Hyperbola Quintessence.
///
/// # Arguments
/// * `line_mask` - The mask representing the line (rank, file, or diagonal).
/// * `pieces_in_line` - The pieces (rooks or bishops) within the specified line.
/// * `occupancy_in_line` - The occupancy (blocking pieces) within the specified line.
///
/// # Returns
/// A `u64` bitboard representing the attack set for the sliding pieces on the given line.
pub fn calculate_sliding_attacks(
    line_mask: u64,
    pieces_in_line: u64,
    occupancy_in_line: u64,
) -> u64 {
    if pieces_in_line == 0 {
        return 0;
    }

    let forward = occupancy_in_line.wrapping_sub(pieces_in_line.wrapping_mul(2));
    let reverse =
        reverse_bits(occupancy_in_line).wrapping_sub(reverse_bits(pieces_in_line).wrapping_mul(2));
    (forward ^ reverse_bits(reverse)) & line_mask
}
