//! The raw counts the two structural weights multiply, for FENs on stdin.
//! Exists to prove the Rust agrees with `analysis/features.py`, which is what
//! the weights were fitted against -- if the two disagree, the fitted value is
//! being applied to a different quantity than it was fitted for.
use chess_engine::chessboard::Chessboard;
use chess_engine::pawn_hash::structure_counts;
use std::io::{self, BufRead};

fn main() {
    for line in io::stdin().lock().lines() {
        let fen = line.unwrap();
        if fen.trim().is_empty() { continue; }
        match Chessboard::from_fen(fen.trim()) {
            Ok(cb) => { let (d, i) = structure_counts(&cb); println!("{d} {i}"); }
            Err(_) => println!("ERR"),
        }
    }
}
