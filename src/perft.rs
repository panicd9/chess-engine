use crate::chessboard::{Chessboard, Color};
use crate::display::{self, display_bitboard, display_board};
use crate::move_gen::{black_legal_moves, white_legal_moves};

pub fn perft(cb: &Chessboard, depth: u32) -> u64 {
    // println!("Depth: {}", depth);
    if depth == 0 {
        return 1;
    }

    let mut nodes = 0;
    let legal_positions = if cb.side_to_move == Color::White {
        white_legal_moves(cb)
    } else {
        black_legal_moves(cb)
    };

    for pos in legal_positions {
        nodes += perft(&pos, depth - 1);
        // display_board(&pos);
    }

    nodes
}

/// Compares two Chessboards and returns a list of moves as a tuple (from_square, to_square).
pub fn compare_boards(before: &Chessboard, after: &Chessboard) -> Option<(usize, usize)> {
    // Compare bitboards for each piece type to identify the difference
    let piece_bitboards = [
        ("P", before.white_pawns, after.white_pawns),
        ("p", before.black_pawns, after.black_pawns),
        ("N", before.white_knights, after.white_knights),
        ("n", before.black_knights, after.black_knights),
        ("B", before.white_bishops, after.white_bishops),
        ("b", before.black_bishops, after.black_bishops),
        ("R", before.white_rooks, after.white_rooks),
        ("r", before.black_rooks, after.black_rooks),
        ("Q", before.white_queens, after.white_queens),
        ("q", before.black_queens, after.black_queens),
        ("K", before.white_king, after.white_king),
        ("k", before.black_king, after.black_king),
    ];

    // Iterate over all piece types
    for (_, before_bitboard, after_bitboard) in piece_bitboards.iter() {
        // Find pieces that have moved using XOR
        let moved = before_bitboard ^ after_bitboard;

        // Iterate over all squares (0 to 63) to find moved pieces
        for i in 0..64 {
            if (moved >> i) & 1 == 1 {
                // Check if there was a piece on square i before
                if (before_bitboard >> i) & 1 == 1 {
                    // A piece was on square i before, now we need to find the destination square
                    for j in 0..64 {
                        if (after_bitboard >> j) & 1 == 1
                            && i != j
                            && (before_bitboard >> j) & 1 == 0
                        {
                            // A piece moved from square i to square j

                            // display_board(before);
                            // display_board(after);
                            // println!("i: {}, j: {}", i, j);
                            return Some((i, j)); // Return the move
                        }
                    }
                }
            }
        }
    }

    // If no move is found (shouldn't happen if only one move is played)
    None
}

/// Perft divide function that prints the move and the corresponding perft count for each move.
pub fn perft_divide(cb: &Chessboard, depth: u32) {
    if depth == 0 {
        return;
    }

    // Generate legal moves for the current turn (white or black)
    let legal_positions = if cb.side_to_move == Color::White {
        white_legal_moves(cb)
    } else {
        black_legal_moves(cb)
    };

    let mut move_counts = vec![];
    let mut total_nodes = 0;

    // For each legal move, calculate the perft of the next depth
    for pos in legal_positions {
        let nodes = perft(&pos, depth - 1);
        // println!("\n Table:");
        // display_board(&pos);
        let compare = compare_boards(cb, &pos).unwrap();
        // println!("COMPARE: {:?}", compare);
        // display_board(&pos);
        let formatted_move = format_move(compare.0, compare.1);
        println!("{}: {}", formatted_move, nodes);
        total_nodes += nodes;
        move_counts.push((pos, nodes));
    }

    println!("Total nodes: {}", total_nodes);
    // // Print the results in the divided format
    // let mut total_nodes = 0;
    // for (pos, nodes) in move_counts {
    //     let moves = compare_boards(cb, &pos); // Get the moves between boards
    //     for (from, to) in moves {
    //         let move_notation = format_move(from, to); // Format move into notation (e.g., "e2e4")
    //         // println!("{}: {}", move_notation, nodes);
    //     }
    //     total_nodes += nodes;
    // }

    // println!("Nodes searched: {}", total_nodes);
}

pub fn format_move(from: usize, to: usize) -> String {
    // Convert square index to chess notation (e.g., "e2e4")
    let start_square = square_to_notation(from);
    let end_square = square_to_notation(to);
    format!("{}{}", start_square, end_square)
}

fn square_to_notation(square: usize) -> String {
    let rank = square / 8 + 1;
    let file = (square % 8) as u8;
    let file_char = (b'a' + file) as char;
    format!("{}{}", file_char, rank)
}

#[cfg(test)]
mod perft_tests {
    use std::time::Instant;

    use super::*;
    use crate::{
        chessboard::{Chessboard, Color},
        utils::{ANTI_DIAGONAL_MASKS, DIAGONAL_MASKS},
    };

    #[test]
    fn test_perft_initial_position() {
        let cb = Chessboard::new_initial_board();
        let nodes = perft(&cb, 1);
        assert_eq!(nodes, 20); // Adjust the expected value based on your move generation logic
    }

    #[test]
    fn test_perft_depth_2() {
        let cb = Chessboard::new_initial_board();
        let nodes = perft(&cb, 2);
        assert_eq!(nodes, 400); // Adjust the expected value based on your move generation logic
    }

    #[test]
    fn test_perft_depth_3() {
        println!("TESTIRANJE::");
        let cb = Chessboard::new_initial_board();
        let nodes = perft(&cb, 3);
        assert_eq!(nodes, 8902); // Adjust the expected value based on your move generation logic
    }

    #[test]
    fn test_perft_depth_4() {
        let cb = Chessboard::new_initial_board();
        let nodes = perft(&cb, 4);
        assert_eq!(nodes, 197281); // Adjust the expected value based on your move generation logic
    }

    #[test]
    fn test_perft_depth_5() {
        let cb = Chessboard::new_initial_board();
        let start = Instant::now();
        let nodes = perft(&cb, 5);
        let duration = start.elapsed();
        println!("Result: {}", nodes);
        println!("Time taken: {:?}", duration);
        assert_eq!(nodes, 4865609); // Adjust the expected value based on your move generation logic
    }

    #[test]
    pub fn test_perft_depth_6() {
        let cb = Chessboard::new_initial_board();
        let nodes = perft(&cb, 6);
        assert_eq!(nodes, 119060324); // Adjust the expected value based on your move generation logic
    }

    #[test]
    fn test_perft_divide() {
        let cb = Chessboard::new_initial_board();
        perft_divide(&cb, 1); // Display moves at depth 1 for white's turn
    }

    #[test]
    fn test_perft_depth_2_divide() {
        let cb = Chessboard::new_initial_board();

        for (i, d) in DIAGONAL_MASKS.iter().enumerate() {
            println!("dia {}: ", i);
            display_bitboard(*d);
        }

        for (i, d) in ANTI_DIAGONAL_MASKS.iter().enumerate() {
            println!("anti {}: ", i);
            display_bitboard(*d);
        }
        perft_divide(&cb, 2); // Display moves at depth 2 for white's turn
    }

    // #[test]
    // fn test_perft_depth_3_divide() {
    //     let cb = Chessboard::new_initial_board();
    //     perft_divide(&cb, 3, true); // Display moves at depth 3 for white's turn
    // }

    #[test]
    fn test_perft_depth_4_divide() {
        let mut cb = Chessboard::new_initial_board();

        
        cb.white_pawns = 0x1fe00;
        cb.black_pawns = 0x7f008000000000;
        cb.white_pawns = 0x80017e00;
        cb.black_pawns = 0x3f00c000000000;
        display_board(&cb);
        perft_divide(&cb, 1); // Display moves at depth 4 for white's turn
    }
}
