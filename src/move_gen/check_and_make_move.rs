use crate::{
    chessboard::{
        rank_masks::{RANK_1, RANK_2, RANK_7, RANK_8},
        Chessboard, Square,
    },
    piece::{self, Piece, PromotionPiece},
    utils::{
        singleton_bitboard_from_square, BLACK_KINGSIDE_TARGET_SQUARE,
        BLACK_KING_CASTLE_EMPTY_SQUARES, BLACK_KING_CASTLE_KING_PASSTHROUGH_SQUARES,
        BLACK_QUEENSIDE_TARGET_SQUARE, BLACK_QUEEN_CASTLE_EMPTY_SQUARES,
        BLACK_QUEEN_CASTLE_KING_PASSTHROUGH_SQUARES, WHITE_KINGSIDE_TARGET_SQUARE,
        WHITE_KING_CASTLE_EMPTY_SQUARES, WHITE_KING_CASTLE_KING_PASSTHROUGH_SQUARES,
        WHITE_QUEENSIDE_TARGET_SQUARE, WHITE_QUEEN_CASTLE_EMPTY_SQUARES,
        WHITE_QUEEN_CASTLE_KING_PASSTHROUGH_SQUARES,
    },
};

use super::{
    move_gen_bishop::single_bishop_attacks,
    move_gen_king::king_attacks,
    move_gen_knight::knight_attacks_from_single_knight_bitboard,
    move_gen_pawn::{
        BLACK_PAWN_ATTACKS, BLACK_PAWN_FORWARD_MOVES, WHITE_PAWN_ATTACKS, WHITE_PAWN_FORWARD_MOVES,
    },
    move_gen_queen::single_queen_attacks,
    move_gen_rook::single_rook_attacks,
};

pub fn check_and_make_white_pawn_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
    promotion_piece: Option<PromotionPiece>,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let en_passant_square = cb.get_en_passant_bitboard();
    let occupancy = cb.get_occupancy();

    let mut remaining_pawns = cb.white_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        if single_pawn != from_bitboard {
            remaining_pawns &= remaining_pawns - 1;
            continue;
        }
        // Forward moves
        let forward = WHITE_PAWN_FORWARD_MOVES[from] & !occupancy;
        if forward != 0 {
            // Not promotion
            if forward & RANK_8 == 0 {
                if forward == to_bitboard {
                    let new_position = cb.make_white_pawn_forward_move(single_pawn, forward);
                    // TODO: Can be optimized by checking if the king is under attack before changing side to move
                    if !new_position.is_white_king_under_attack() {
                        return Ok(new_position);
                    }
                }

                // Double forward move (only from the second rank)
                if (single_pawn & RANK_2) != 0 {
                    let double_forward = (single_pawn << 16) & !occupancy;
                    if double_forward != 0 && double_forward == to_bitboard {
                        let new_double_position =
                            cb.make_white_pawn_double_forward_move(single_pawn, double_forward);
                        if !new_double_position.is_white_king_under_attack() {
                            return Ok(new_double_position);
                        }
                    }
                }
                // Promotion
            } else {
                // Queen, knight, bishop, rook
                let new_promotion_positions =
                    cb.make_all_white_pawn_promotion_moves(single_pawn, forward);
                match promotion_piece {
                    Some(piece) => match piece {
                        PromotionPiece::Queen => return Ok(new_promotion_positions[0]),
                        PromotionPiece::Knight => return Ok(new_promotion_positions[1]),
                        PromotionPiece::Bishop => return Ok(new_promotion_positions[2]),
                        PromotionPiece::Rook => return Ok(new_promotion_positions[3]),
                    },
                    None => return Err("Promotion piece not provided"),
                }
            }
        }

        // Attack moves
        let attacks = WHITE_PAWN_ATTACKS[from] & cb.get_black_occupancy();
        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack != to_bitboard {
                remaining_attacks &= remaining_attacks - 1;
                continue;
            }

            if single_attack & RANK_8 == 0 {
                let new_position = cb.make_white_pawn_capture_move(single_pawn, single_attack);
                if !new_position.is_white_king_under_attack() {
                    return Ok(new_position);
                }
            } else {
                let new_promotion_positions =
                    cb.make_all_white_pawn_capture_promotion_moves(single_pawn, single_attack);
                match promotion_piece {
                    Some(piece) => match piece {
                        PromotionPiece::Queen => return Ok(new_promotion_positions[0]),
                        PromotionPiece::Knight => return Ok(new_promotion_positions[1]),
                        PromotionPiece::Bishop => return Ok(new_promotion_positions[2]),
                        PromotionPiece::Rook => return Ok(new_promotion_positions[3]),
                    },
                    None => return Err("Promotion piece not provided"),
                }
            }
            remaining_attacks &= remaining_attacks - 1;
        }

        // En passant capture
        if en_passant_square != 0 {
            let en_passant_mask = en_passant_square & WHITE_PAWN_ATTACKS[from];
            if en_passant_mask != 0 {
                let new_position =
                    cb.make_white_pawn_en_passant_capture(single_pawn, en_passant_mask);
                if !new_position.is_white_king_under_attack() {
                    return Ok(new_position);
                }
            }
        }
        remaining_pawns &= remaining_pawns - 1;
    }

    Err("Invalid move")
}

pub fn check_and_make_black_pawn_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
    promotion_piece: Option<PromotionPiece>,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let en_passant_square = cb.get_en_passant_bitboard();
    let occupancy = cb.get_occupancy();

    let mut remaining_pawns = cb.black_pawns;
    while remaining_pawns != 0 {
        let single_pawn = remaining_pawns & remaining_pawns.wrapping_neg();
        if single_pawn != from_bitboard {
            remaining_pawns &= remaining_pawns - 1;
            continue;
        }
        // Forward moves
        let forward = BLACK_PAWN_FORWARD_MOVES[from] & !occupancy;
        if forward != 0 {
            // Not promotion
            if forward & RANK_1 == 0 {
                if forward == to_bitboard {
                    let new_position = cb.make_black_pawn_forward_move(single_pawn, forward);
                    if !new_position.is_black_king_under_attack() {
                        return Ok(new_position);
                    }
                }

                // Double forward move (only from the seventh rank)
                if (single_pawn & RANK_7) != 0 {
                    let double_forward = (single_pawn >> 16) & !occupancy;
                    if double_forward != 0 && double_forward == to_bitboard {
                        let new_double_position =
                            cb.make_black_pawn_double_forward_move(single_pawn, double_forward);
                        if !new_double_position.is_black_king_under_attack() {
                            return Ok(new_double_position);
                        }
                    }
                }
                // Promotion
            } else {
                // Queen, knight, bishop, rook
                let new_promotion_positions =
                    cb.make_all_black_pawn_promotion_moves(single_pawn, forward);
                match promotion_piece {
                    Some(piece) => match piece {
                        PromotionPiece::Queen => return Ok(new_promotion_positions[0]),
                        PromotionPiece::Knight => return Ok(new_promotion_positions[1]),
                        PromotionPiece::Bishop => return Ok(new_promotion_positions[2]),
                        PromotionPiece::Rook => return Ok(new_promotion_positions[3]),
                    },
                    None => return Err("Promotion piece not provided"),
                }
            }
        }

        // Attack moves
        let attacks = BLACK_PAWN_ATTACKS[from] & cb.get_white_occupancy();
        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack != to_bitboard {
                remaining_attacks &= remaining_attacks - 1;
                continue;
            }

            if single_attack & RANK_1 == 0 {
                let new_position = cb.make_black_pawn_capture_move(single_pawn, single_attack);
                if !new_position.is_black_king_under_attack() {
                    return Ok(new_position);
                }
            } else {
                let new_promotion_positions =
                    cb.make_all_black_pawn_capture_promotion_moves(single_pawn, single_attack);
                match promotion_piece {
                    Some(piece) => match piece {
                        PromotionPiece::Queen => return Ok(new_promotion_positions[0]),
                        PromotionPiece::Knight => return Ok(new_promotion_positions[1]),
                        PromotionPiece::Bishop => return Ok(new_promotion_positions[2]),
                        PromotionPiece::Rook => return Ok(new_promotion_positions[3]),
                    },
                    None => return Err("Promotion piece not provided"),
                }
            }
            remaining_attacks &= remaining_attacks - 1;
        }

        // En passant capture
        if en_passant_square != 0 {
            let en_passant_mask = en_passant_square & BLACK_PAWN_ATTACKS[from];
            if en_passant_mask != 0 {
                let new_position =
                    cb.make_black_pawn_en_passant_capture(single_pawn, en_passant_mask);
                if !new_position.is_black_king_under_attack() {
                    return Ok(new_position);
                }
            }
        }
        remaining_pawns &= remaining_pawns - 1;
    }

    Err("Invalid move")
}

pub fn check_and_make_white_rook_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let mut remaining_rooks = cb.white_rooks;

    while remaining_rooks != 0 {
        let single_rook_bitboard = remaining_rooks & remaining_rooks.wrapping_neg(); // Get the least significant rook
        if single_rook_bitboard != from_bitboard {
            remaining_rooks &= remaining_rooks - 1; // Remove the least significant rook
            continue;
        }

        let occupancy = cb.get_occupancy();
        let attacks =
            single_rook_attacks(occupancy, single_rook_bitboard) & !cb.get_white_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack_bitboard != to_bitboard {
                remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
                continue;
            }
            // let mov = Move::new(cb, Piece::Rook, single_rook_bitboard, single_attack_bitboard);
            let new_position =
                cb.make_white_rook_move(single_rook_bitboard, single_attack_bitboard);
            if !new_position.is_white_king_under_attack() {
                return Ok(new_position);
            }
            remaining_attacks &= remaining_attacks - 1;
        }
        remaining_rooks &= remaining_rooks - 1;
    }

    return Err("Invalid move");
}

pub fn check_and_make_black_rook_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let mut remaining_rooks = cb.black_rooks;

    while remaining_rooks != 0 {
        let single_rook_bitboard = remaining_rooks & remaining_rooks.wrapping_neg(); // Get the least significant rook
        if single_rook_bitboard != from_bitboard {
            remaining_rooks &= remaining_rooks - 1; // Remove the least significant rook
            continue;
        }

        let occupancy = cb.get_occupancy();
        let attacks =
            single_rook_attacks(occupancy, single_rook_bitboard) & !cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack_bitboard != to_bitboard {
                remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
                continue;
            }

            let new_position =
                cb.make_black_rook_move(single_rook_bitboard, single_attack_bitboard);
            if !new_position.is_black_king_under_attack() {
                return Ok(new_position);
            }
            remaining_attacks &= remaining_attacks - 1;
        }
        remaining_rooks &= remaining_rooks - 1;
    }

    Err("Invalid move")
}

pub fn check_and_make_white_bishop_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let mut remaining_bishops = cb.white_bishops;

    while remaining_bishops != 0 {
        let single_bishop_bitboard = remaining_bishops & remaining_bishops.wrapping_neg(); // Get the least significant bishop
        if single_bishop_bitboard != from_bitboard {
            remaining_bishops &= remaining_bishops - 1; // Remove the least significant bishop
            continue;
        }
        let occupancy = cb.get_occupancy();
        let attacks =
            single_bishop_attacks(occupancy, single_bishop_bitboard) & !cb.get_white_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack_bitboard != to_bitboard {
                remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
                continue;
            }
            let new_position =
                cb.make_white_bishop_move(single_bishop_bitboard, single_attack_bitboard);
            if !new_position.is_white_king_under_attack() {
                return Ok(new_position);
            }
            remaining_attacks &= remaining_attacks - 1;
        }
        remaining_bishops &= remaining_bishops - 1;
    }

    return Err("Invalid move");
}

pub fn check_and_make_black_bishop_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let mut remaining_bishops = cb.black_bishops;

    while remaining_bishops != 0 {
        let single_bishop_bitboard = remaining_bishops & remaining_bishops.wrapping_neg(); // Get the least significant bishop
        if single_bishop_bitboard != from_bitboard {
            remaining_bishops &= remaining_bishops - 1; // Remove the least significant bishop
            continue;
        }
        let occupancy = cb.get_occupancy();
        let attacks =
            single_bishop_attacks(occupancy, single_bishop_bitboard) & !cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack_bitboard != to_bitboard {
                remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
                continue;
            }
            let new_position =
                cb.make_black_bishop_move(single_bishop_bitboard, single_attack_bitboard);
            if !new_position.is_black_king_under_attack() {
                return Ok(new_position);
            }
            remaining_attacks &= remaining_attacks - 1;
        }
        remaining_bishops &= remaining_bishops - 1;
    }

    Err("Invalid move")
}

pub fn check_and_make_white_knight_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let mut remaining_knights = cb.white_knights;

    while remaining_knights != 0 {
        let single_knight_bitboard = remaining_knights & remaining_knights.wrapping_neg(); // Get the least significant knight
        if single_knight_bitboard != from_bitboard {
            remaining_knights &= remaining_knights - 1; // Remove the least significant knight
            continue;
        }
        let attacks = knight_attacks_from_single_knight_bitboard(single_knight_bitboard)
            & !cb.get_white_occupancy();
        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack_bitboard != to_bitboard {
                remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
                continue;
            }
            let new_position =
                cb.make_white_knight_move(single_knight_bitboard, single_attack_bitboard);
            if !new_position.is_white_king_under_attack() {
                return Ok(new_position);
            }
            remaining_attacks &= remaining_attacks - 1;
        }
        remaining_knights &= remaining_knights - 1; 
    }
    return Err("Invalid move");
}

pub fn check_and_make_black_knight_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let mut remaining_knights = cb.black_knights;

    while remaining_knights != 0 {
        let single_knight_bitboard = remaining_knights & remaining_knights.wrapping_neg(); // Get the least significant knight
        if single_knight_bitboard != from_bitboard {
            remaining_knights &= remaining_knights - 1; // Remove the least significant knight
            continue;
        }
        let attacks = knight_attacks_from_single_knight_bitboard(single_knight_bitboard)
            & !cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack_bitboard != to_bitboard {
                remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
                continue;
            }
            let new_position =
                cb.make_black_knight_move(single_knight_bitboard, single_attack_bitboard);
            if !new_position.is_black_king_under_attack() {
                return Ok(new_position);
            }
            remaining_attacks &= remaining_attacks - 1;
        }
        remaining_knights &= remaining_knights - 1;
    }

    Err("Invalid move")
}

pub fn check_and_make_white_queen_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let mut remaining_queens = cb.white_queens;

    while remaining_queens != 0 {
        let single_queen_bitboard = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB
        if single_queen_bitboard != from_bitboard {
            remaining_queens &= remaining_queens - 1; // Remove LSB
            continue;
        }
        let occupancy = cb.get_occupancy();
        let attacks =
            single_queen_attacks(occupancy, single_queen_bitboard) & !cb.get_white_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack_bitboard != to_bitboard {
                remaining_attacks &= remaining_attacks - 1; // Remove LSB
                continue;
            }
            let new_position =
                cb.make_white_queen_move(single_queen_bitboard, single_attack_bitboard);
            if !new_position.is_white_king_under_attack() {
                return Ok(new_position);
            }
            remaining_attacks &= remaining_attacks - 1;
        }
        remaining_queens &= remaining_queens - 1; 
    }

    Err("Invalid move")
}

pub fn check_and_make_black_queen_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let mut remaining_queens = cb.black_queens;

    while remaining_queens != 0 {
        let single_queen_bitboard = remaining_queens & remaining_queens.wrapping_neg(); // Extract LSB
        if single_queen_bitboard != from_bitboard {
            remaining_queens &= remaining_queens - 1; // Remove LSB
            continue;
        }
        let occupancy = cb.get_occupancy();
        let attacks =
            single_queen_attacks(occupancy, single_queen_bitboard) & !cb.get_black_occupancy();

        let mut remaining_attacks = attacks;
        while remaining_attacks != 0 {
            let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
            if single_attack_bitboard != to_bitboard {
                remaining_attacks &= remaining_attacks - 1; // Remove LSB
                continue;
            }
            let new_position =
                cb.make_black_queen_move(single_queen_bitboard, single_attack_bitboard);
            if !new_position.is_black_king_under_attack() {
                return Ok(new_position);
            }
            remaining_attacks &= remaining_attacks - 1;
        }
        remaining_queens &= remaining_queens - 1;
    }

    Err("Invalid move")
}

pub fn check_and_make_white_king_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let king = cb.white_king;
    if king != from_bitboard {
        return Err("Invalid move");
    }

    let occupancy = cb.get_occupancy();

    // Normal king moves
    let attacks = king_attacks(king) & !cb.get_white_occupancy();
    let mut remaining_attacks = attacks;

    while remaining_attacks != 0 {
        let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
        if single_attack_bitboard != to_bitboard {
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
            continue;
        }
        let new_position = cb.make_white_king_move(king, single_attack_bitboard);
        if !new_position.is_white_king_under_attack() {
            return Ok(new_position);
        }
        remaining_attacks &= remaining_attacks - 1;
    }

    // Castling moves
    if cb.white_can_castle_king_side && WHITE_KINGSIDE_TARGET_SQUARE == to_bitboard {
        // Kingside castling: ensure the squares between the king and rook are empty
        let are_kingside_castle_squares_empty = WHITE_KING_CASTLE_EMPTY_SQUARES & occupancy == 0;
        if are_kingside_castle_squares_empty {
            // Only calculate attacks if the squares are empty
            let are_kingside_castle_squares_attacked =
                (cb.all_black_attacks() & WHITE_KING_CASTLE_KING_PASSTHROUGH_SQUARES) != 0;
            if !are_kingside_castle_squares_attacked {
                let new_position = cb.make_white_kingside_castle();
                return Ok(new_position);
            }
        }
    }

    if cb.white_can_castle_queen_side && WHITE_QUEENSIDE_TARGET_SQUARE == to_bitboard {
        // Queenside castling: ensure the squares between the king and rook are empty
        let are_queen_side_castle_squares_empty = WHITE_QUEEN_CASTLE_EMPTY_SQUARES & occupancy == 0;
        if are_queen_side_castle_squares_empty {
            // Only calculate attacks if the squares are empty
            let are_queenside_castle_squares_attacked =
                (cb.all_black_attacks() & WHITE_QUEEN_CASTLE_KING_PASSTHROUGH_SQUARES) != 0;
            if !are_queenside_castle_squares_attacked {
                let new_position = cb.make_white_queenside_castle();
                return Ok(new_position);
            }
        }
    }

    return Err("Invalid move");
}

pub fn check_and_make_black_king_move(
    cb: &Chessboard,
    from: Square,
    to: Square,
) -> Result<Chessboard, &'static str> {
    let from_bitboard = singleton_bitboard_from_square(from);
    let to_bitboard = singleton_bitboard_from_square(to);

    let king = cb.black_king;
    if king != from_bitboard {
        return Err("Invalid move");
    }

    let occupancy = cb.get_occupancy();

    // Normal king moves
    let attacks = king_attacks(king) & !cb.get_black_occupancy();
    let mut remaining_attacks = attacks;

    while remaining_attacks != 0 {
        let single_attack_bitboard = remaining_attacks & remaining_attacks.wrapping_neg();
        if single_attack_bitboard != to_bitboard {
            remaining_attacks &= remaining_attacks - 1; // Remove the least significant attack
            continue;
        }
        let new_position = cb.make_black_king_move(king, single_attack_bitboard);
        if !new_position.is_black_king_under_attack() {
            return Ok(new_position);
        }
        remaining_attacks &= remaining_attacks - 1; 
    }

    // Castling moves
    if cb.black_can_castle_king_side && BLACK_KINGSIDE_TARGET_SQUARE == to_bitboard {
        // Kingside castling: ensure the squares between the king and rook are empty
        let are_kingside_castle_squares_empty = BLACK_KING_CASTLE_EMPTY_SQUARES & occupancy == 0;
        if are_kingside_castle_squares_empty {
            // Only calculate attacks if the squares are empty
            let are_kingside_castle_squares_attacked =
                (cb.all_white_attacks() & BLACK_KING_CASTLE_KING_PASSTHROUGH_SQUARES) != 0;
            if !are_kingside_castle_squares_attacked {
                let new_position = cb.make_black_kingside_castle();
                return Ok(new_position);
            }
        }
    }

    if cb.black_can_castle_queen_side && BLACK_QUEENSIDE_TARGET_SQUARE == to_bitboard {
        // Queenside castling: ensure the squares between the king and rook are empty
        let are_queen_side_castle_squares_empty = BLACK_QUEEN_CASTLE_EMPTY_SQUARES & occupancy == 0;
        if are_queen_side_castle_squares_empty {
            // Only calculate attacks if the squares are empty
            let are_queenside_castle_squares_attacked =
                (cb.all_white_attacks() & BLACK_QUEEN_CASTLE_KING_PASSTHROUGH_SQUARES) != 0;
            if !are_queenside_castle_squares_attacked {
                let new_position = cb.make_black_queenside_castle();
                return Ok(new_position);
            }
        }
    }

    Err("Invalid move")
}
