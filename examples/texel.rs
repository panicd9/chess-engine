//! Texel tuning of the six hand-written evaluation weights against a set of
//! positions labelled with the result of the game they came from.
//!
//!     texel <train file> [<validation file>] [max positions]
//!     texel --score 3,5,5,5,2,60 <file> [<file>] [max positions]
//!
//! Two formats are read, which is enough for both public sets:
//!
//!     <FEN> [1.0]             lichess-big3-resolved.book
//!     <FEN> c9 "1-0";         quiet-labeled.epd  (EPD: FEN is only four fields)
//!
//! Labels are from white's point of view and so is `evaluate()`, so they line
//! up with no conversion.
//!
//! This fits the *static* evaluation, not a search. Both public sets are
//! "resolved" -- quiescence has already been applied, and neither contains a
//! position in check -- which is what makes the static evaluation the right
//! thing to fit. Doing it in process rather than over UCI is what makes it
//! finish: `evaluate()` is ~86 ns, so a pass over seven million positions is
//! under a second, against nine minutes through `go depth 1` on twelve engines.
//!
//! Two traps, both of which bit the UCI version first:
//!
//! * **The baseline comes from the engine.** The starting value of each weight
//!   is read out of the live static, never written down here, so it cannot
//!   drift from the binary. The bounds mirror the `uci.rs` option table.
//! * **A stalemate has no meaningful static evaluation** and both sets contain
//!   a few (44 per 100k in big3, 116 per 100k in quiet-labeled). They are
//!   dropped at load.

use chess_engine::chessboard::{Chessboard, Color};
use chess_engine::eval::{evaluate, weights};
use chess_engine::move_gen::has_any_legal_move;
use std::thread;

/// Name, live default, and the bounds from the `uci.rs` option table.
fn spec() -> Vec<(&'static str, i32, i32, i32)> {
    vec![
        ("KnightMobility", weights::get(&weights::KNIGHT_MOBILITY), 0, 30),
        ("BishopMobility", weights::get(&weights::BISHOP_MOBILITY), 0, 30),
        ("RookMobility", weights::get(&weights::ROOK_MOBILITY), 0, 30),
        ("QueenMobility", weights::get(&weights::QUEEN_MOBILITY), 0, 30),
        ("KingShield", weights::get(&weights::MISSING_SHIELD_PAWN), 0, 100),
        ("PassedPawnScale", weights::get(&weights::PASSED_PAWN_SCALE), 0, 400),
    ]
}

fn apply(values: &[i32], names: &[&str]) {
    for (name, value) in names.iter().zip(values) {
        assert!(weights::set(name, *value), "unknown weight {name}");
    }
}

/// One labelled position: the parsed board and the result, 1.0 for a white win.
type Sample = (Chessboard, f64);

fn parse(line: &str) -> Option<Sample> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let (fen, result) = if let Some(rest) = line.strip_suffix(']') {
        let (fen, label) = rest.rsplit_once(" [")?;
        (fen, label.parse::<f64>().ok()?)
    } else {
        let (fen, rest) = line.split_once(" c9 ")?;
        let result = match rest.trim_end_matches(';').trim_matches('"') {
            "1-0" => 1.0,
            "0-1" => 0.0,
            "1/2-1/2" => 0.5,
            _ => return None,
        };
        (fen, result)
    };
    // EPD gives four fields; from_fen insists on six.
    let mut fen = fen.to_string();
    match fen.split_whitespace().count() {
        6 => {}
        5 => fen.push_str(" 1"),
        4 => fen.push_str(" 0 1"),
        _ => return None,
    }
    let board = Chessboard::from_fen(&fen).ok()?;
    // A position with no move has no static evaluation worth fitting.
    if !has_any_legal_move(&board, board.side_to_move == Color::White) {
        return None;
    }
    Some((board, result))
}

fn load(path: &str, limit: usize) -> Vec<Sample> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut out = Vec::new();
    let mut skipped = 0usize;
    for line in text.lines() {
        if out.len() >= limit {
            break;
        }
        match parse(line) {
            Some(s) => out.push(s),
            None => skipped += 1,
        }
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let wins = out.iter().filter(|(_, r)| *r == 1.0).count();
    let draws = out.iter().filter(|(_, r)| *r == 0.5).count();
    println!(
        "  {name}: {} positions ({} skipped), W {:.1}% D {:.1}%",
        out.len(),
        skipped,
        100.0 * wins as f64 / out.len() as f64,
        100.0 * draws as f64 / out.len() as f64
    );
    out
}

/// Mean squared error between the sigmoid of the evaluation and the result.
/// Split over threads: `evaluate()` only reads the weights, which are set
/// before the threads start.
fn mean_error(set: &[Sample], k: f64, threads: usize) -> f64 {
    if set.is_empty() {
        return 0.0;
    }
    let chunk = set.len().div_ceil(threads);
    let total: f64 = thread::scope(|s| {
        let handles: Vec<_> = set
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .map(|(board, result)| {
                            let cp = evaluate(board) as f64;
                            let predicted = 1.0 / (1.0 + (-k * cp).exp());
                            (result - predicted) * (result - predicted)
                        })
                        .sum::<f64>()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    });
    total / set.len() as f64
}

/// The sigmoid scale that best predicts the results we have. Unimodal in K,
/// so a ternary search finds it. Fixing K at a guess flattens the very error
/// differences the descent has to read.
fn fit_k(set: &[Sample], threads: usize) -> f64 {
    let (mut lo, mut hi) = (1.0 / 1000.0, 1.0 / 60.0);
    for _ in 0..40 {
        let a = lo + (hi - lo) / 3.0;
        let b = hi - (hi - lo) / 3.0;
        if mean_error(set, a, threads) < mean_error(set, b, threads) {
            hi = b;
        } else {
            lo = a;
        }
    }
    (lo + hi) / 2.0
}

/// Coarse to fine, sized to each parameter's own range: one schedule for a
/// 0..30 weight and a 0..400 percentage either crawls or overshoots.
fn steps(lo: i32, hi: i32) -> Vec<i32> {
    let mut out = Vec::new();
    for frac in [0.08, 0.03, 0.01] {
        let s = (((hi - lo) as f64 * frac).round() as i32).max(1);
        if !out.contains(&s) {
            out.push(s);
        }
    }
    // A narrow range collapses two fractions onto the same step; the rounds
    // still have to have something to do.
    while out.len() < 3 {
        out.push(1);
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (score_only, args) = match args.split_first() {
        // Several candidate sets in one pass, so a 7M-position corpus is
        // loaded once: --score 3,5,5,5,2,60;3,3,4,3,7,76
        Some((flag, rest)) if flag == "--score" => {
            let sets: Vec<Vec<i32>> = rest[0]
                .split(';')
                .map(|set| set.split(',').map(|v| v.trim().parse().unwrap()).collect())
                .collect();
            (Some(sets), rest[1..].to_vec())
        }
        _ => (None, args),
    };
    if args.is_empty() {
        eprintln!("usage: texel [--score w1,..,w6] <train file> [<validation file>] [max positions]");
        std::process::exit(2);
    }
    let threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(8);
    let limit = args
        .iter()
        .find_map(|a| a.parse::<usize>().ok())
        .unwrap_or(usize::MAX);
    let files: Vec<&String> = args.iter().filter(|a| a.parse::<usize>().is_err()).collect();

    let spec = spec();
    let names: Vec<&str> = spec.iter().map(|(n, ..)| *n).collect();
    let defaults: Vec<i32> = spec.iter().map(|(_, d, ..)| *d).collect();

    let train = load(files[0], limit);
    let holdout = files.get(1).map(|p| load(p, limit));

    let t0 = std::time::Instant::now();
    apply(&defaults, &names);
    let k = fit_k(&train, threads);
    println!(
        "  fitted K = {k:.6} (1/{:.0}) on {threads} threads, {:.1}s\n",
        1.0 / k,
        t0.elapsed().as_secs_f64()
    );

    if let Some(sets) = score_only {
        let mut report = |label: String, v: &[i32]| {
            apply(v, &names);
            let a = mean_error(&train, k, threads);
            match holdout.as_ref().map(|h| mean_error(h, k, threads)) {
                Some(b) => println!("  {label:<26}  train {a:.6}   holdout {b:.6}"),
                None => println!("  {label:<26}  train {a:.6}"),
            }
        };
        let show = |v: &[i32]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        report(format!("defaults {}", show(&defaults)), &defaults);
        for set in &sets {
            assert_eq!(set.len(), names.len(), "a candidate set needs six values");
            report(show(set), set);
        }
        return;
    }

    apply(&defaults, &names);
    let base_train = mean_error(&train, k, threads);
    let base_hold = holdout.as_ref().map(|h| mean_error(h, k, threads));
    println!("  start train error {base_train:.6}");

    let mut current = defaults.clone();
    let mut best = base_train;
    for round in 0..3 {
        let mut improved = true;
        while improved {
            improved = false;
            for (i, (name, _, lo, hi)) in spec.iter().enumerate() {
                let step = steps(*lo, *hi)[round];
                for delta in [step, -step] {
                    let mut trial = current.clone();
                    trial[i] = (trial[i] + delta).clamp(*lo, *hi);
                    if trial[i] == current[i] {
                        continue;
                    }
                    apply(&trial, &names);
                    let e = mean_error(&train, k, threads);
                    if e < best - 1e-12 {
                        best = e;
                        current = trial;
                        improved = true;
                        println!("    {name:<16} {:>4}   train {e:.6}", current[i]);
                        break;
                    }
                }
            }
        }
        println!(
            "  -- round {} settled: {best:.6}  ({:.1}s)",
            round + 1,
            t0.elapsed().as_secs_f64()
        );
    }

    println!("\n{:=<64}", "");
    println!("{:<18}{:>9}{:>9}{:>10}", "weight", "default", "tuned", "change");
    println!("{:-<64}", "");
    for (i, name) in names.iter().enumerate() {
        println!(
            "{:<18}{:>9}{:>9}{:>+10}",
            name,
            defaults[i],
            current[i],
            current[i] - defaults[i]
        );
    }
    println!("{:-<64}", "");
    println!(
        "train error  {base_train:.6} -> {best:.6}  ({:+.2}%)",
        (best - base_train) / base_train * 100.0
    );
    if let (Some(before), Some(set)) = (base_hold, holdout.as_ref()) {
        apply(&current, &names);
        let after = mean_error(set, k, threads);
        println!(
            "holdout      {before:.6} -> {after:.6}  ({:+.2}%)   <- the number worth believing",
            (after - before) / before * 100.0
        );
    }
    println!("{:=<64}", "");
    for (i, name) in names.iter().enumerate() {
        println!("option.{name}={}", current[i]);
    }
}
