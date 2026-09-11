use chess_engine::chessboard::{Chessboard, Color};
use chess_engine::notation::describe_move;
use chess_engine::search::{self, History};

const POSITIONS: &[(&str, &str, u32)] = &[
    ("startpos",   "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 9),
    ("kiwipete",   "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 8),
    ("pos3",       "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 10),
    ("pos4",       "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 8),
    ("pos5",       "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 8),
    ("pos6",       "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10", 8),
    // Stalemate-prone: few pieces, cramped kings.
    ("stalemate1", "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1", 6),
    ("stalemate2", "8/8/8/8/8/5k2/5p2/5K2 w - - 0 1", 8),
    ("kpk",        "8/8/8/4k3/8/4K3/4P3/8 w - - 0 1", 12),
    ("promo race", "8/P6k/8/8/8/8/6p1/K7 w - - 0 1", 10),
    ("zugzwang",   "8/8/p1p5/1p5p/1P5p/8/PPP2K1p/4R1rk w - - 0 1", 10),
    ("mate in 3",  "r1r5/1P6/8/8/8/8/8/4K2k w - - 0 1", 8),
];

fn main() {
    for (name, fen, depth) in POSITIONS {
        let cb = Chessboard::from_fen(fen).expect(name);
        let is_white = cb.side_to_move == Color::White;
        search::reset_nodes();
        let mut h = History::new();
        h.ensure_table(64);
        let mut last = (0, cb);
        for d in 1..=*depth {
            last = search::nega_max_alpha_beta_best_move(
                &cb, d, is_white, i32::MIN + 1, i32::MAX - 1, &mut h);
        }
        let mv = describe_move(&cb, &last.1).unwrap_or_else(|| "----".into());
        println!("{name:12} d={depth:<3} nodes={:<10} score={:<12} best={mv}",
                 search::nodes_searched(), last.0);
    }
}
