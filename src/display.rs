use colored::Colorize;

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

pub fn display_board(cb: &Chessboard) {
    println!();
    println!("    A  B  C  D  E  F  G  H");
    println!("  +------------------------+");

    for rank in (0..8).rev() {
        print!("{} |", rank + 1); // Print the rank number
        for file in 0..8 {
            let square = rank * 8 + file; // Calculate the square index
            let piece_char = get_piece_char(cb, square); // Get the piece character at that square
            print!(" {} ", piece_char); // Print the piece or empty space
        }
        println!("| {}", rank + 1); // Print the rank number again at the end of the row
    }

    println!("  +------------------------+");
    println!("    A  B  C  D  E  F  G  H");
}

pub fn display_board_string(cb: &Chessboard) -> String {
    let mut board_representation = String::new();
    board_representation.push_str("\n    A  B  C  D  E  F  G  H\n");
    board_representation.push_str("  +------------------------+\n");

    for rank in (0..8).rev() {
        board_representation.push_str(&format!("{} |", rank + 1));
        for file in 0..8 {
            let square = rank * 8 + file;
            let piece_char = get_piece_char(cb, square); // Get the piece character at that square
            board_representation.push_str(&format!(" {} ", piece_char));
        }
        board_representation.push_str(&format!("| {}\n", rank + 1));
    }

    board_representation.push_str("  +------------------------+\n");
    board_representation.push_str("    A  B  C  D  E  F  G  H\n");
    board_representation
}

// pub fn display_bitboard(bitboard: u64) {
//     println!("");
    
//     for rank in (0..8).rev() {
//         for file in 0..8 {
//             let square = rank * 8 + file;
//             let bit = (bitboard >> square) & 1;
//             print!("{} ", bit);
//         }
//         println!();
//     }
// }

pub fn display_bitboard(bitboard: u64) {
    println!("");
    
    for rank in (0..8).rev() {
        for file in 0..8 {
            let square = rank * 8 + file;
            let bit = (bitboard >> square) & 1;
            if bit == 1 {
                print!("{} ", "1".red());
            } else {
                print!("{} ", "0".blue());
            }
        }
        println!();
    }
}
