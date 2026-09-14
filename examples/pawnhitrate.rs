//! Hit rate of the pawn hash over a real search. Counts, not times, so this is
//! unaffected by whatever else the machine is doing.
use chess_engine::chessboard::{Chessboard, Color};
use chess_engine::pawn_hash::{reset_stats, stats};
use chess_engine::search::{self, History};

fn main() {
    let positions = [
        ("startpos", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 10),
        ("middlegame", "r1bq1rk1/pp2bppp/2n1pn2/2pp4/3P1B2/2PBPN2/PP1N1PPP/R2Q1RK1 w - - 0 9", 10),
        ("endgame", "8/1p3pk1/6pp/2p5/2P4P/6P1/1P3PK1/8 w - - 0 40", 14),
    ];
    for (name, fen, depth) in positions {
        let cb = Chessboard::from_fen(fen).unwrap();
        let is_white = cb.side_to_move == Color::White;
        reset_stats();
        search::reset_nodes();
        let mut h = History::new();
        h.ensure_table(64);
        for d in 1..=depth {
            search::nega_max_alpha_beta_best_move(&cb, d, is_white, i32::MIN + 1, i32::MAX - 1, &mut h);
        }
        let (hits, misses) = stats();
        let total = hits + misses;
        println!("{name:11} d={depth:<3} nodes={:<9} probes={total:<9} hit rate {:.2}%",
                 search::nodes_searched(), 100.0 * hits as f64 / total as f64);
    }
}
