//! Reading and writing moves in the long algebraic notation UCI uses:
//! `e2e4`, or `e7e8q` for a promotion.

use std::fmt;

use crate::chessboard::{Chessboard, Color, SquareIndex};
use crate::piece::{ColoredPiece, PromotionPiece};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    WrongLength,
    BadFile(char),
    BadRank(char),
    BadPromotionPiece(char),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::WrongLength => write!(f, "a move looks like `e2e4`, or `e7e8q` to promote"),
            ParseError::BadFile(c) => write!(f, "`{c}` is not a file (a-h)"),
            ParseError::BadRank(c) => write!(f, "`{c}` is not a rank (1-8)"),
            ParseError::BadPromotionPiece(c) => {
                write!(f, "`{c}` is not a promotion piece (q, r, b or n)")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// A move as written, before it has been checked for legality.
#[derive(Debug, Clone, Copy)]
pub struct ParsedMove {
    pub from: SquareIndex,
    pub to: SquareIndex,
    pub promotion: Option<PromotionPiece>,
}

/// Parse `e2e4` or `e7e8q`.
pub fn parse_move(text: &str) -> Result<ParsedMove, ParseError> {
    let chars: Vec<char> = text.trim().chars().collect();
    if chars.len() != 4 && chars.len() != 5 {
        return Err(ParseError::WrongLength);
    }
    let from = parse_square(chars[0], chars[1])?;
    let to = parse_square(chars[2], chars[3])?;
    let promotion = match chars.get(4) {
        Some(&c) => Some(parse_promotion_piece(c)?),
        None => None,
    };
    Ok(ParsedMove { from, to, promotion })
}

fn parse_square(file: char, rank: char) -> Result<SquareIndex, ParseError> {
    let file_index = match file.to_ascii_lowercase() {
        c @ 'a'..='h' => c as usize - 'a' as usize,
        c => return Err(ParseError::BadFile(c)),
    };
    let rank_index = match rank {
        c @ '1'..='8' => c as usize - '1' as usize,
        c => return Err(ParseError::BadRank(c)),
    };
    Ok(rank_index * 8 + file_index)
}

fn parse_promotion_piece(piece: char) -> Result<PromotionPiece, ParseError> {
    match piece.to_ascii_lowercase() {
        'q' => Ok(PromotionPiece::Queen),
        'r' => Ok(PromotionPiece::Rook),
        'b' => Ok(PromotionPiece::Bishop),
        'n' => Ok(PromotionPiece::Knight),
        c => Err(ParseError::BadPromotionPiece(c)),
    }
}

/// Square index to algebraic notation: 28 -> `e4`.
pub fn square_to_notation(square: SquareIndex) -> String {
    let file = (b'a' + (square % 8) as u8) as char;
    let rank = square / 8 + 1;
    format!("{file}{rank}")
}

/// Two squares as a UCI move: `e2e4`.
pub fn format_move(from: SquareIndex, to: SquareIndex) -> String {
    format!("{}{}", square_to_notation(from), square_to_notation(to))
}

/// Work out which move turned `before` into `after`, and render it as UCI.
///
/// Move generation yields whole positions rather than moves, so the move has to
/// be recovered by diffing the two boards. Only the moving side's occupancy is
/// compared: a captured piece belongs to the opponent, so it never appears as a
/// square the mover left or arrived on. That makes captures, en passant and
/// promotions all reduce to a single from/to pair.
pub fn describe_move(before: &Chessboard, after: &Chessboard) -> Option<String> {
    let (before_occupancy, after_occupancy) = match before.side_to_move {
        Color::White => (before.get_white_occupancy(), after.get_white_occupancy()),
        Color::Black => (before.get_black_occupancy(), after.get_black_occupancy()),
    };

    let vacated = before_occupancy & !after_occupancy;
    let filled = after_occupancy & !before_occupancy;
    if vacated == 0 || filled == 0 {
        return None;
    }

    // Castling moves two pieces at once; UCI names it by the king's travel.
    if vacated.count_ones() == 2 {
        let (king_before, king_after) = match before.side_to_move {
            Color::White => (before.white_king, after.white_king),
            Color::Black => (before.black_king, after.black_king),
        };
        return Some(format_move(
            king_before.trailing_zeros() as SquareIndex,
            king_after.trailing_zeros() as SquareIndex,
        ));
    }

    let from = vacated.trailing_zeros() as SquareIndex;
    let to = filled.trailing_zeros() as SquareIndex;
    let mut text = format_move(from, to);

    // A pawn that arrives as something else has promoted.
    if matches!(
        before.piece_square[from],
        ColoredPiece::WhitePawn | ColoredPiece::BlackPawn
    ) {
        if let Some(suffix) = promotion_suffix(after.piece_square[to]) {
            text.push(suffix);
        }
    }

    Some(text)
}

fn promotion_suffix(piece: ColoredPiece) -> Option<char> {
    match piece {
        ColoredPiece::WhiteQueen | ColoredPiece::BlackQueen => Some('q'),
        ColoredPiece::WhiteRook | ColoredPiece::BlackRook => Some('r'),
        ColoredPiece::WhiteBishop | ColoredPiece::BlackBishop => Some('b'),
        ColoredPiece::WhiteKnight | ColoredPiece::BlackKnight => Some('n'),
        _ => None,
    }
}
