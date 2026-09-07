//! Deep perft runs, minutes to hours each. Ignored by default:
//! `cargo test --release --test fen_positions -- --ignored`
//!
//! The suite that guards move generation on every run is
//! `perft_matches_reference_counts` in `src/perft.rs`, which covers the same
//! positions at shallower depths in under a second.

use chess_engine::{chessboard::Chessboard, perft::perft};

fn check(fen: &str, depth: u32, expected: u64) {
    let cb = Chessboard::from_fen(fen).unwrap();
    assert_eq!(perft(&cb, depth), expected, "{fen} at depth {depth}");
}

#[test]
#[ignore = "billions of nodes"]
fn initial_position_depth_7() {
    check("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 7, 3_195_901_860);
}

#[test]
#[ignore = "billions of nodes"]
fn kiwipete_depth_6() {
    check("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 6, 8_031_647_685);
}

#[test]
#[ignore = "billions of nodes"]
fn endgame_depth_8() {
    check("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 8, 3_009_794_393);
}

#[test]
#[ignore = "billions of nodes"]
fn promotions_depth_6() {
    check("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 6, 706_045_033);
}

#[test]
#[ignore = "hundreds of millions of nodes"]
fn position_5_depth_5() {
    check("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 5, 89_941_194);
}
