use colored::Colorize;

use crate::chessboard::{Chessboard, Color};

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
    ' '
}

pub fn display_board(cb: &Chessboard) {
    // Print top file labels
    println!();
    println!(" +---+---+---+---+---+---+---+---+");

    for rank in (0..8).rev() {
        // Print rank number at the start of the row
        print!(" |");
        for file in 0..8 {
            let square = rank * 8 + file; // Calculate the square index
            let piece_char = get_piece_char(cb, square); // Get the piece character at that square
            print!(" {} |", piece_char); // Print the piece or empty space inside the grid
        }
        // Print rank number again at the end of the row
        println!(" {}", rank + 1);
        println!(" +---+---+---+---+---+---+---+---+");
    }

    // Print bottom file labels
    println!("   a   b   c   d   e   f   g   h");
    println!("\nFEN: {}\n", to_fen(cb));
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

pub fn to_fen(cb: &Chessboard) -> String {
    // Piece placement
    let mut board = String::new();
    for rank in (0..8).rev() {
        let mut empty_squares = 0;
        for file in 0..8 {
            let square = rank * 8 + file;
            let piece = get_piece_char(cb, square);
            match piece {
                'P' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('P');
                }
                'p' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('p');
                }
                'N' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('N');
                }
                'n' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('n');
                }
                'B' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('B');
                }
                'b' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('b');
                }
                'R' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('R');
                }
                'r' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('r');
                }
                'Q' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('Q');
                }
                'q' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('q');
                }
                'K' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('K');
                }
                'k' => {
                    if empty_squares > 0 {
                        board.push_str(&empty_squares.to_string());
                        empty_squares = 0;
                    }
                    board.push('k');
                }
                ' ' => empty_squares += 1,
                _ => {
                    panic!("Invalid piece character: {}", piece);
                }
            }
        }
        if empty_squares > 0 {
            board.push_str(&empty_squares.to_string());
        }
        if rank > 0 {
            board.push('/');
        }
    }

    // Side to move
    let side_to_move = match cb.side_to_move {
        Color::White => "w",
        Color::Black => "b",
    };

    // Castling rights
    let mut castling_rights = String::new();
    if cb.white_can_castle_king_side {
        castling_rights.push('K');
    }
    if cb.white_can_castle_queen_side {
        castling_rights.push('Q');
    }
    if cb.black_can_castle_king_side {
        castling_rights.push('k');
    }
    if cb.black_can_castle_queen_side {
        castling_rights.push('q');
    }
    if castling_rights.is_empty() {
        castling_rights.push('-');
    }

    // En passant
    let en_passant = if cb.en_passant != 0 {
        let ep_square = cb.en_passant;
        let file = (ep_square % 8) as u8;
        let rank = (ep_square / 8) as u8;
        format!("{}{}", (b'a' + file) as char, (b'1' + rank) as char)
    } else {
        "-".to_string()
    };

    // Halfmove clock
    let halfmove_clock = cb.halfmove_clock.to_string();

    // Fullmove counter
    let fullmove_counter = cb.fullmove_counter.to_string();

    // Return FEN string
    format!(
        "{} {} {} {} {} {}",
        board, side_to_move, castling_rights, en_passant, halfmove_clock, fullmove_counter
    )
}
