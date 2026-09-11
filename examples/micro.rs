use std::hint::black_box;
use std::time::Instant;
use chess_engine::chessboard::Chessboard;
use chess_engine::eval::evaluate;
use chess_engine::zobrist;
use chess_engine::move_gen::move_gen_rook::single_rook_attacks;

fn bench<F: FnMut() -> u64>(name: &str, iters: u64, mut f: F) {
    for _ in 0..iters / 8 { black_box(f()); }
    let t = Instant::now();
    let mut acc = 0u64;
    for _ in 0..iters { acc = acc.wrapping_add(f()); }
    black_box(acc);
    println!("{:42} {:7.1} ns", name, t.elapsed().as_nanos() as f64 / iters as f64);
}

fn main() {
    let cb = Chessboard::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
    bench("all_black_attacks()  [castling test]", 2_000_000, || black_box(&cb).all_black_attacks());
    bench("is_white_king_under_attack()", 10_000_000, || black_box(&cb).is_white_king_under_attack() as u64);
    bench("evaluate()", 5_000_000, || evaluate(black_box(&cb)) as u64);
    bench("zobrist::hash()", 5_000_000, || zobrist::hash(black_box(&cb)));
    bench("single_rook_attacks()", 50_000_000, || single_rook_attacks(black_box(0x1000_0800u64), black_box(1u64 << 28)));
    bench("get_occupancy()", 50_000_000, || black_box(&cb).get_occupancy());
    bench("white_legal_moves() [48 moves]", 500_000, || chess_engine::move_gen::white_legal_moves(black_box(&cb)).len() as u64);
    bench("white_captures_into() [prototype]", 500_000, || {
        let mut v = Vec::with_capacity(16);
        chess_engine::move_gen::white_captures_into(black_box(&cb), &mut v);
        v.len() as u64
    });
}
