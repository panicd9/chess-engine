use crate::chessboard::Chessboard;

fn get_piece_char(cb: &Chessboard, square: usize) -> char {
    let pieces = [
        (cb.white_pawns, 'P'), (cb.black_pawns, 'p'),
        (cb.white_knights, 'N'), (cb.black_knights, 'n'),
        (cb.white_bishops, 'B'), (cb.black_bishops, 'b'),
        (cb.white_rooks, 'R'), (cb.black_rooks, 'r'),
        (cb.white_queens, 'Q'), (cb.black_queens, 'q'),
        (cb.white_king, 'K'), (cb.black_king, 'k'),
    ];

    for &(bitboard, piece) in &pieces {
        if (bitboard >> square) & 1 == 1 {
            return piece;
        }
    }
    '.'
}

pub fn display_board(cb: &Chessboard, bitboard: Option<u64>) {
    println!("");
    println!("    A  B  C  D  E  F  G  H");
    println!("  +------------------------+");
    for rank in (0..8).rev() {
        print!("{} |", rank + 1);
        for file in 0..8 {
            let square = rank * 8 + file;
            let piece_char = if let Some(bitboard) = bitboard {
                let bit = (bitboard >> square) & 1;
                if bit == 1 { '1' } else { '0' }
            } else {
                get_piece_char(cb, square)
            };
            print!(" {} ", piece_char);
        }
        println!("| {}", rank + 1);
    }
    println!("  +------------------------+");
    println!("    A  B  C  D  E  F  G  H");
}

pub fn display_board_string(cb: &Chessboard, bitboard: Option<u64>) -> String {
    let mut board_representation = String::new();
    board_representation.push_str("\n    A  B  C  D  E  F  G  H\n");
    board_representation.push_str("  +------------------------+\n");
    for rank in (0..8).rev() {
        board_representation.push_str(&format!("{} |", rank + 1));
        for file in 0..8 {
            let square = rank * 8 + file;
            let piece_char = if let Some(bitboard) = bitboard {
                let bit = (bitboard >> square) & 1;
                if bit == 1 { '1' } else { '0' }
            } else {
                get_piece_char(cb, square)
            };
            board_representation.push_str(&format!(" {} ", piece_char));
        }
        board_representation.push_str(&format!("| {}\n", rank + 1));
    }
    board_representation.push_str("  +------------------------+\n");
    board_representation.push_str("    A  B  C  D  E  F  G  H\n");
    board_representation
}

pub fn display_bitboard(bitboard: u64) {
    println!("");
    
    for rank in (0..8).rev() {
        for file in 0..8 {
            let square = rank * 8 + file;
            let bit = (bitboard >> square) & 1;
            print!("{} ", bit);
        }
        println!();
    }
}

