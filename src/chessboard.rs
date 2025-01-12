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

use crate::display;

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

pub(crate) const WHITE_KING_START_SQUARE:   u64 = 0x0000000000000010;
pub(crate) const WHITE_QUEEN_CASTLE_SQURE:  u64 = 0x0000000000000004;
pub(crate) const WHITE_KING_CASTLE_SQURE:   u64 = 0x0000000000000040;

pub(crate) const BLACK_KING_START_SQUARE:   u64 = 0x1000000000000000;
pub(crate) const BLACK_QUEEN_CASTLE_SQURE:  u64 = 0x0400000000000000;
pub(crate) const BLACK_KING_CASTLE_SQURE:   u64 = 0x4000000000000000;


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

#[derive(Clone)]
pub(crate) struct Chessboard {
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

    pub side_to_move: Color,
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

            side_to_move: Color::White,
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
    }

    pub fn get_white_occupancy(&self) -> u64 {
        self.white_pawns
            | self.white_knights
            | self.white_bishops
            | self.white_rooks
            | self.white_queens
            | self.white_king
    }

    pub fn get_black_occupancy(&self) -> u64 {
        self.black_pawns
            | self.black_knights
            | self.black_bishops
            | self.black_rooks
            | self.black_queens
            | self.black_king
    }

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
    
    pub(crate) fn get_en_passant_bitboard(&self) -> u64 {
        todo!()
    }

    pub fn display_board_string(&self) -> String{
        display::display_board_string(self, None)
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
    }

    pub fn capture_white_piece(&mut self, target: SingletonBitboard) {
        self.white_pawns &= !target;
        self.white_knights &= !target;
        self.white_bishops &= !target;
        self.white_rooks &= !target;
        self.white_queens &= !target;
        self.white_king &= !target;
    }

    pub fn make_white_rook_move(&self, from: SingletonBitboard, to: SingletonBitboard) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the rook
        new_chessboard.white_rooks &= !from;
        new_chessboard.white_rooks |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_black_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_black_piece(to);
        }

        new_chessboard.change_side_to_move();
        new_chessboard
    }

    pub fn make_black_rook_move(&self, from: SingletonBitboard, to: SingletonBitboard) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the rook
        new_chessboard.black_rooks &= !from;
        new_chessboard.black_rooks |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_white_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_white_piece(to);
        }

        new_chessboard.change_side_to_move();
        new_chessboard
    }
    
    pub(crate) fn make_white_knight_move(&self, from: u64, to: u64) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the knight
        new_chessboard.white_knights &= !from;
        new_chessboard.white_knights |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_black_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_black_piece(to);
        }

        new_chessboard.change_side_to_move();
        new_chessboard
    }

    pub fn make_black_knight_move(&self, from: SingletonBitboard, to: SingletonBitboard) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the knight
        new_chessboard.black_knights &= !from;
        new_chessboard.black_knights |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_white_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_white_piece(to);
        }

        new_chessboard.change_side_to_move();
        new_chessboard
    }
    
    pub(crate) fn make_white_king_move(&self, single_king_bitboard: u64, single_attack_bitboard: u64) -> Chessboard {
        todo!()
    }
    
    pub(crate) fn make_black_king_move(&self, single_king_bitboard: u64, single_attack_bitboard: u64) -> Chessboard {
        todo!()
    }
    
    pub(crate) fn make_white_bishop_move(&self, single_bishop_bitboard: u64, single_attack_bitboard: u64) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the bishop
        new_chessboard.white_bishops &= !single_bishop_bitboard;
        new_chessboard.white_bishops |= single_attack_bitboard;

        // Capture enemy piece if it exists
        let occupancy = self.get_black_occupancy();
        if occupancy & single_attack_bitboard != 0 {
            new_chessboard.capture_black_piece(single_attack_bitboard);
        }

        new_chessboard.change_side_to_move();
        new_chessboard
    }

    pub fn make_black_bishop_move(&self, from: SingletonBitboard, to: SingletonBitboard) -> Chessboard {
        let mut new_chessboard = self.clone();

        // Move the bishop
        new_chessboard.black_bishops &= !from;
        new_chessboard.black_bishops |= to;

        // Capture enemy piece if it exists
        let occupancy = self.get_occupancy();
        if occupancy & to != 0 {
            new_chessboard.capture_white_piece(to);
        }

        new_chessboard.change_side_to_move();
        new_chessboard
    }
}
