//! Entry point.
//!
//! With no arguments the engine speaks UCI on stdin/stdout, which is what a GUI
//! or a match runner expects. `play` starts the interactive terminal board
//! instead.

use std::env;
use std::process;

use chess_engine::{chessboard::Chessboard, playing_ui::play, uci};

const USAGE: &str = "\
chess-engine - a bitboard chess engine

Usage:
  chess-engine              Speak UCI on stdin/stdout (for GUIs and match runners)
  chess-engine play [FEN]   Play in the terminal, from FEN or the initial position
  chess-engine --help       Show this message

Enter moves as `e2e4`, or `e7e8q` to promote. `quit` to stop.";

fn main() {
    match env::args().nth(1).as_deref() {
        None => {
            if let Err(err) = uci::run() {
                eprintln!("uci: {err}");
                process::exit(1);
            }
        }
        Some("play") => {
            // Any remaining arguments are a FEN, which the shell will have split
            // on its spaces.
            let fen = env::args().skip(2).collect::<Vec<_>>().join(" ");
            let board = if fen.trim().is_empty() {
                Chessboard::new_initial_board()
            } else {
                match Chessboard::from_fen(fen.trim()) {
                    Ok(board) => board,
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(2);
                    }
                }
            };
            play(board);
        }
        Some("-h") | Some("--help") => println!("{USAGE}"),
        Some(other) => {
            eprintln!("unknown argument `{other}`\n\n{USAGE}");
            process::exit(2);
        }
    }
}
