use std::{hint::black_box, time::Duration};
use criterion::{criterion_group, criterion_main, Criterion};
use chess_engine::{chessboard::Chessboard, perft::perft};

fn perft_depth_5(cb: &Chessboard) -> u64 {
    black_box(perft(cb, 5, true))
}

fn perft_depth_6(cb: &Chessboard) -> u64 {
    black_box(perft(&cb, 6, true))
}

fn criterion_benchmark(c: &mut Criterion) {
    let cb = Chessboard::new_initial_board();

    c.bench_function("perft depth 5", |b| b.iter(|| perft_depth_5(&cb)));

    // c.bench_function("perft depth 6", |b| b.iter(|| perft_depth_6(&cb)));
}

criterion_group!{
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(30)).sample_size(10);
    targets = criterion_benchmark
  }
criterion_main!(benches);