use std::hint::black_box;
use std::time::Instant;
use chess_engine::chessboard::Chessboard;
use chess_engine::eval::evaluate;
use chess_engine::move_gen::move_gen_bishop::all_bishops_attacks;
use chess_engine::move_gen::move_gen_queen::all_queens_attacks;
use chess_engine::move_gen::move_gen_rook::all_rooks_attacks;

fn bench<F: FnMut() -> u64>(name: &str, iters: u64, mut f: F) {
    for _ in 0..iters / 8 { black_box(f()); }
    let t = Instant::now();
    let mut acc = 0u64;
    for _ in 0..iters { acc = acc.wrapping_add(f()); }
    black_box(acc);
    println!("  {:44} {:7.1} ns", name, t.elapsed().as_nanos() as f64 / iters as f64);
}

// What mobility() does today: each *_attacks() helper calls get_occupancy() itself.
fn mobility_now(cb: &Chessboard) -> i32 {
    let w = cb.white_knights_attacks().count_ones() as i32 * 4
        + cb.white_bishops_attacks().count_ones() as i32 * 4
        + cb.white_rooks_attacks().count_ones() as i32 * 2
        + cb.white_queens_attacks().count_ones() as i32;
    let b = cb.black_knights_attacks().count_ones() as i32 * 4
        + cb.black_bishops_attacks().count_ones() as i32 * 4
        + cb.black_rooks_attacks().count_ones() as i32 * 2
        + cb.black_queens_attacks().count_ones() as i32;
    w - b
}

// Occupancy computed once and passed in.
fn mobility_hoisted(cb: &Chessboard) -> i32 {
    let o = cb.get_occupancy();
    let w = cb.white_knights_attacks().count_ones() as i32 * 4
        + all_bishops_attacks(o, cb.white_bishops).count_ones() as i32 * 4
        + all_rooks_attacks(o, cb.white_rooks).count_ones() as i32 * 2
        + all_queens_attacks(o, cb.white_queens).count_ones() as i32;
    let b = cb.black_knights_attacks().count_ones() as i32 * 4
        + all_bishops_attacks(o, cb.black_bishops).count_ones() as i32 * 4
        + all_rooks_attacks(o, cb.black_rooks).count_ones() as i32 * 2
        + all_queens_attacks(o, cb.black_queens).count_ones() as i32;
    w - b
}

fn main() {
    let cb = Chessboard::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
    bench("evaluate() whole", 3_000_000, || evaluate(black_box(&cb)) as u64);
    bench("mobility, as written", 5_000_000, || mobility_now(black_box(&cb)) as u64);
    bench("mobility, occupancy hoisted", 5_000_000, || mobility_hoisted(black_box(&cb)) as u64);
    bench("  knights only (table lookups)", 10_000_000, || {
        (black_box(&cb).white_knights_attacks().count_ones()
            + black_box(&cb).black_knights_attacks().count_ones()) as u64
    });
    bench("  sliders only (hyperbola)", 5_000_000, || {
        let o = black_box(&cb).get_occupancy();
        (all_bishops_attacks(o, cb.white_bishops).count_ones()
            + all_rooks_attacks(o, cb.white_rooks).count_ones()
            + all_queens_attacks(o, cb.white_queens).count_ones()
            + all_bishops_attacks(o, cb.black_bishops).count_ones()
            + all_rooks_attacks(o, cb.black_rooks).count_ones()
            + all_queens_attacks(o, cb.black_queens).count_ones()) as u64
    });
    bench("  get_occupancy x6", 20_000_000, || {
        let c = black_box(&cb);
        c.get_occupancy() ^ c.get_occupancy() ^ c.get_occupancy()
            ^ c.get_occupancy() ^ c.get_occupancy() ^ c.get_occupancy()
    });
}
