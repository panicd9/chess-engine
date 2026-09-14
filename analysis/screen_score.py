#!/usr/bin/env python3
"""Score the futility screen: which cell picks the better move for the same work?

Every cell searched the same positions with the same node budget, so the only
thing that differs is the move chosen. Quality is centipawn loss against
Stockfish 19 at depth 22:

    loss = sf(before) - sf(after the chosen move)    from the mover's point of view

Because every cell sees exactly the same positions, the comparison against the
default is **paired** -- differences are taken position by position and the
interval comes from those differences. That is far more sensitive than comparing
two independent means, and it is the only reason a few hundred positions can say
anything at all.

Two guards, both from this project's scars:

* **Calibration first.** `CAL_margin0_d3` (maximally aggressive) and `CAL_off`
  are known-bad settings. If they do not rank worse than the default, the metric
  is not measuring move quality and the screen must be thrown away rather than
  mined for candidates.
* **This can reject, it cannot accept.** A measured improvement here is not Elo;
  twice in this project a real improvement by one measure converted to nothing
  over the board. The output is at most two candidates for a real match.

Run:  ~/projects/lichess-bot/.venv/bin/python analysis/screen_score.py
"""
import argparse
import collections
import json
import math
import os

import chess

DEFAULT_DIR = os.path.expanduser("~/projects/chess-engine-games/analysis")
MATE_CP = 2000
DEFAULT_CELL = "m120_d3"


def load_sf(path):
    out = {}
    for line in open(path):
        r = json.loads(line)
        if "error" in r:
            continue
        v = r.get("sf_cp")
        if v is None:
            m = r.get("sf_mate")
            v = None if m is None else (MATE_CP if m > 0 else -MATE_CP)
        if v is not None:
            out[r["fen"]] = v
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=DEFAULT_DIR)
    ap.add_argument("--screen", default=None)
    ap.add_argument("--sf", default=None)
    ap.add_argument("--clamp", type=int, default=500,
                    help="cap a single position's loss, so one blunder cannot "
                         "decide the ranking")
    ap.add_argument("--baseline", default=DEFAULT_CELL)
    args = ap.parse_args()

    sf = load_sf(args.sf or os.path.join(args.dir, "sf22.jsonl"))
    screen = args.screen or os.path.join(args.dir, "screen_futility.jsonl")

    # cell -> fen -> loss, plus whether the move matched Stockfish's own choice.
    loss = collections.defaultdict(dict)
    match = collections.defaultdict(list)
    depth = collections.defaultdict(list)
    missing = 0

    sf_best = {}
    for line in open(os.path.join(args.dir, "sf22.jsonl")):
        r = json.loads(line)
        if r.get("sf_best"):
            sf_best[r["fen"]] = r["sf_best"]

    for line in open(screen):
        r = json.loads(line)
        mv = r.get("move")
        if not mv or "error" in r:
            continue
        before = sf.get(r["fen"])
        if before is None:
            missing += 1
            continue
        b = chess.Board(r["fen"])
        mover_is_white = b.turn == chess.WHITE
        try:
            b.push_uci(mv)
        except Exception:
            missing += 1
            continue
        after = sf.get(b.fen())
        if after is None:
            missing += 1
            continue
        sign = 1 if mover_is_white else -1
        l = (before * sign) - (after * sign)
        loss[r["cell"]][r["fen"]] = max(0, min(l, args.clamp))
        if r["fen"] in sf_best:
            match[r["cell"]].append(1 if mv == sf_best[r["fen"]] else 0)
        if r.get("depth"):
            depth[r["cell"]].append(r["depth"])

    base = loss.get(args.baseline)
    if not base:
        raise SystemExit(f"baseline cell {args.baseline} not in the screen")
    print(f"scored {sum(len(v) for v in loss.values())} searches over "
          f"{len(base)} positions; {missing} unusable\n")

    rows = []
    for cell, d in loss.items():
        shared = [f for f in d if f in base]
        diffs = [d[f] - base[f] for f in shared]          # negative = better
        n = len(diffs)
        mean = sum(diffs) / n
        sd = math.sqrt(sum((x - mean) ** 2 for x in diffs) / n) if n > 1 else 0.0
        ci = 1.96 * sd / math.sqrt(n) if n else 0.0
        rows.append({
            "cell": cell,
            "acpl": sum(d.values()) / len(d),
            "vs_base": mean, "ci": ci, "n": n,
            "match": 100 * sum(match[cell]) / len(match[cell]) if match[cell] else float("nan"),
            "depth": sum(depth[cell]) / len(depth[cell]) if depth[cell] else float("nan"),
        })

    rows.sort(key=lambda r: r["acpl"])
    print(f"{'cell':<16}{'acpl':>7}{'vs base':>10}{'95% CI':>9}"
          f"{'sf-move %':>11}{'mean depth':>12}")
    for r in rows:
        star = "  <- default" if r["cell"] == args.baseline else ""
        flag = "  CALIBRATION" if r["cell"].startswith("CAL") else star
        print(f"{r['cell']:<16}{r['acpl']:>7.1f}{r['vs_base']:>+10.1f}"
              f"{r['ci']:>9.1f}{r['match']:>10.1f}%{r['depth']:>12.2f}{flag}")

    # ---- the gate -----------------------------------------------------------
    base_acpl = next(r["acpl"] for r in rows if r["cell"] == args.baseline)
    print()
    ok = True
    for cal in ("CAL_margin0_d3", "CAL_off"):
        r = next((x for x in rows if x["cell"] == cal), None)
        if r is None:
            continue
        worse = r["acpl"] > base_acpl
        print(f"calibration {cal}: acpl {r['acpl']:.1f} vs default {base_acpl:.1f} "
              f"-> {'worse, as expected' if worse else 'NOT WORSE -- metric suspect'}")
        ok &= worse
    print("\nINSTRUMENT " + ("PASSED: the metric ranks known-bad settings below the "
                             "default, so the ranking above is worth reading."
                             if ok else
                             "FAILED: do not pick candidates from this screen."))


if __name__ == "__main__":
    main()
