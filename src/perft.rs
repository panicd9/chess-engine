use crate::chessboard::{Chessboard, Color};
use crate::notation::describe_move;
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
        nodes += perft(&pos.chessboard, depth - 1);
        // display_board(&pos);
    }

    nodes
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
        let nodes = perft(&pos.chessboard, depth - 1);
        // println!("\n Table:");
        // display_board(&pos);
        let played = describe_move(cb, &pos.chessboard).unwrap_or_else(|| "????".to_string());
        println!("{played}: {nodes}");
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

#[cfg(test)]
mod perft_tests {
    use super::*;

    /// Reference counts from the chessprogramming wiki. These are the standard
    /// positions: each one exercises a different corner of move generation
    /// (castling, en passant, promotions, capture-promotions, pins).
    const POSITIONS: &[(&str, &str, &[u64])] = &[
        (
            "initial",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            &[20, 400, 8902, 197281, 4865609],
        ),
        (
            "kiwipete",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            &[48, 2039, 97862, 4085603],
        ),
        (
            "endgame",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            &[14, 191, 2812, 43238, 674624],
        ),
        (
            "promotions",
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            &[6, 264, 9467, 422333],
        ),
        (
            "position 5",
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            &[44, 1486, 62379, 2103487],
        ),
        (
            "position 6",
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            &[46, 2079, 89890, 3894594],
        ),
    ];

    #[test]
    fn perft_matches_reference_counts() {
        for (name, fen, expected) in POSITIONS {
            let cb = Chessboard::from_fen(fen).unwrap();
            for (index, &want) in expected.iter().enumerate() {
                let depth = index as u32 + 1;
                assert_eq!(perft(&cb, depth), want, "{name} at depth {depth} ({fen})");
            }
        }
    }

    /// Slow. Run with `cargo test --release -- --ignored`.
    #[test]
    #[ignore = "takes minutes"]
    fn perft_deep() {
        let cb = Chessboard::new_initial_board();
        assert_eq!(perft(&cb, 6), 119_060_324);
    }
}
