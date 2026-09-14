//! How much does the pawn term actually cost, and what would a key cost?
//! The pawn hash only pays if the term is expensive relative to hashing it.
use std::hint::black_box;
use std::time::Instant;
use chess_engine::chessboard::Chessboard;
use chess_engine::eval::evaluate;
use chess_engine::pawn_hash::{passed_pawns, uncached};

fn bench<F: FnMut() -> u64>(name: &str, iters: u64, mut f: F) -> f64 {
    for _ in 0..iters / 8 { black_box(f()); }
    let t = Instant::now();
    let mut acc = 0u64;
    for _ in 0..iters { acc = acc.wrapping_add(f()); }
    black_box(acc);
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("  {:38} {:7.2} ns", name, ns);
    ns
}

/// Candidate pawn key: mix the two pawn bitboards. No incremental maintenance,
/// no growth of Chessboard -- which matters, since it is Copy and cloned per node.
#[inline]
fn pawn_key(w: u64, b: u64, scale: i32) -> u64 {
    let mut x = w.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.rotate_left(32).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30; x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27; x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31) ^ (scale as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93)
}

fn main() {
    let fens = [
        ("middlegame", "r1bq1rk1/pp2bppp/2n1pn2/2pp4/3P1B2/2PBPN2/PP1N1PPP/R2Q1RK1 w - - 0 9"),
        ("endgame",    "8/1p3pk1/6pp/2p5/2P4P/6P1/1P3PK1/8 w - - 0 40"),
        ("startpos",   "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
    ];
    for (name, fen) in fens {
        let cb = Chessboard::from_fen(fen).unwrap();
        println!("{name}:");
        let ev = bench("evaluate()", 5_000_000, || evaluate(black_box(&cb)) as u64);
        let pp = bench("passed_pawns() uncached", 20_000_000, || uncached(black_box(&cb)) as u64);
        let hit = bench("passed_pawns() cached (hit)", 20_000_000, || passed_pawns(black_box(&cb)) as u64);
        let key = bench("pawn_key()", 50_000_000, || {
            pawn_key(black_box(cb.white_pawns), black_box(cb.black_pawns), black_box(84))
        });
        println!("  -> term was {:.1}% of evaluate(); cached costs {:.1} ns, saving {:.1} ns ({:.1}% of eval)\n",
                 100.0 * pp / ev, hit, pp - hit, 100.0 * (pp - hit) / ev);
        let _ = key;
    }
}
