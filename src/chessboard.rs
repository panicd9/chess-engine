//      A    B    C    D    E    F    G    H
//    +----+----+----+----+----+----+----+----+
//  8 | 56 | 57 | 58 | 59 | 60 | 61 | 62 | 63 |  8th rank
//    +----+----+----+----+----+----+----+----+
//  7 | 48 | 49 | 50 | 51 | 52 | 53 | 54 | 55 |  7th rank
//    +----+----+----+----+----+----+----+----+
//  6 | 40 | 41 | 42 | 43 | 44 | 45 | 46 | 47 |  6th rank
//    +----+----+----+----+----+----+----+----+
//  5 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 |  5th rank
//    +----+----+----+----+----+----+----+----+
//  4 | 24 | 25 | 26 | 27 | 28 | 29 | 30 | 31 |  4th rank
//    +----+----+----+----+----+----+----+----+
//  3 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 |  3rd rank
//    +----+----+----+----+----+----+----+----+
//  2 |  8 |  9 | 10 | 11 | 12 | 13 | 14 | 15 |  2nd rank
//    +----+----+----+----+----+----+----+----+
//  1 |  0 |  1 |  2 |  3 |  4 |  5 |  6 |  7 |  1st rank
//    +----+----+----+----+----+----+----+----+
//       A    B    C    D    E    F    G    H   - file(s)

use core::net;

use file_masks::{FILE_A, FILE_H};

use crate::{
    display::{self, display_board},
    move_gen::{
        move_gen_bishop::{all_bishops_attacks, single_bishop_attacks},
        move_gen_king::king_attacks,
        move_gen_knight::knight_attacks_from_single_knight_bitboard,
        move_gen_pawn::{single_black_pawn_attacks, single_white_pawn_attacks},
        move_gen_queen::{all_queens_attacks, single_queen_attacks},
        move_gen_rook::{all_rooks_attacks, single_rook_attacks},
    },
    utils::{BLACK_ROOKS_MASK, BLACK_ROOK_KINGSIDE, WHITE_ROOKS_MASK, WHITE_ROOK_KINGSIDE},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    White,
    Black,
}

// Board size constants
const BOARD_SIZE: usize = 64;

pub type Bitboard = u64;
pub type SingletonBitboard = u64;
pub type Square = usize;

pub(crate) mod rank_masks {
    pub const RANK_1: u64 = 0x00000000000000FF;
    pub const RANK_2: u64 = 0x000000000000FF00;
    pub const RANK_3: u64 = 0x0000000000FF0000;
    pub const RANK_4: u64 = 0x00000000FF000000;
    pub const RANK_5: u64 = 0x000000FF00000000;
    pub const RANK_6: u64 = 0x0000FF0000000000;
    pub const RANK_7: u64 = 0x00FF000000000000;
    pub const RANK_8: u64 = 0xFF00000000000000;
}

pub(crate) mod file_masks {
    pub const FILE_A: u64 = 0x0101010101010101;
    pub const FILE_B: u64 = 0x0202020202020202;
    pub const FILE_C: u64 = 0x0404040404040404;
    pub const FILE_D: u64 = 0x0808080808080808;
    pub const FILE_E: u64 = 0x1010101010101010;
    pub const FILE_F: u64 = 0x2020202020202020;
    pub const FILE_G: u64 = 0x4040404040404040;
    pub const FILE_H: u64 = 0x8080808080808080;
}

pub(crate) const WHITE_KING_START_SQUARE: u64 = 0x0000000000000010;
pub(crate) const WHITE_QUEEN_CASTLE_SQUARE: u64 = 0x0000000000000004;
pub(crate) const WHITE_KING_CASTLE_SQUARE: u64 = 0x0000000000000040;
pub const WHITE_CASTLE_KINGSIDE_ROOK_END_SQUARE: u64 = 0x20;
pub const WHITE_CASTLE_QUEENSIDE_ROOK_END_SQUARE: u64 = 0x8;
pub const WHITE_CASTLE_KINGSIDE_ROOK_MASK: u64 = 0xa0;
pub const WHITE_CASTLE_QUEENSIDE_ROOK_MASK: u64 = 0x9;

pub const WHITE_KINGSIDE_CASTLE_OCCUPANCY_MASK: u64 = 0xf0;
pub const WHITE_QUEENSIDE_CASTLE_OCCUPANCY_MASK: u64 = 0x1d;

pub const BLACK_KINGSIDE_CASTLE_OCCUPANCY_MASK: u64 = 0xf000000000000000;
pub const BLACK_QUEENSIDE_CASTLE_OCCUPANCY_MASK: u64 = 0x1d00000000000000;

pub(crate) const BLACK_KING_START_SQUARE: u64 = 0x1000000000000000;
pub(crate) const BLACK_QUEEN_CASTLE_SQURE: u64 = 0x0400000000000000;
pub(crate) const BLACK_KING_CASTLE_SQUARE: u64 = 0x4000000000000000;
pub const BLACK_CASTLE_KINGSIDE_ROOK_END_SQUARE: u64 = 0x2000000000000000;
pub const BLACK_CASTLE_QUEENSIDE_ROOK_END_SQUARE: u64 = 0x800000000000000;
pub const BLACK_CASTLE_KINGSIDE_ROOK_MASK: u64 = 0xa000000000000000;
pub const BLACK_CASTLE_QUEENSIDE_ROOK_MASK: u64 = 0x900000000000000;

// Square masks
pub(crate) const RANK_MASKS: [u64; 8] = [
    0x00000000000000FF,
    0x000000000000FF00,
    0x0000000000FF0000,
    0x00000000FF000000,
    0x000000FF00000000,
    0x0000FF0000000000,
    0x00FF000000000000,
    0xFF00000000000000,
];

pub(crate) const FILE_MASKS: [u64; 8] = [
    0x0101010101010101,
    0x0202020202020202,
    0x0404040404040404,
    0x0808080808080808,
    0x1010101010101010,
    0x2020202020202020,
    0x4040404040404040,
    0x8080808080808080,
];

#[derive(Debug, Copy)]
pub struct Chessboard {
    pub white_pawns: u64,
    pub black_pawns: u64,
    pub white_knights: u64,
    pub black_knights: u64,
    pub white_bishops: u64,
    pub black_bishops: u64,
    pub white_rooks: u64,
    pub black_rooks: u64,
    pub white_queens: u64,
    pub black_queens: u64,
    pub white_king: u64,
    pub black_king: u64,

    pub en_passant: SingletonBitboard,

    pub white_occupancy: u64,
    pub black_occupancy: u64,
    pub occupancy: u64,

    pub white_can_castle_king_side: bool,
    pub white_can_castle_queen_side: bool,
    pub black_can_castle_king_side: bool,
    pub black_can_castle_queen_side: bool,
    pub side_to_move: Color,

    // New fields
    pub halfmove_clock: u32, // Number of half-moves since the last pawn move or capture
    pub fullmove_counter: u32, // Number of full moves in the game
}

impl Clone for Chessboard {
    fn clone(&self) -> Self {
        Chessboard {
            white_pawns: self.white_pawns,
            black_pawns: self.black_pawns,
            white_knights: self.white_knights,
            black_knights: self.black_knights,
            white_bishops: self.white_bishops,
            black_bishops: self.black_bishops,
            white_rooks: self.white_rooks,
            black_rooks: self.black_rooks,
            white_queens: self.white_queens,
            black_queens: self.black_queens,
            white_king: self.white_king,
            black_king: self.black_king,

            white_occupancy: self.white_occupancy,
            black_occupancy: self.black_occupancy,
            occupancy: self.occupancy,

            en_passant: 0x0, // Clear en_passant

            white_can_castle_king_side: self.white_can_castle_king_side,
            white_can_castle_queen_side: self.white_can_castle_queen_side,
            black_can_castle_king_side: self.black_can_castle_king_side,
            black_can_castle_queen_side: self.black_can_castle_queen_side,
            side_to_move: self.side_to_move,

            halfmove_clock: self.halfmove_clock,
            fullmove_counter: self.fullmove_counter,
        }
    }
}

impl Chessboard {
    pub fn new_initial_board() -> Self {
        Self {
            white_pawns: 0x000000000000FF00,
            black_pawns: 0x00FF000000000000,
            white_knights: 0x0000000000000042,
            black_knights: 0x4200000000000000,
            white_bishops: 0x0000000000000024,
            black_bishops: 0x2400000000000000,
            white_rooks: 0x0000000000000081,
            black_rooks: 0x8100000000000000,
            white_queens: 0x0000000000000008,
            black_queens: 0x0800000000000000,
            white_king: 0x0000000000000010,
            black_king: 0x1000000000000000,

            white_occupancy: 0x000000000000FFFF,
            black_occupancy: 0xFFFF000000000000,
            occupancy: 0xFFFF00000000FFFF,

            en_passant: 0x0,

            white_can_castle_king_side: true,
            white_can_castle_queen_side: true,
            black_can_castle_king_side: true,
            black_can_castle_queen_side: true,
            side_to_move: Color::White,

            halfmove_clock: 0,
            fullmove_counter: 1,
        }
    }

    pub fn get_occupancy(&self) -> u64 {
        self.white_pawns
            | self.black_pawns
            | self.white_knights
            | self.black_knights
            | self.white_bishops
            | self.black_bishops
            | self.white_rooks
            | self.black_rooks
            | self.white_queens
            | self.black_queens
            | self.white_king
            | self.black_king

        // self.white_occupancy | self.black_occupancy
    }

    pub fn get_white_occupancy(&self) -> u64 {
        self.white_pawns
            | self.white_knights
            | self.white_bishops
            | self.white_rooks
            | self.white_queens
            | self.white_king

        // self.white_occupancy
    }

    pub fn get_black_occupancy(&self) -> u64 {
        self.black_pawns
            | self.black_knights
            | self.black_bishops
            | self.black_rooks
            | self.black_queens
            | self.black_king

        // self.black_occupancy
    }

    pub fn from_fen(fen: &str) -> Result<Self, String> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            return Err("Invalid FEN: FEN must have exactly 6 fields".to_string());
        }

        // Parse piece placement
        let board = parts[0];
        let mut white_pawns = 0u64;
        let mut black_pawns = 0u64;
        let mut white_knights = 0u64;
        let mut black_knights = 0u64;
        let mut white_bishops = 0u64;
        let mut black_bishops = 0u64;
        let mut white_rooks = 0u64;
        let mut black_rooks = 0u64;
        let mut white_queens = 0u64;
        let mut black_queens = 0u64;
        let mut white_king = 0u64;
        let mut black_king = 0u64;

        let mut rank = 7;
        let mut file = 0;
        for c in board.chars() {
            match c {
                'P' => white_pawns |= 1 << (rank * 8 + file),
                'p' => black_pawns |= 1 << (rank * 8 + file),
                'N' => white_knights |= 1 << (rank * 8 + file),
                'n' => black_knights |= 1 << (rank * 8 + file),
                'B' => white_bishops |= 1 << (rank * 8 + file),
                'b' => black_bishops |= 1 << (rank * 8 + file),
                'R' => white_rooks |= 1 << (rank * 8 + file),
                'r' => black_rooks |= 1 << (rank * 8 + file),
                'Q' => white_queens |= 1 << (rank * 8 + file),
                'q' => black_queens |= 1 << (rank * 8 + file),
                'K' => white_king |= 1 << (rank * 8 + file),
                'k' => black_king |= 1 << (rank * 8 + file),
                '/' => {
                    rank -= 1;
                    file = 0;
                    continue;
                }
                '1'..='8' => {
                    file += c.to_digit(10).unwrap() as usize;
                    continue;
                }
                _ => return Err("Invalid character in FEN board".to_string()),
            }
            file += 1;
        }

        // Parse side to move
        let side_to_move = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err("Invalid side to move".to_string()),
        };

        // Parse castling rights
        let mut white_can_castle_king_side = false;
        let mut white_can_castle_queen_side = false;
        let mut black_can_castle_king_side = false;
        let mut black_can_castle_queen_side = false;

        for c in parts[2].chars() {
            match c {
                'K' => white_can_castle_king_side = true,
                'Q' => white_can_castle_queen_side = true,
                'k' => black_can_castle_king_side = true,
                'q' => black_can_castle_queen_side = true,
                '-' => break,
                _ => return Err("Invalid castling rights".to_string()),
            }
        }

        // Parse en passant target square
        let en_passant = if parts[3] != "-" {
            let ep_file = parts[3].chars().next().unwrap() as u8 - b'a';
            let ep_rank = parts[3].chars().nth(1).unwrap() as u8 - b'1';
            1 << (ep_rank * 8 + ep_file)
        } else {
            0
        };

        // Parse halfmove clock
        let halfmove_clock: u32 = parts[4]
            .parse()
            .map_err(|_| "Invalid halfmove clock".to_string())?;

        // Parse fullmove counter
        let fullmove_counter: u32 = parts[5]
            .parse()
            .map_err(|_| "Invalid fullmove counter".to_string())?;

        let white_occupancy =
            white_pawns | white_knights | white_bishops | white_rooks | white_queens | white_king;

        let black_occupancy =
            black_pawns | black_knights | black_bishops | black_rooks | black_queens | black_king;

        let occupancy = white_occupancy | black_occupancy;

        Ok(Self {
            white_pawns,
            black_pawns,
            white_knights,
            black_knights,
            white_bishops,
            black_bishops,
            white_rooks,
            black_rooks,
            white_queens,
            black_queens,
            white_king,
            black_king,
            en_passant,
            white_occupancy,
            black_occupancy,
            occupancy,
            white_can_castle_king_side,
            white_can_castle_queen_side,
            black_can_castle_king_side,
            black_can_castle_queen_side,
            side_to_move,
            halfmove_clock,
            fullmove_counter,
        })
    }

    // pub fn get_white_attacks(&self) -> Bitboard {
    //     let white_pawn_attacks = self.white_pawns_attacks();
    //     let white_knight_attacks = self.white_knights_attacks();
    //     let white_rook_attacks = self.white_rooks_attacks();
    //     let white_bishop_attacks = self.white_bishops_attacks();
    //     let white_queen_attacks = self.white_queens_attacks();
    //     let white_king_attacks = self.white_king_attacks();

    //     white_pawn_attacks
    //         | white_knight_attacks
    //         | white_bishop_attacks
    //         | white_rook_attacks
    //         | white_queen_attacks
    //         | white_king_attacks
    // }

    pub fn get_moving_color_occupancy(&self) -> u64 {
        match self.side_to_move {
            Color::White => self.get_white_occupancy(),
            Color::Black => self.get_black_occupancy(),
        }
    }

    pub fn get_enemy_color_occupancy(&self) -> u64 {
        match self.side_to_move {
            Color::White => self.get_black_occupancy(),
            Color::Black => self.get_white_occupancy(),
        }
    }

    pub(crate) fn get_en_passant_bitboard(&self) -> SingletonBitboard {
        self.en_passant
    }

    pub fn display_board_string(&self) -> String {
        display::display_board_string(self)
    }

    pub fn change_side_to_move(&mut self) {
        self.side_to_move = match self.side_to_move {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    pub fn capture_black_piece(&mut self, target: SingletonBitboard) {
        self.black_pawns &= !target;
        self.black_knights &= !target;
        self.black_bishops &= !target;
        self.black_rooks &= !target;
        self.black_queens &= !target;
        self.black_king &= !target;
        self.black_occupancy &= !target;
    }

    pub fn capture_white_piece(&mut self, target: SingletonBitboard) {
        self.white_pawns &= !target;
        self.white_knights &= !target;
        self.white_bishops &= !target;
        self.white_rooks &= !target;
        self.white_queens &= !target;
        self.white_king &= !target;
        self.white_occupancy &= !target;
    }

    pub fn make_white_rook_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // TODO: BENCHMARH THIS
        // self.white_rooks ^= from | to;
        // self.white_occupancy ^= from | to;

        // Move the rook
        new_chessboard.white_rooks &= !from;
        new_chessboard.white_rooks |= to;

        new_chessboard.white_occupancy &= !from;
        new_chessboard.white_occupancy |= to;

        // Rooks is moved rarely from initial position so we can skip both checks frequently
        // TODO: BENCHMARK DIFFERENCE
        if from & WHITE_ROOKS_MASK != 0 {
            if from == WHITE_ROOK_KINGSIDE {
                new_chessboard.white_can_castle_king_side = false;
            } else {
                new_chessboard.white_can_castle_queen_side = false;
            }
        }

        // Capture enemy piece if it exists
        let occupancy = self.get_black_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_black_piece(to);
        }

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_rook_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the rook
        new_chessboard.black_rooks &= !from;
        new_chessboard.black_rooks |= to;

        new_chessboard.black_occupancy &= !from;
        new_chessboard.black_occupancy |= to;

        // Rooks is moved rarely from initial position so we can skip both checks frequently
        if from & BLACK_ROOKS_MASK != 0 {
            if from == BLACK_ROOK_KINGSIDE {
                new_chessboard.black_can_castle_king_side = false;
            } else {
                new_chessboard.black_can_castle_queen_side = false;
            }
        }

        // Capture enemy piece if it exists
        let occupancy = self.get_white_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_white_piece(to);
        }

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }

    pub(crate) fn make_white_knight_move(&self, from: u64, to: u64) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the knight
        new_chessboard.white_knights &= !from;
        new_chessboard.white_knights |= to;

        new_chessboard.white_occupancy &= !from;
        new_chessboard.white_occupancy |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_black_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_black_piece(to);
        }

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_knight_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the knight
        new_chessboard.black_knights &= !from;
        new_chessboard.black_knights |= to;

        new_chessboard.black_occupancy &= !from;
        new_chessboard.black_occupancy |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_white_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_white_piece(to);
        }

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }

    pub(crate) fn make_white_king_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the king
        new_chessboard.white_king &= !from;
        new_chessboard.white_king |= to;

        new_chessboard.white_occupancy &= !from;
        new_chessboard.white_occupancy |= to;

        new_chessboard.white_can_castle_king_side = false;
        new_chessboard.white_can_castle_queen_side = false;

        // Capture enemy piece if it exists
        let occupancy = self.get_black_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_black_piece(to);
        }

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub(crate) fn make_black_king_move(&self, from: u64, to: u64) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the king
        new_chessboard.black_king &= !from;
        new_chessboard.black_king |= to;

        new_chessboard.black_occupancy &= !from;
        new_chessboard.black_occupancy |= to;

        new_chessboard.black_can_castle_king_side = false;
        new_chessboard.black_can_castle_queen_side = false;

        // Capture enemy piece if it exists
        let occupancy = self.get_white_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_white_piece(to);
        }

        new_chessboard.side_to_move = Color::White;

        // display_board(&new_chessboard);
        // println!("\n\n");
        new_chessboard
    }

    pub(crate) fn make_white_bishop_move(&self, from: u64, to: u64) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the bishop
        new_chessboard.white_bishops &= !from;
        new_chessboard.white_bishops |= to;

        new_chessboard.white_occupancy &= !from;
        new_chessboard.white_occupancy |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_black_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_black_piece(to);
        }

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_bishop_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the bishop
        new_chessboard.black_bishops &= !from;
        new_chessboard.black_bishops |= to;

        new_chessboard.black_occupancy &= !from;
        new_chessboard.black_occupancy |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_white_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_white_piece(to);
        }

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }

    pub fn make_white_pawn_forward_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the pawn
        new_chessboard.white_pawns &= !from;
        new_chessboard.white_pawns |= to;

        new_chessboard.white_occupancy &= !from;
        new_chessboard.white_occupancy |= to;

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_pawn_forward_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the pawn
        new_chessboard.black_pawns &= !from;
        new_chessboard.black_pawns |= to;

        new_chessboard.black_occupancy &= !from;
        new_chessboard.black_occupancy |= to;

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }

    pub fn make_white_pawn_double_forward_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the pawn
        new_chessboard.white_pawns &= !from;
        new_chessboard.white_pawns |= to;

        new_chessboard.white_occupancy &= !from;
        new_chessboard.white_occupancy |= to;

        new_chessboard.en_passant = to >> 8;

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_pawn_double_forward_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the pawn
        new_chessboard.black_pawns &= !from;
        new_chessboard.black_pawns |= to;

        new_chessboard.black_occupancy &= !from;
        new_chessboard.black_occupancy |= to;

        new_chessboard.en_passant = to << 8;

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }

    pub fn make_white_pawn_capture_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the pawn
        new_chessboard.white_pawns &= !from;
        new_chessboard.white_pawns |= to;

        new_chessboard.white_occupancy &= !from;
        new_chessboard.white_occupancy |= to;

        // Capture enemy piece
        new_chessboard.capture_black_piece(to);

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_pawn_capture_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the pawn
        new_chessboard.black_pawns &= !from;
        new_chessboard.black_pawns |= to;

        new_chessboard.black_occupancy &= !from;
        new_chessboard.black_occupancy |= to;

        // Capture enemy piece
        new_chessboard.capture_white_piece(to);

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }

    pub fn make_all_white_pawn_promotion_moves(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> [Chessboard; 4] {
        let mut new_chessboards = [self.clone(); 4];

        // Promote the pawn to a queen
        new_chessboards[0].white_pawns &= !from;
        new_chessboards[0].white_queens |= to;

        new_chessboards[0].white_occupancy |= to;
        new_chessboards[0].side_to_move = Color::Black;

        // Promote the pawn to a knight
        new_chessboards[1].white_pawns &= !from;
        new_chessboards[1].white_knights |= to;

        new_chessboards[1].white_occupancy |= to;
        new_chessboards[1].side_to_move = Color::Black;

        // Promote the pawn to a bishop
        new_chessboards[2].white_pawns &= !from;
        new_chessboards[2].white_bishops |= to;

        new_chessboards[2].white_occupancy |= to;
        new_chessboards[2].side_to_move = Color::Black;

        // Promote the pawn to a rook
        new_chessboards[3].white_pawns &= !from;
        new_chessboards[3].white_rooks |= to;

        new_chessboards[3].white_occupancy |= to;
        new_chessboards[3].side_to_move = Color::Black;

        new_chessboards
    }

    pub fn make_all_black_pawn_promotion_moves(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> [Chessboard; 4] {
        let mut new_chessboards = [Chessboard::new_initial_board(); 4];

        // Promote the pawn to a queen
        new_chessboards[0].black_pawns &= !from;
        new_chessboards[0].black_queens |= to;

        new_chessboards[0].black_occupancy |= to;
        new_chessboards[0].side_to_move = Color::White;

        // Promote the pawn to a knight
        new_chessboards[1].black_pawns &= !from;
        new_chessboards[1].black_knights |= to;

        new_chessboards[1].black_occupancy |= to;
        new_chessboards[1].side_to_move = Color::White;

        // Promote the pawn to a bishop
        new_chessboards[2].black_pawns &= !from;
        new_chessboards[2].black_bishops |= to;

        new_chessboards[2].black_occupancy |= to;
        new_chessboards[2].side_to_move = Color::White;

        // Promote the pawn to a rook
        new_chessboards[3].black_pawns &= !from;
        new_chessboards[3].black_rooks |= to;

        new_chessboards[3].black_occupancy |= to;
        new_chessboards[3].side_to_move = Color::White;

        new_chessboards
    }

    pub fn make_all_white_pawn_capture_promotion_moves(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> [Chessboard; 4] {
        let mut new_chessboards = [Chessboard::new_initial_board(); 4];

        // Promote the pawn to a queen
        new_chessboards[0].white_pawns &= !from;
        new_chessboards[0].white_queens |= to;

        new_chessboards[0].white_occupancy |= to;
        new_chessboards[0].capture_black_piece(to);
        new_chessboards[0].side_to_move = Color::Black;

        // Promote the pawn to a knight
        new_chessboards[1].white_pawns &= !from;
        new_chessboards[1].white_knights |= to;

        new_chessboards[1].white_occupancy |= to;
        new_chessboards[1].capture_black_piece(to);
        new_chessboards[1].side_to_move = Color::Black;

        // Promote the pawn to a bishop
        new_chessboards[2].white_pawns &= !from;
        new_chessboards[2].white_bishops |= to;

        new_chessboards[2].white_occupancy |= to;
        new_chessboards[2].capture_black_piece(to);
        new_chessboards[2].side_to_move = Color::Black;

        // Promote the pawn to a rook
        new_chessboards[3].white_pawns &= !from;
        new_chessboards[3].white_rooks |= to;

        new_chessboards[3].white_occupancy |= to;
        new_chessboards[3].capture_black_piece(to);
        new_chessboards[3].side_to_move = Color::Black;

        new_chessboards
    }

    pub fn make_all_black_pawn_capture_promotion_moves(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> [Chessboard; 4] {
        let mut new_chessboards = [Chessboard::new_initial_board(); 4];

        // Promote the pawn to a queen
        new_chessboards[0].black_pawns &= !from;
        new_chessboards[0].black_queens |= to;

        new_chessboards[0].black_occupancy |= to;
        new_chessboards[0].capture_white_piece(to);
        new_chessboards[0].side_to_move = Color::White;

        // Promote the pawn to a knight
        new_chessboards[1].black_pawns &= !from;
        new_chessboards[1].black_knights |= to;

        new_chessboards[1].black_occupancy |= to;
        new_chessboards[1].capture_white_piece(to);
        new_chessboards[1].side_to_move = Color::White;

        // Promote the pawn to a bishop
        new_chessboards[2].black_pawns &= !from;
        new_chessboards[2].black_bishops |= to;

        new_chessboards[2].black_occupancy |= to;
        new_chessboards[2].capture_white_piece(to);
        new_chessboards[2].side_to_move = Color::White;

        // Promote the pawn to a rook
        new_chessboards[3].black_pawns &= !from;
        new_chessboards[3].black_rooks |= to;

        new_chessboards[3].black_occupancy |= to;
        new_chessboards[3].capture_white_piece(to);
        new_chessboards[3].side_to_move = Color::White;

        new_chessboards
    }

    pub fn make_white_queen_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the queen
        new_chessboard.white_queens &= !from;
        new_chessboard.white_queens |= to;

        new_chessboard.white_occupancy &= !from;
        new_chessboard.white_occupancy |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_black_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_black_piece(to);
        }

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_queen_move(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the queen
        new_chessboard.black_queens &= !from;
        new_chessboard.black_queens |= to;

        new_chessboard.black_occupancy &= !from;
        new_chessboard.black_occupancy |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_white_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_white_piece(to);
        }

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }

    pub fn make_white_pawn_en_passant_capture(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the pawn
        new_chessboard.white_pawns &= !from;
        new_chessboard.white_pawns |= to;

        new_chessboard.white_occupancy &= !from;
        new_chessboard.white_occupancy |= to;

        // Capture the black pawn
        let caputred_pawn: SingletonBitboard = to >> 8;
        new_chessboard.black_pawns &= !caputred_pawn;
        new_chessboard.black_occupancy &= !caputred_pawn;

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_pawn_en_passant_capture(
        &self,
        from: SingletonBitboard,
        to: SingletonBitboard,
    ) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the pawn
        new_chessboard.black_pawns &= !from;
        new_chessboard.black_pawns |= to;

        new_chessboard.black_occupancy &= !from;
        new_chessboard.black_occupancy |= to;

        // Capture the white pawn
        let caputred_pawn: SingletonBitboard = to >> 8;
        new_chessboard.white_pawns &= !caputred_pawn;
        new_chessboard.white_occupancy &= !caputred_pawn;

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }

    pub fn make_white_kingside_castle(&self) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the king
        new_chessboard.white_king = WHITE_KING_CASTLE_SQUARE;

        // Move the rook
        new_chessboard.white_rooks ^= WHITE_CASTLE_KINGSIDE_ROOK_MASK;

        new_chessboard.white_occupancy ^= WHITE_KINGSIDE_CASTLE_OCCUPANCY_MASK;

        new_chessboard.white_can_castle_king_side = false;
        new_chessboard.white_can_castle_queen_side = false;

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_kingside_castle(&self) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the king
        new_chessboard.black_king = BLACK_KING_CASTLE_SQUARE;

        // Move the rook
        new_chessboard.black_rooks ^= BLACK_CASTLE_KINGSIDE_ROOK_MASK;

        new_chessboard.black_occupancy ^= BLACK_KINGSIDE_CASTLE_OCCUPANCY_MASK;

        new_chessboard.black_can_castle_king_side = false;
        new_chessboard.black_can_castle_queen_side = false;

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }

    pub fn make_white_queenside_castle(&self) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the king
        new_chessboard.white_king = WHITE_QUEEN_CASTLE_SQUARE;

        // Move the rook
        new_chessboard.white_rooks ^= WHITE_CASTLE_QUEENSIDE_ROOK_MASK;

        new_chessboard.white_occupancy ^= WHITE_QUEENSIDE_CASTLE_OCCUPANCY_MASK;

        new_chessboard.white_can_castle_king_side = false;
        new_chessboard.white_can_castle_queen_side = false;

        new_chessboard.side_to_move = Color::Black;
        new_chessboard
    }

    pub fn make_black_queenside_castle(&self) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the king
        new_chessboard.black_king = BLACK_QUEEN_CASTLE_SQURE;

        // Move the rook
        new_chessboard.black_rooks ^= BLACK_CASTLE_QUEENSIDE_ROOK_MASK;

        new_chessboard.black_occupancy ^= BLACK_QUEENSIDE_CASTLE_OCCUPANCY_MASK;

        new_chessboard.black_can_castle_king_side = false;
        new_chessboard.black_can_castle_queen_side = false;

        new_chessboard.side_to_move = Color::White;
        new_chessboard
    }
}

impl Chessboard {
    pub fn white_pawns_attacks(&self) -> Bitboard {
        // Calculate left and right attacks for all pawns
        let left_attacks = (self.white_pawns << 7) & !FILE_H;
        let right_attacks = (self.white_pawns << 9) & !FILE_A;

        // Combine the attacks
        left_attacks | right_attacks
    }

    pub fn black_pawn_attacks(&self) -> Bitboard {
        // Calculate left and right attacks for all pawns
        let left_attacks = (self.black_pawns >> 9) & !FILE_H;
        let right_attacks = (self.black_pawns >> 7) & !FILE_A;

        // Combine the attacks
        left_attacks | right_attacks
    }

    pub fn white_knights_attacks(&self) -> Bitboard {
        let mut combined_attacks = 0;
        let mut remaining_knights = self.white_knights;

        while remaining_knights != 0 {
            let single_knight = remaining_knights & remaining_knights.wrapping_neg(); // Extract LSB (single knight)
            combined_attacks |= knight_attacks_from_single_knight_bitboard(single_knight);
            remaining_knights &= remaining_knights - 1; // Remove LSB
        }

        combined_attacks
    }

    pub fn black_knights_attacks(&self) -> Bitboard {
        let mut combined_attacks = 0;
        let mut remaining_knights = self.black_knights;

        while remaining_knights != 0 {
            let single_knight = remaining_knights & remaining_knights.wrapping_neg(); // Extract LSB (single knight)
            combined_attacks |= knight_attacks_from_single_knight_bitboard(single_knight);
            remaining_knights &= remaining_knights - 1; // Remove LSB
        }

        combined_attacks
    }

    pub fn white_rooks_attacks(&self) -> Bitboard {
        all_rooks_attacks(self.get_occupancy(), self.white_rooks)
    }

    pub fn black_rooks_attacks(&self) -> Bitboard {
        all_rooks_attacks(self.get_occupancy(), self.black_rooks)
    }

    pub fn white_bishops_attacks(&self) -> Bitboard {
        all_bishops_attacks(self.get_occupancy(), self.white_bishops)
    }

    pub fn black_bishops_attacks(&self) -> Bitboard {
        all_bishops_attacks(self.get_occupancy(), self.black_bishops)
    }

    pub fn white_queens_attacks(&self) -> Bitboard {
        all_queens_attacks(self.get_occupancy(), self.white_queens)
    }

    pub fn black_queens_attacks(&self) -> Bitboard {
        all_queens_attacks(self.get_occupancy(), self.black_queens)
    }

    pub fn white_king_attacks(&self) -> Bitboard {
        king_attacks(self.white_king)
    }

    pub fn black_king_attacks(&self) -> Bitboard {
        king_attacks(self.black_king)
    }

    pub fn all_white_attacks(&self) -> Bitboard {
        self.white_pawns_attacks()
            | self.white_knights_attacks()
            | self.white_bishops_attacks()
            | self.white_rooks_attacks()
            | self.white_queens_attacks()
            | self.white_king_attacks()
    }

    pub fn all_black_attacks(&self) -> Bitboard {
        self.black_pawn_attacks()
            | self.black_knights_attacks()
            | self.black_bishops_attacks()
            | self.black_rooks_attacks()
            | self.black_queens_attacks()
            | self.black_king_attacks()
    }

    pub fn is_white_king_under_attack(&self) -> bool {
        let white_king_bitboard = self.white_king;
        let occupancy = self.get_occupancy();

        // Check for attacks from black bishops or queens (since they share similar attack patterns)
        let bishop_direction = single_bishop_attacks(occupancy, white_king_bitboard);
        if bishop_direction & (self.black_bishops | self.black_queens) != 0 {
            return true;
        }

        // Check for attacks from black rooks or queens (since they share similar attack patterns)
        let rook_direction = single_rook_attacks(occupancy, white_king_bitboard);
        if rook_direction & (self.black_rooks | self.black_queens) != 0 {
            return true;
        }

        // Check for attacks from black knights
        let knight_direction = knight_attacks_from_single_knight_bitboard(white_king_bitboard);
        if knight_direction & self.black_knights != 0 {
            return true;
        }

        // Check for attacks from black pawns (note: black pawns attack diagonally downwards)
        let pawn_direction = single_white_pawn_attacks(white_king_bitboard);
        if pawn_direction & self.black_pawns != 0 {
            return true;
        }

        // If no attack found, return false
        false
    }

    pub fn is_black_king_under_attack(&self) -> bool {
        let black_king_bitboard = self.black_king;
        let occupancy = self.get_occupancy();

        // Check for attacks from white bishops or queens (since they share similar attack patterns)
        if black_king_bitboard == 0 {
            display_board(self);
        }
        let bishop_direction = single_bishop_attacks(occupancy, black_king_bitboard);
        if bishop_direction & (self.white_bishops | self.white_queens) != 0 {
            return true;
        }

        // Check for attacks from white rooks or queens (since they share similar attack patterns)
        let rook_direction = single_rook_attacks(occupancy, black_king_bitboard);
        if rook_direction & (self.white_rooks | self.white_queens) != 0 {
            return true;
        }

        // Check for attacks from white knights
        let knight_direction = knight_attacks_from_single_knight_bitboard(black_king_bitboard);
        if knight_direction & self.white_knights != 0 {
            return true;
        }

        // Check for attacks from white pawns (note: white pawns attack diagonally upwards)
        let pawn_direction = single_black_pawn_attacks(black_king_bitboard);
        if pawn_direction & self.white_pawns != 0 {
            return true;
        }

        // If no attack found, return false
        false
    }
}

// impl Chessboard {
//     pub fn make_white_pawn_forward_move_if_legal(&self, from: SingletonBitboard, to: SingletonBitboard) -> Option<Chessboard> {

//     }
// }
