use std::env;
use std::time::Instant;
use chess_engine::chessboard::{Chessboard, Color};
use chess_engine::search::{self, History};

// A spread of middlegame/endgame positions, so one lucky position cannot carry
// the result.
const SUITE: &[&str] = &[
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "2rr3k/pp3pp1/1nnqbN1p/3pN3/2pP4/2P3Q1/PPB4P/R4RK1 w - - 0 1",
    "r1bqkb1r/pp3ppp/2n1pn2/3p4/3P4/2N1PN2/PP3PPP/R1BQKB1R w KQkq - 0 7",
];

fn main() {
    let mut args = env::args().skip(1);
    let hash: usize = args.next().unwrap().parse().unwrap();
    let depth: u32 = args.next().unwrap().parse().unwrap();

    // "reuse" as a third arg: allocate the table once instead of per position,
    // to separate search cost from allocation cost.
    let reuse = args.next().is_some();
    let mut shared = History::new();
    shared.ensure_table(hash);

    let t = Instant::now();
    search::reset_nodes();
    for fen in SUITE {
        let cb = Chessboard::from_fen(fen).unwrap();
        let is_white = cb.side_to_move == Color::White;
        let mut fresh = History::new();
        if !reuse { fresh.ensure_table(hash); }
        let h = if reuse { &mut shared } else { &mut fresh };
        for d in 1..=depth {
            search::nega_max_alpha_beta_best_move(
                &cb, d, is_white, i32::MIN + 1, i32::MAX - 1, h);
        }
    }
    let e = t.elapsed();
    println!("hash={hash:<5} depth={depth}  {:>10} nodes  {:>7.0} ms  {:>7.0} knps",
             search::nodes_searched(), e.as_secs_f64() * 1e3,
             search::nodes_searched() as f64 / e.as_secs_f64() / 1e3);
}
