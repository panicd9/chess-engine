//! Where a Syzygy probe's time goes: the FEN round trip into shakmaty, or the
//! table lookup itself.
//!
//! `tablebase::probe` hands the position to `shakmaty_syzygy` as a FEN string,
//! so every probing node pays a format, a parse and a position build before
//! any table is touched. This times each step against the whole probe, then
//! runs a real fixed-depth search from the same position so the per-probe cost
//! can be checked against the nps it actually explains.
//!
//! Run it twice -- with SYZYGY_PATH set and unset -- to get the search side of
//! the comparison; without tables `probe` early-outs and the search is the
//! control. Build with `--features tbstats` for the split timed inside the
//! search, which is the one to trust: the tight loop above it understates the
//! lookup about 4x, because there the mapped pages stay resident and the caches
//! hot. Absolute times move with the machine's state -- the tables-off control
//! ran 2004 knps cool and 1298-1546 straight after a two-hour match -- so
//! compare runs by ratio, and interleave on and off.
//!
//!     SYZYGY_PATH=~/syzygy/3-4-5:~/syzygy/6-wdl:~/syzygy/6-dtz DEPTH=14 \
//!       taskset -c 4-11 cargo run --release --features tbstats --example tbprobe [FEN]
use std::collections::HashSet;
use std::hint::black_box;
use std::sync::atomic::AtomicU64;
use std::time::Instant;

static LOOKUP: AtomicU64 = AtomicU64::new(0);

/// (minor faults, major faults) so far. The tables are 148 GB of mmap against
/// 30 GB of RAM, so a probe that misses the page cache is an NVMe read, and
/// that cost lands nowhere a timer around `probe_wdl` can see it.
fn faults() -> (u64, u64) {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    // The command name is parenthesised and may contain spaces, so split after
    // the last ')'. From there state is field 0, minflt 7 and majflt 9.
    let f: Vec<&str> = stat.rsplit_once(')').unwrap().1.split_whitespace().collect();
    (f[7].parse().unwrap(), f[9].parse().unwrap())
}

use chess_engine::chessboard::{Chessboard, Color};
use chess_engine::display::to_fen;
use chess_engine::move_gen::legal_moves;
use chess_engine::search::{self, History};
use chess_engine::tablebase;
use chess_engine::zobrist;
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Position as _};
use shakmaty_syzygy::Tablebase;

/// Positions reachable from `root` that the tables can actually answer, in the
/// order a search meets them: the probing nodes, not a synthetic list.
fn probing_positions(root: &Chessboard, max_pieces: u32, want: usize) -> Vec<Chessboard> {
    let mut seen = HashSet::new();
    let mut frontier = vec![*root];
    let mut out = Vec::new();
    while !frontier.is_empty() && out.len() < want {
        let mut next = Vec::new();
        for cb in &frontier {
            for m in legal_moves(cb) {
                if !seen.insert(zobrist::hash(&m.chessboard)) {
                    continue;
                }
                if m.chessboard.get_occupancy().count_ones() <= max_pieces {
                    out.push(m.chessboard);
                    if out.len() >= want {
                        return out;
                    }
                }
                next.push(m.chessboard);
            }
        }
        frontier = next;
    }
    out
}

/// Per-call nanoseconds, cycling through the whole list so the table access is
/// as scattered as a search's and no single entry stays hot.
fn bench<F: FnMut(&Chessboard) -> u64>(name: &str, pos: &[Chessboard], iters: usize, mut f: F) -> f64 {
    for cb in pos.iter().take(pos.len().min(iters / 8)) {
        black_box(f(cb));
    }
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..iters {
        acc = acc.wrapping_add(f(black_box(&pos[i % pos.len()])));
    }
    black_box(acc);
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("  {name:44} {ns:8.0} ns");
    ns
}

fn main() {
    let fen = std::env::args().nth(1).unwrap_or_else(||
        // QHhQat9o at ply 104: seven pieces, the position the bot searched at
        // 341 knps in the game.
        "8/1N6/8/3r1k2/P4b1R/7K/8/8 w - - 3 53".to_string());
    let depth: u32 = std::env::var("DEPTH").ok().and_then(|d| d.parse().ok()).unwrap_or(14);
    let cb = Chessboard::from_fen(&fen).expect("bad fen");

    let tb_on = match std::env::var("SYZYGY_PATH") {
        Ok(p) if !p.is_empty() => {
            let t = Instant::now();
            let (loaded, bad) = tablebase::load(&p);
            println!("loaded {loaded} tables ({} rejected, max pieces {}) in {:.1}s",
                     bad.len(), tablebase::max_pieces(), t.elapsed().as_secs_f64());
            true
        }
        _ => {
            println!("no SYZYGY_PATH -- tables off (control run)");
            false
        }
    };
    println!("position {fen}  ({} pieces)\n", cb.get_occupancy().count_ones());

    if tb_on {
        let max = tablebase::max_pieces() as u32;
        let pos = probing_positions(&cb, max, 4096);
        let answered = pos.iter().filter(|p| tablebase::probe(p).is_some()).count();
        println!("probe cost over {} reachable positions of <= {max} pieces ({answered} answered):",
                 pos.len());

        let iters = 200_000;
        let fen_only = bench("to_fen()", &pos, iters, |cb| to_fen(cb).len() as u64);
        let parsed = bench("to_fen() + parse::<Fen>()", &pos, iters, |cb| {
            to_fen(cb).parse::<Fen>().unwrap().as_setup().board.occupied().0
        });
        let converted = bench("to_fen() + parse + into_position()  [conversion]", &pos, iters, |cb| {
            let p: Chess = to_fen(cb).parse::<Fen>().unwrap()
                .into_position(CastlingMode::Standard).unwrap();
            p.board().occupied().0
        });
        let whole = bench("tablebase::probe()                  [whole probe]", &pos, iters, |cb| {
            tablebase::probe(black_box(cb)).is_some() as u64
        });

        // The lookup measured directly rather than by subtraction: convert
        // every position up front, then time only what touches the tables.
        let mut own: Tablebase<Chess> = Tablebase::new();
        for dir in std::env::var("SYZYGY_PATH").unwrap().split(':').filter(|d| !d.is_empty()) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let f = e.path();
                match f.extension().and_then(|x| x.to_str()) {
                    Some("rtbw") | Some("rtbz") => { let _ = own.add_file(&f); }
                    _ => {}
                }
            }
        }
        let ready: Vec<Chess> = pos.iter()
            .map(|cb| to_fen(cb).parse::<Fen>().unwrap()
                 .into_position(CastlingMode::Standard).unwrap())
            .collect();
        let warm = ready.len();
        for p in &ready { black_box(own.probe_wdl(p).is_ok()); }
        let t = Instant::now();
        let mut acc = 0u64;
        for i in 0..iters { acc += own.probe_wdl(black_box(&ready[i % warm])).is_ok() as u64; }
        black_box(acc);
        let lookup_direct = t.elapsed().as_nanos() as f64 / iters as f64;
        println!("  {:44} {lookup_direct:8.0} ns", "probe_wdl() alone, pre-converted");

        // One position over and over. If this is much cheaper than cycling
        // 4096, the cost is a block cache being thrashed -- memory or disk. If
        // it is the same, the cost is per-probe compute and no cache would
        // help.
        let one = &ready[0];
        for _ in 0..1000 { black_box(own.probe_wdl(one).is_ok()); }
        let t = Instant::now();
        let mut acc = 0u64;
        for _ in 0..iters { acc += own.probe_wdl(black_box(one)).is_ok() as u64; }
        black_box(acc);
        let hot = t.elapsed().as_nanos() as f64 / iters as f64;
        println!("  {:44} {hot:8.0} ns", "probe_wdl() alone, one hot position");
        LOOKUP.store(lookup_direct as u64, std::sync::atomic::Ordering::Relaxed);

        println!("\n  split: conversion {:.0} ns ({:.0}%) -- of which format {:.0}, parse {:.0}, build {:.0}",
                 converted, 100.0 * converted / whole,
                 fen_only, parsed - fen_only, converted - parsed);
        println!("         table lookup {:.0} ns ({:.0}%) by subtraction, {:.0} ns measured directly\n",
                 whole - converted, 100.0 * (whole - converted) / whole, lookup_direct);
    }

    // ---- the same position, searched for real ----
    tablebase::reset_hits();
    search::reset_nodes();
    let (min0, maj0) = faults();
    let mut h = History::new();
    h.ensure_table(1024);
    let is_white = cb.side_to_move == Color::White;
    let t = Instant::now();
    for d in 1..=depth {
        black_box(search::nega_max_alpha_beta_best_move(
            &cb, d, is_white, i32::MIN + 1, i32::MAX - 1, &mut h));
    }
    let e = t.elapsed().as_secs_f64();
    let (min1, maj1) = faults();
    let n = search::nodes_searched();
    let hits = tablebase::hits();
    println!("search depth {depth}: {n} nodes, {:.0} ms, {:.0} knps", e * 1e3, n as f64 / e / 1e3);
    println!("  tablebase hits {hits} ({:.3}% of nodes)", 100.0 * hits as f64 / n as f64);
    let per = LOOKUP.load(std::sync::atomic::Ordering::Relaxed) as f64;
    if per > 0.0 {
        let probe_s = hits as f64 * per / 1e9;
        println!("  {hits} probes x {per:.0} ns = {probe_s:.2} s of {:.2} s ({:.0}% of the search)",
                 e, 100.0 * probe_s / e);
        println!("  without that time the same tree would run at {:.0} knps",
                 n as f64 / (e - probe_s) / 1e3);
    }
    #[cfg(feature = "tbstats")]
    if hits > 0 {
        let (conv, wdl) = tablebase::probe_nanos();
        println!("  IN SITU, timed inside the search:");
        println!("    conversion   {:>6.0} ns/probe   {:.2} s   {:>4.1}% of the search",
                 conv as f64 / hits as f64, conv as f64 / 1e9, 100.0 * conv as f64 / 1e9 / e);
        println!("    table lookup {:>6.0} ns/probe   {:.2} s   {:>4.1}% of the search",
                 wdl as f64 / hits as f64, wdl as f64 / 1e9, 100.0 * wdl as f64 / 1e9 / e);
        let probed = (conv + wdl) as f64 / 1e9;
        println!("    together     {:>6.0} ns/probe   {probed:.2} s   {:>4.1}% of the search",
                 (conv + wdl) as f64 / hits as f64, 100.0 * probed / e);
        println!("    the rest of the tree then runs at {:.0} knps", n as f64 / (e - probed) / 1e3);
    }
    println!("  page faults during the search: {} minor, {} major",
             min1 - min0, maj1 - maj0);
    if hits > 0 {
        println!("    = {:.2} minor and {:.2} major faults per probe",
                 (min1 - min0) as f64 / hits as f64, (maj1 - maj0) as f64 / hits as f64);
    }
}
