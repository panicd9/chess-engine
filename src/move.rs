use crate::{
    chessboard::{
        rank_masks::{RANK_1, RANK_8},
        Chessboard, WHITE_KING_CASTLE_SQUARE, WHITE_KING_START_SQUARE, WHITE_QUEEN_CASTLE_SQUARE,
    },
    piece::Piece,
};

#[derive(Debug, Clone, Copy)]
pub struct Move {
    piece_to_move: Piece,
    from: u64,
    to: u64,
    flags: u32,
}

impl Move {
    // Define some example flags for move types (more can be added as needed)
    const QUIET_MOVE: u32 = 0;
    const DOUBLE_PAWN_PUSH: u32 = 1;
    const CASTLE_KING: u32 = 2;
    const CASTLE_QUEEN: u32 = 3;
    const CAPTURE: u32 = 4;
    const EP_CAPTURE: u32 = 5;
    const PROMOTION: u32 = 6;
    const PROMOTION_CAPTURE: u32 = 7;
    const CHECK: u32 = 8;
    const CHECKMATE: u32 = 9;

    // Constructor to create a move with flags
    pub fn new_with_flags(piece: Piece, from_bitboard: u64, to_bitboard: u64, flags: u32) -> Self {
        Move {
            piece_to_move: piece,
            from: from_bitboard,
            to: to_bitboard,
            flags,
        }
    }

    // Constructor to create a move
    pub fn new(cb: &Chessboard, piece: Piece, from: u64, to: u64) -> Self {
        let enemy_occupancy = cb.get_enemy_color_occupancy();

        match piece {
            Piece::Pawn => {
                let is_promotion = to & (RANK_1 | RANK_8) > 0;

                // Quiet move (most common)
                if (enemy_occupancy & to) == 0 {
                    if !is_promotion {
                        // Double pawn push (less common than quiet move, but still a quiet move)
                        if to == (from << 16) || to == (from >> 16) {
                            return Self::new_with_flags(Piece::Pawn, from, to, Self::DOUBLE_PAWN_PUSH);
                        }
                        // Regular quiet move
                        return Self::new_with_flags(Piece::Pawn, from, to, Self::QUIET_MOVE);
                    }
                    // Promotion (quiet move)
                    return Self::new_with_flags(Piece::Pawn, from, to, Self::PROMOTION);
                }

                // Captures (second most common)
                if !is_promotion {
                    // Regular capture
                    if cb.get_en_passant_bitboard() == to {
                        // En passant capture (rare compared to regular captures)
                        return Self::new_with_flags(Piece::Pawn, from, to, Self::EP_CAPTURE);
                    }
                    return Self::new_with_flags(Piece::Pawn, from, to, Self::CAPTURE);
                }

                // Promotion capture
                return Self::new_with_flags(Piece::Pawn, from, to, Self::PROMOTION_CAPTURE);
            }
            Piece::King => {
                // Check if the move is a capture
                if enemy_occupancy & to != 0 {
                    return Self::new_with_flags(Piece::King, from, to, Self::CAPTURE);
                }

                // Check for castling (least frequent but must be prioritized due to overlap with quiet moves)
                if from == WHITE_KING_START_SQUARE {
                    if to == WHITE_KING_CASTLE_SQUARE {
                        return Self::new_with_flags(Piece::King, from, to, Self::CASTLE_KING);
                    }

                    if to == WHITE_QUEEN_CASTLE_SQUARE {
                        return Self::new_with_flags(Piece::King, from, to, Self::CASTLE_QUEEN);
                    }
                }

                // Regular king move
                return Self::new_with_flags(Piece::King, from, to, Self::QUIET_MOVE);
            }

            Piece::Knight => {
                // Check if it's a capture move
                if enemy_occupancy & to != 0 {
                    return Self::new_with_flags(Piece::Knight, from, to, Self::CAPTURE);
                } else {
                    return Self::new_with_flags(Piece::Knight, from, to, Self::QUIET_MOVE);
                }
            },
            Piece::Rook => {
                // Check if it's a capture move
                if enemy_occupancy & to != 0 {
                    return Self::new_with_flags(Piece::Rook, from, to, Self::CAPTURE);
                } else {
                    return Self::new_with_flags(Piece::Rook, from, to, Self::QUIET_MOVE);
                }
            }
            Piece::Bishop => {
                // Check if it's a capture move
                if enemy_occupancy & to != 0 {
                    return Self::new_with_flags(Piece::Bishop, from, to, Self::CAPTURE);
                } else {
                    return Self::new_with_flags(Piece::Bishop, from, to, Self::QUIET_MOVE);
                }
            },
            Piece::Queen => {
                // Check if it's a capture move
                if enemy_occupancy & to != 0 {
                    return Self::new_with_flags(Piece::Queen, from, to, Self::CAPTURE);
                } else {
                    return Self::new_with_flags(Piece::Queen, from, to, Self::QUIET_MOVE);
                }
            },
        }
    }
 
    // Getters for extracting information
    pub fn get_from_square(&self) -> usize {
        (self.from & self.from.wrapping_neg()) as usize
    }

    pub fn get_to_square(&self) -> usize {
        (self.to & self.to.wrapping_neg()) as usize
    }

    pub fn get_flags(&self) -> u32 {
        self.flags
    }
}

// #[derive(Debug, Clone, Copy)]
// pub struct Move {
//     move_code: u32,  // 32-bit move encoding
// }

// impl Move {
//     const FROM_MASK: u32 = 0b111111;   // 6 bits for the from-square (0-63)
//     const TO_MASK: u32 = 0b111111 << 6;   // 6 bits for the to-square (0-63)
//     const FLAG_MASK: u32 = 0b1111 << 12;  // 4 bits for the move flag (e.g., promotion, castling)

//     // Define some example flags for move types (more can be added as needed)
//     const QUIET_MOVE: u32 = 0;
//     const DOUBLE_PAWN_PUSH: u32 = 1;
//     const CASTLE_KING: u32 = 2;
//     const CASTLE_QUEEN: u32 = 3;
//     const CAPTURE: u32 = 4;
//     const EP_CAPTURE: u32 = 5;

//     // Constructor to create a move with flags
//     pub fn new_with_flags(from_square: usize, to_square: usize, flags: u32) -> Self {
//         let move_code = ((from_square as u32) & Self::FROM_MASK)
//                         | (((to_square as u32) << 6) & Self::TO_MASK)
//                         | ((flags << 12) & Self::FLAG_MASK);
//         Move { move_code }
//     }

//     // Constructor to create a move
//     pub fn new(cb: &Chessboard, piece: Piece, from: u64, to: u64) -> Self {
//         let enemy_occupancy = cb.get_enemy_color_occupancy();

//         match piece {
//             Piece::Pawn => {
//                 // Check if it's a capture move
//                 if enemy_occupancy & (1u64 << to) != 0 {
//                     return Self::new_with_flags(from, to, Self::CAPTURE);
//                 }

//                 // Check if it's an en passant capture
//                 if cb.get_en_passant_square() == to {
//                     return Self::new_with_flags(from, to, Self::EP_CAPTURE);
//                 }

//                 // Check if it's a double pawn push
//                 if from as i32 - to as i32 == 16 {
//                     return Self::new_with_flags(from, to, Self::DOUBLE_PAWN_PUSH);
//                 }
//             }
//             Piece::King => {
//                 // Check if it's a king-side castle
//                 if from == 4 && to == 6 {
//                     return Self::new_with_flags(from, to, Self::CASTLE_KING);
//                 }

//                 // Check if it's a queen-side castle
//                 if from == 4 && to == 2 {
//                     return Self::new_with_flags(from, to, Self::CASTLE_QUEEN);
//                 }
//             }
//             Piece::Knight => todo!(),
//             Piece::Rook => {
//                 // Check if it's a capture move
//                 if enemy_occupancy & to != 0 {
//                     return Self::new_with_flags(from.trailing_zeros(), to.trailing_zeros(), Self::CAPTURE);
//                 } else {
//                     return Self::new_with_flags(from, to, Self::QUIET_MOVE);
//                 }
//             },
//             Piece::Bishop => todo!(),
//             Piece::Queen => todo!(),
//         }

//         let move_code = ((from as u32) & Self::FROM_MASK)
//                         | (((to as u32) << 6) & Self::TO_MASK)
//                         | (((flags) << 12) & Self::FLAG_MASK);
//         Move { move_code }
//     }

//     // Getters for extracting information
//     pub fn get_from(&self) -> u8 {
//         (self.move_code & Self::FROM_MASK) as u8
//     }

//     pub fn get_to(&self) -> u8 {
//         ((self.move_code & Self::TO_MASK) >> 6) as u8
//     }

//     pub fn get_flags(&self) -> u32 {
//         (self.move_code & Self::FLAG_MASK) >> 12
//     }

//     // Example: Check if it's a capture move
//     pub fn is_capture(&self) -> bool {
//         self.get_flags() == Self::CAPTURE
//     }
// }
