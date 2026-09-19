//! Texel tuning of the **piece-square tables** against game results.
//!
//!     texel_pst <train file> [<validation file>] [max positions]
//!
//! `examples/texel` fits the couple of dozen scalar weights. This fits the 768
//! numbers those weights sit on top of -- and which have never been fitted for
//! *this* engine at all. They are PeSTO's, tuned by Ronald Friederich for
//! RofChade as a **complete evaluation standing alone**. Everything since --
//! mobility, pawn structure, passed pawns, and the six terms added in September
//! -- has been bolted on top of tables that were already absorbing those
//! effects, which is double counting. The passed-pawn term is the documented
//! case: it measured "over-generous by ~16cp per pawn, on top of what the
//! piece-square tables already pay an advanced pawn", and the fix was to shrink
//! the *term*, because the table was untouchable. It is touchable here.
//!
//! ## Why this can be fitted properly and the scalars cannot
//!
//! The evaluation is **linear** in the table entries:
//!
//!     pst = (mg_diff * phase + eg_diff * (24 - phase)) / 24
//!
//! so this is a logistic regression, not the coordinate crawl `texel.rs` has to
//! use. Analytic gradients, one pass per epoch, no local minima to speak of.
//! What is fitted is the **full** table entry, material included
//! (`MG_VALUE[p] + MG_PESTO[p][sq]`), because the two are collinear and only
//! their sum is identifiable; the two are separated again on the way out.
//!
//! ## The scale is pinned, on purpose
//!
//! `K` is fitted once against the *current* tables and then frozen. Without
//! that, the weights and `K` trade off freely and the fit is at liberty to
//! rescale the whole evaluation -- which would silently change the meaning of
//! every constant expressed in evaluation units: `FUTILITY_MARGIN_PER_PLY`,
//! `DELTA_MARGIN`, `ASPIRATION_INITIAL`, the drawish material test. Freezing K
//! lets the fit *reshape* the tables without *resizing* them.
//!
//! A ridge pull back towards the starting tables keeps rare squares -- a queen
//! on a1 in the endgame -- from running off on a handful of positions.
use chess_engine::chessboard::{Chessboard, Color};
use chess_engine::eval::evaluate_split;
use chess_engine::move_gen::has_any_legal_move;
use chess_engine::piece_square_tables::{EG_VALUE, MG_VALUE};
use chess_engine::utils::flip;
use std::thread;

const PIECES: usize = 6;
const SQUARES: usize = 64;
const HALF: usize = PIECES * SQUARES; // 384 mg entries, then 384 eg entries
const NPARAM: usize = 2 * HALF;

/// One position, reduced to everything the fit needs and nothing else. The
/// board itself is dropped: at seven million positions it is 1.3 GB that the
/// gradient never looks at.
struct Sample {
    /// Every term that is not the tables, held fixed while they move.
    rest: i32,
    /// Drawish scale in sixty-fourths, applied to (pst + rest).
    scale: i32,
    phase: i32,
    result: f32,
    /// `(table index, +1 for white / -1 for black)`, packed one per piece.
    /// The index is the same for the midgame and endgame halves.
    feats: Vec<(u16, i8)>,
}

fn parse(line: &str) -> Option<(Chessboard, f32)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let (fen, result) = if let Some(i) = line.find("c9") {
        // quiet-labeled.epd:  <4-field fen> c9 "1-0";
        let tag = line[i..].split('"').nth(1)?;
        let r = match tag {
            "1-0" => 1.0,
            "0-1" => 0.0,
            "1/2-1/2" => 0.5,
            _ => return None,
        };
        (format!("{} 0 1", line[..i].trim()), r)
    } else {
        // lichess-big3-resolved.book:  <fen> [1.0]
        let i = line.rfind('[')?;
        let r: f32 = line[i + 1..].trim_end_matches(']').trim().parse().ok()?;
        (line[..i].trim().to_string(), r)
    };
    if !(result == 0.0 || result == 0.5 || result == 1.0) {
        return None;
    }
    let board = Chessboard::from_fen(&fen).ok()?;
    if !has_any_legal_move(&board, board.side_to_move == Color::White) {
        return None;
    }
    Some((board, result))
}

/// The table indices this position touches. White's piece on `sq` reads
/// `MG_PESTO[p][flip(sq)]`, black's reads `MG_PESTO[p][sq]`, and black's
/// contribution is subtracted -- so one signed index per piece covers both
/// halves of the table.
fn features(cb: &Chessboard) -> Vec<(u16, i8)> {
    let mut out = Vec::with_capacity(32);
    for (sq, &cp) in cb.piece_square.iter().enumerate() {
        let code = cp as usize;
        if code >= 12 {
            continue; // Empty.
        }
        let piece = code / 2;
        let white = code % 2 == 0;
        let idx = piece * SQUARES + if white { flip(sq) } else { sq };
        out.push((idx as u16, if white { 1 } else { -1 }));
    }
    out
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
            Some((cb, result)) => {
                let (pst, rest, scale) = evaluate_split(&cb);
                let phase = phase_of(&cb);
                let feats = features(&cb);
                // Self-check: the features and phase must reproduce the
                // engine's own piece-square value exactly, or every gradient
                // below is computed against the wrong function. Checked on the
                // first positions only; it is deterministic.
                if out.len() < 2000 {
                    let p0 = initial();
                    let (mut mg, mut eg) = (0i64, 0i64);
                    for &(idx, sign) in &feats {
                        mg += sign as i64 * p0[idx as usize] as i64;
                        eg += sign as i64 * p0[HALF + idx as usize] as i64;
                    }
                    let mine = ((mg * phase as i64 + eg * (24 - phase) as i64) / 24) as i32;
                    assert_eq!(mine, pst, "feature extraction disagrees with eval()");
                }
                out.push(Sample { rest, scale, phase, result, feats });
            }
            None => skipped += 1,
        }
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    println!("  {name}: {} positions ({skipped} skipped)", out.len());
    out
}

/// The same phase `piece_square_tables::eval` computes, recomputed here because
/// the fit needs it per position and that function does not hand it back.
fn phase_of(cb: &Chessboard) -> i32 {
    const INC: [i32; 6] = [0, 1, 1, 2, 4, 0];
    let mut phase = 0;
    for &cp in cb.piece_square.iter() {
        let code = cp as usize;
        if code < 12 {
            phase += INC[code / 2];
        }
    }
    phase.min(24)
}

/// The starting parameter vector: the tables exactly as the engine has them,
/// material folded in.
fn initial() -> Vec<f64> {
    let mut p = vec![0.0; NPARAM];
    for piece in 0..PIECES {
        for sq in 0..SQUARES {
            let i = piece * SQUARES + sq;
            p[i] = (MG_VALUE[piece] + chess_engine::piece_square_tables::MG_PESTO[piece][sq]) as f64;
            p[HALF + i] =
                (EG_VALUE[piece] + chess_engine::piece_square_tables::EG_PESTO[piece][sq]) as f64;
        }
    }
    p
}

#[inline]
fn eval_of(s: &Sample, p: &[f64]) -> f64 {
    let (mut mg, mut eg) = (0.0, 0.0);
    for &(idx, sign) in &s.feats {
        let f = sign as f64;
        mg += f * p[idx as usize];
        eg += f * p[HALF + idx as usize];
    }
    let pst = (mg * s.phase as f64 + eg * (24 - s.phase) as f64) / 24.0;
    (pst + s.rest as f64) * s.scale as f64 / 64.0
}

fn mean_error(set: &[Sample], p: &[f64], k: f64, threads: usize) -> f64 {
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
                        .map(|smp| {
                            let sig = 1.0 / (1.0 + (-k * eval_of(smp, p)).exp());
                            let d = smp.result as f64 - sig;
                            d * d
                        })
                        .sum::<f64>()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    });
    total / set.len() as f64
}

fn fit_k(set: &[Sample], p: &[f64], threads: usize) -> f64 {
    let (mut lo, mut hi) = (1.0 / 1000.0, 1.0 / 60.0);
    for _ in 0..40 {
        let a = lo + (hi - lo) / 3.0;
        let b = hi - (hi - lo) / 3.0;
        if mean_error(set, p, a, threads) < mean_error(set, p, b, threads) {
            hi = b;
        } else {
            lo = a;
        }
    }
    (lo + hi) / 2.0
}

/// One pass over the training set, accumulating dE/dp for the mean squared
/// error. Each thread keeps its own 768-wide accumulator and they are summed
/// at the end, which is cheaper than any sharing.
fn gradient(set: &[Sample], p: &[f64], k: f64, threads: usize) -> Vec<f64> {
    let chunk = set.len().div_ceil(threads);
    let parts: Vec<Vec<f64>> = thread::scope(|s| {
        let handles: Vec<_> = set
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    let mut g = vec![0.0f64; NPARAM];
                    for smp in part {
                        let e = eval_of(smp, p);
                        let sig = 1.0 / (1.0 + (-k * e).exp());
                        // d/dE of (r - sigma)^2
                        let d = -2.0 * (smp.result as f64 - sig) * k * sig * (1.0 - sig);
                        let s64 = smp.scale as f64 / 64.0;
                        let wmg = d * s64 * smp.phase as f64 / 24.0;
                        let weg = d * s64 * (24 - smp.phase) as f64 / 24.0;
                        for &(idx, sign) in &smp.feats {
                            let f = sign as f64;
                            g[idx as usize] += wmg * f;
                            g[HALF + idx as usize] += weg * f;
                        }
                    }
                    g
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut g = vec![0.0f64; NPARAM];
    for part in parts {
        for i in 0..NPARAM {
            g[i] += part[i];
        }
    }
    let n = set.len() as f64;
    for v in g.iter_mut() {
        *v /= n;
    }
    g
}

/// Split the fitted table back into a material value per piece and a zero-mean
/// square offset, which is the shape `piece_square_tables.rs` stores and which
/// keeps `MG_VALUE` meaning what the rest of the engine thinks it means.
///
/// A pawn can never stand on the first or last rank, so those sixteen entries
/// get no gradient and must be left out of the mean or they drag it towards the
/// starting value.
fn decompose(p: &[f64], half: usize) -> (Vec<i32>, Vec<[i32; 64]>) {
    let mut values = Vec::new();
    let mut tables = Vec::new();
    for piece in 0..PIECES {
        let live: Vec<usize> = (0..SQUARES)
            .filter(|&sq| !(piece == 0 && !(8..56).contains(&sq)))
            .collect();
        let mean: f64 =
            live.iter().map(|&sq| p[half + piece * SQUARES + sq]).sum::<f64>() / live.len() as f64;
        let value = mean.round() as i32;
        let mut t = [0i32; 64];
        for sq in 0..SQUARES {
            t[sq] = if piece == 0 && !(8..56).contains(&sq) {
                0
            } else {
                (p[half + piece * SQUARES + sq] - value as f64).round() as i32
            };
        }
        values.push(value);
        tables.push(t);
    }
    (values, tables)
}

fn emit(name: &str, values: &[i32], tables: &[[i32; 64]], out: &mut String) {
    let names = ["PAWN", "KNIGHT", "BISHOP", "ROOK", "QUEEN", "KING"];
    out.push_str(&format!(
        "const {name}_VALUE: [i32; 6] = [{}];\n",
        values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
    ));
    for (piece, t) in tables.iter().enumerate() {
        out.push_str(&format!("const {name}_{}_TABLE: [i32; 64] = [\n", names[piece]));
        for rank in 0..8 {
            out.push_str("   ");
            for file in 0..8 {
                out.push_str(&format!("{:5},", t[rank * 8 + file]));
            }
            out.push('\n');
        }
        out.push_str("];\n");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let files: Vec<&String> = args.iter().filter(|a| !a.contains('=')).collect();
    let getf = |k: &str, d: f64| -> f64 {
        args.iter()
            .find(|a| a.starts_with(&format!("{k}=")))
            .and_then(|a| a[k.len() + 1..].parse().ok())
            .unwrap_or(d)
    };
    let limit = getf("limit", 2_000_000.0) as usize;
    let epochs = getf("epochs", 300.0) as usize;
    let lr = getf("lr", 2.0);
    // Early stopping on a held-out *corpus* is the main guard; ridge is off
    // by default and available if a fit starts chasing rare squares.
    let ridge = getf("ridge", 0.0);
    let threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(8);

    if files.is_empty() {
        eprintln!("usage: texel_pst <train> [<holdout>] [limit=N epochs=N lr=F ridge=F]");
        std::process::exit(2);
    }
    println!("loading (limit {limit}, {threads} threads)");
    let train = load(files[0], limit);
    let holdout = if files.len() > 1 { load(files[1], limit) } else { Vec::new() };

    let p0 = initial();
    let mut p = p0.clone();

    // Pin the scale: K is fitted once against the tables as they are, then held.
    let k = fit_k(&train, &p, threads);
    let e0_train = mean_error(&train, &p, k, threads);
    let e0_hold = mean_error(&holdout, &p, k, threads);
    println!("K = {:.6} (1/{:.0}), frozen", k, 1.0 / k);
    println!("start   train {:.6}   holdout {:.6}", e0_train, e0_hold);

    // Adam.
    let (b1, b2, eps) = (0.9f64, 0.999f64, 1e-8);
    let mut m = vec![0.0f64; NPARAM];
    let mut v = vec![0.0f64; NPARAM];
    let mut best = (f64::MAX, p.clone(), 0usize);
    for epoch in 1..=epochs {
        let mut g = gradient(&train, &p, k, threads);
        // The data gradient is already a mean, so the ridge is added at its
        // own scale, not divided by the sample count again.
        if ridge != 0.0 {
            for i in 0..NPARAM {
                g[i] += ridge * (p[i] - p0[i]);
            }
        }
        for i in 0..NPARAM {
            m[i] = b1 * m[i] + (1.0 - b1) * g[i];
            v[i] = b2 * v[i] + (1.0 - b2) * g[i] * g[i];
            let mh = m[i] / (1.0 - b1.powi(epoch as i32));
            let vh = v[i] / (1.0 - b2.powi(epoch as i32));
            p[i] -= lr * mh / (vh.sqrt() + eps);
        }
        if epoch % 20 == 0 || epoch == epochs {
            let et = mean_error(&train, &p, k, threads);
            let eh = mean_error(&holdout, &p, k, threads);
            // The holdout picks the stopping point, not the training error.
            if !holdout.is_empty() && eh < best.0 {
                best = (eh, p.clone(), epoch);
            }
            println!(
                "epoch {epoch:>4}   train {et:.6} ({:+.2}%)   holdout {eh:.6} ({:+.2}%)",
                100.0 * (et - e0_train) / e0_train,
                100.0 * (eh - e0_hold) / e0_hold
            );
        }
    }
    if !holdout.is_empty() {
        println!("best holdout at epoch {} ({:.6})", best.2, best.0);
        p = best.1;
    }

    let (mgv, mgt) = decompose(&p, 0);
    let (egv, egt) = decompose(&p, HALF);
    println!("\nmaterial, fitted vs current:");
    for (i, n) in ["pawn", "knight", "bishop", "rook", "queen", "king"].iter().enumerate() {
        println!(
            "  {n:<7} mg {:>5} -> {:<5}   eg {:>5} -> {:<5}",
            MG_VALUE[i], mgv[i], EG_VALUE[i], egv[i]
        );
    }
    let mut out = String::new();
    emit("MG", &mgv, &mgt, &mut out);
    emit("EG", &egv, &egt, &mut out);
    let path = "/tmp/pst_fitted.rs";
    std::fs::write(path, &out).unwrap();
    println!("\nwrote {path}");
}
