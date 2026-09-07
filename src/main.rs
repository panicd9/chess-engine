//! Entry point.
//!
//! With no arguments the engine speaks UCI on stdin/stdout, which is what a GUI
//! or a match runner expects. `play` starts the interactive terminal board
//! instead.

use std::env;
use std::process;

use chess_engine::{playing_ui::play, uci};

const USAGE: &str = "\
chess-engine - a bitboard chess engine

Usage:
  chess-engine          Speak UCI on stdin/stdout (for GUIs and match runners)
  chess-engine play     Play in the terminal
  chess-engine --help   Show this message";

fn main() {
    match env::args().nth(1).as_deref() {
        None => {
            if let Err(err) = uci::run() {
                eprintln!("uci: {err}");
                process::exit(1);
            }
        }
        Some("play") => play(),
        Some("-h") | Some("--help") => println!("{USAGE}"),
        Some(other) => {
            eprintln!("unknown argument `{other}`\n\n{USAGE}");
            process::exit(2);
        }
    }
}
