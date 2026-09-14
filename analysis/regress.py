#!/usr/bin/env python3
"""Which missing feature explains the most of our disagreement with Stockfish?

Fits, over quiet positions,

    ours = a*sf + b + sum(w_i * f_i)

`a` absorbs the centipawn-scale difference between the two evaluations; a feature
we do not represent shows up as a non-zero `w_i`, and the term worth adding is
`-w_i` in our own centipawns. Sort by **cp explained** (|w_i| x sd of the
feature), not by the coefficient: a large weight on a feature that barely varies
explains nothing.

**Test the instrument before believing the ranking.** `--knockout` re-scores our
column with a weight zeroed and refits; the regression must then find the term it
just lost. If it cannot recover a term we *do* have, it cannot be trusted about
terms we do not.

This fits a *static* evaluation, so it needs resolved positions. Game positions
are not resolved, so anything in check, already decided, or whose best move is a
capture is dropped -- a cheap stand-in for the quiescence that the public corpora
have already had applied.

Run:  ~/projects/lichess-bot/.venv/bin/python analysis/regress.py
"""
import argparse
import json
import math
import os
import subprocess
import sys

import chess

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from features import NAMES, features  # noqa: E402

DEFAULT_DIR = os.path.expanduser("~/projects/chess-engine-games/analysis")
EVALBATCH = os.path.expanduser(
    "~/projects/chess-engine-pawnhash/target/release/examples/evalbatch")


def solve(a, b):
    """Gauss-Jordan: returns (solution, inverse). Small and well-scaled here."""
    n = len(b)
    m = [row[:] + [0.0] * n + [b[i]] for i, row in enumerate(a)]
    for i in range(n):
        m[i][n + i] = 1.0
    for col in range(n):
        pivot = max(range(col, n), key=lambda r: abs(m[r][col]))
        if abs(m[pivot][col]) < 1e-12:
            raise SystemExit(f"singular design matrix at column {col}")
        m[col], m[pivot] = m[pivot], m[col]
        d = m[col][col]
        m[col] = [x / d for x in m[col]]
        for r in range(n):
            if r == col:
                continue
            factor = m[r][col]
            if factor:
                m[r] = [x - factor * y for x, y in zip(m[r], m[col])]
    solution = [m[i][2 * n] for i in range(n)]
    inverse = [[m[i][n + j] for j in range(n)] for i in range(n)]
    return solution, inverse


def load_quiet(directory, sf_file, decided, only=None, assume_resolved=False):
    """`only` pins the position set, so two oracle depths can be compared on
    exactly the same positions rather than on whatever each one's filter keeps."""
    rows = []
    for line in open(os.path.join(directory, sf_file)):
        r = json.loads(line)
        if "error" in r or r.get("sf_cp") is None:
            continue
        if only is not None:
            if r["fen"] in only:
                rows.append((r["fen"], float(r["sf_cp"])))
            continue
        if abs(r["sf_cp"]) > decided:
            continue
        if not assume_resolved:
            board = chess.Board(r["fen"])
            if board.is_check():
                continue
            best = r.get("sf_best")
            if best and board.is_capture(chess.Move.from_uci(best)):
                continue                  # not resolved: quiescence would go on
        rows.append((r["fen"], float(r["sf_cp"])))
    return rows


def our_evals(fens, weights):
    args = [EVALBATCH] + list(weights)
    out = subprocess.run(args, input="\n".join(fens) + "\n", text=True,
                         capture_output=True, check=True).stdout.split("\n")
    return [None if v.strip() in ("", "ERR") else float(v) for v in out]


def fit(rows, weights=()):
    fens = [f for f, _ in rows]
    ours = our_evals(fens, weights)
    X, y = [], []
    for (fen, sf), mine in zip(rows, ours):
        if mine is None:
            continue
        f = features(chess.Board(fen))
        X.append([1.0, sf] + [float(f[n]) for n in NAMES])
        y.append(mine)
    p, n = len(X[0]), len(X)
    xtx = [[sum(X[r][i] * X[r][j] for r in range(n)) for j in range(p)] for i in range(p)]
    xty = [sum(X[r][i] * y[r] for r in range(n)) for i in range(p)]
    beta, inv = solve(xtx, xty)
    resid = [y[r] - sum(beta[i] * X[r][i] for i in range(p)) for r in range(n)]
    rss = sum(e * e for e in resid)
    mean_y = sum(y) / n
    tss = sum((v - mean_y) ** 2 for v in y)
    sigma2 = rss / (n - p)
    se = [math.sqrt(max(sigma2 * inv[i][i], 0.0)) for i in range(p)]
    means = [sum(X[r][i] for r in range(n)) / n for i in range(p)]
    sd = [math.sqrt(sum((X[r][i] - means[i]) ** 2 for r in range(n)) / n)
          for i in range(p)]
    return {"n": n, "beta": beta, "se": se, "sd": sd, "r2": 1 - rss / tss}


def report(res, title):
    print(f"\n{title}  (n = {res['n']}, R2 = {res['r2']:.4f}, "
          f"scale a = {res['beta'][1]:.3f}, intercept = {res['beta'][0]:+.1f})")
    print(f"  {'feature':<22}{'term to add':>12}{'+/-SE':>8}{'|t|':>6}{'cp explained':>14}")
    rows = []
    for i, name in enumerate(NAMES, start=2):
        coef, se, sd = res["beta"][i], res["se"][i], res["sd"][i]
        rows.append((name, -coef, se, abs(coef / se) if se else 0.0, abs(coef) * sd))
    for name, term, se, t, cp in sorted(rows, key=lambda r: -r[4]):
        print(f"  {name:<22}{term:>+12.1f}{se:>8.1f}{t:>6.0f}{cp:>14.1f}")
    return {r[0]: r for r in rows}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=DEFAULT_DIR)
    ap.add_argument("--sf", default="sf22.jsonl")
    ap.add_argument("--decided", type=int, default=600)
    ap.add_argument("--assume-resolved", action="store_true",
                    help="corpus is already quiescence-resolved (the public sets "
                         "are): keep the |sf| filter, skip the check/capture one")
    ap.add_argument("--fens", default=None,
                    help="file of FENs to pin the position set (skips the quiet filter)")
    ap.add_argument("--knockout", default="PassedPawnScale=0",
                    help="instrument test: zero a term we have and refit")
    args = ap.parse_args()

    only = None
    if args.fens:
        only = {l.strip() for l in open(args.fens) if l.strip()}
    rows = load_quiet(args.dir, args.sf, args.decided, only, args.assume_resolved)
    print(f"{len(rows)} quiet positions from {args.sf}")
    base = report(fit(rows), f"BASELINE ({args.sf})")

    if args.knockout:
        knocked = report(fit(rows, [args.knockout]), f"KNOCKOUT {args.knockout}")
        name = "passed_pawn"
        moved = knocked[name][1] - base[name][1]
        print(f"\ninstrument test: zeroing {args.knockout} moved `{name}` by {moved:+.1f} cp")
        print("  " + ("PASSED -- the regression recovers a term it was denied."
                      if abs(moved) > 3 else
                      "FAILED -- it cannot find a term we removed; do not trust the ranking."))


if __name__ == "__main__":
    main()
