//! Print the static evaluation of each FEN given on the command line.
//!
//!     evalfen [Name=value ...] "<fen>" ["<fen>" ...]
//!
//! Any argument containing `=` sets an evaluation weight first, by the same
//! names the UCI options use, so a term can be isolated by zeroing it.
//!
//! From white's point of view, in centipawns, with whatever the weights are
//! compiled to default to.
use chess_engine::chessboard::Chessboard;
use chess_engine::eval::{evaluate, weights};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    for arg in args.iter().filter(|a| a.contains('=')) {
        let (name, value) = arg.split_once('=').unwrap();
        assert!(
            weights::set(name, value.parse().expect("weight value")),
            "unknown weight {name}"
        );
    }
    for fen in args.iter().filter(|a| !a.contains('=')).cloned() {
        match Chessboard::from_fen(&fen) {
            Ok(board) => println!("{:>6}  {fen}", evaluate(&board)),
            Err(e) => println!("{:>6}  {fen}", format!("ERR {e}")),
        }
    }
}
