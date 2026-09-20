//! Check the tablebase module against our own Chessboard type.
use chess_engine::chessboard::Chessboard;
use chess_engine::tablebase::{self, Verdict};
fn main() {
    let path = std::env::var("SYZYGY_PATH")
        .unwrap_or_else(|_| format!("{}/syzygy/3-4-5", std::env::var("HOME").unwrap()));
    let (loaded, bad) = tablebase::load(&path);
    println!("loaded {loaded} tables, {} rejected, max pieces {}", bad.len(), tablebase::max_pieces());
    for b in bad.iter().take(5) { println!("  rejected {b}"); }
    for fen in std::env::args().skip(1) {
        match Chessboard::from_fen(&fen) {
            Ok(cb) => match tablebase::probe(&cb) {
                Some(v) => println!("  {:?}\t{fen}", v),
                None => println!("  (no answer)\t{fen}"),
            },
            Err(e) => println!("  bad fen: {e}"),
        }
    }
}
