//! Static evaluation of every FEN on stdin, one per line, one score per line.
//!
//!     evalbatch [Name=value ...] < fens.txt
//!
//! The per-position twin of `evalfen`, for the case where there are hundreds of
//! thousands of them and a command line will not hold them. Any argument
//! containing `=` sets an evaluation weight first, by the same names the UCI
//! options use, so a term can be isolated by zeroing it.
//!
//! Prints `ERR` for a FEN the board rejects -- a missing king or the waiting
//! side in check -- so the output stays line-for-line with the input.
use chess_engine::chessboard::Chessboard;
use chess_engine::eval::{evaluate, weights};
use std::io::{self, BufWriter, Read, Write};

fn main() {
    for arg in std::env::args().skip(1).filter(|a| a.contains('=')) {
        let (name, value) = arg.split_once('=').unwrap();
        assert!(
            weights::set(name, value.parse().expect("weight value")),
            "unknown weight {name}"
        );
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).expect("read stdin");
    let out = io::stdout();
    let mut out = BufWriter::new(out.lock());
    for line in input.lines() {
        let fen = line.trim();
        if fen.is_empty() {
            continue;
        }
        match Chessboard::from_fen(fen) {
            Ok(board) => writeln!(out, "{}", evaluate(&board)).unwrap(),
            Err(_) => writeln!(out, "ERR").unwrap(),
        }
    }
}
