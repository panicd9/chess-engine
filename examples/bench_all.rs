use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering as AO};
use std::hint::black_box;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, AO::Relaxed);
        BYTES.fetch_add(l.size() as u64, AO::Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { System.dealloc(p, l) }
}
#[global_allocator]
static A: Counting = Counting;
use std::time::Instant;
use chess_engine::chessboard::Chessboard;
use chess_engine::perft::perft;
use chess_engine::search::{self, History};

fn main() {
    let start = Chessboard::new_initial_board();
    let kiwi = Chessboard::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();

    // ---- perft: pure move generation ----
    for (name, cb, d, expect) in [
        ("startpos perft 5", start, 5u32, 4_865_609u64),
        ("kiwipete perft 4", kiwi, 4, 4_085_603),
    ] {
        let t = Instant::now();
        let n = perft(black_box(&cb), d);
        let e = t.elapsed();
        assert_eq!(n, expect, "{name}");
        println!("{name:20} {n:>10} nodes  {:>7.0} ms  {:>8.2} Mnps",
                 e.as_secs_f64() * 1e3, n as f64 / e.as_secs_f64() / 1e6);
    }

    // ---- fixed-depth search: nodes + time ----
    for (name, cb, d) in [("startpos", start, 10u32), ("kiwipete", kiwi, 8u32)] {
        search::reset_nodes();
        let mut h = History::new();
        h.ensure_table(64);
        ALLOCS.store(0, AO::Relaxed); BYTES.store(0, AO::Relaxed);
        let t = Instant::now();
        // Plain iterative deepening, as the UCI driver does.
        for depth in 1..=d {
            let is_white = cb.side_to_move == chess_engine::chessboard::Color::White;
            black_box(search::nega_max_alpha_beta_best_move(
                &cb, depth, is_white, i32::MIN + 1, i32::MAX - 1, &mut h));
        }
        let e = t.elapsed();
        let n = search::nodes_searched();
        println!("search {name:13} d={d}  {n:>10} nodes  {:>7.0} ms  {:>8.0} knps",
                 e.as_secs_f64() * 1e3, n as f64 / e.as_secs_f64() / 1e3);
        println!("   heap allocations {} ({:.1} per node), {:.1} MB total",
                 ALLOCS.load(AO::Relaxed), ALLOCS.load(AO::Relaxed) as f64 / n as f64,
                 BYTES.load(AO::Relaxed) as f64 / 1e6);
    }
}
