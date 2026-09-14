#!/usr/bin/env python3
"""Screen futility's two parameters at fixed nodes, before spending matches.

The fair question for a *pruning* parameter is neither "which reaches depth N
cheapest" (that rewards aggression) nor "which is best at a fixed depth" (that
rewards caution). It is **which plays better for the same work** -- so every cell
gets the same node budget and we compare the moves they choose.

Fixed nodes also makes the screen immune to load: the same position at the same
budget gives the same move and the same node count whether the box is idle or
busy, so this can run beside a match without either disturbing the other.

Stage 1 (this script): run every cell over the sample, record the move chosen.
Stage 2 (`sf_eval.py`): score any *resulting* position we have not seen before --
the games only gave us evaluations for moves actually played, so a cell that
picks something else needs a new label.
Stage 3 (`screen_score.py`): centipawn loss per cell.

Run:  ~/projects/lichess-bot/.venv/bin/python analysis/screen_futility.py
"""
import argparse
import json
import os
import random
import subprocess
import sys
from multiprocessing import Pool

DEFAULT_DIR = os.path.expanduser("~/projects/chess-engine-games/analysis")
ENGINE = os.path.expanduser("~/projects/chess-engine-games/engine-w12")

_eng = None
_budget = None


class Engine:
    """One long-lived engine process, reused across positions."""

    def __init__(self, path):
        self.p = subprocess.Popen([path], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, text=True, bufsize=1)
        self._send("uci")
        self._until("uciok")

    def _send(self, line):
        self.p.stdin.write(line + "\n")
        self.p.stdin.flush()

    def _until(self, prefix):
        out = []
        for line in self.p.stdout:
            line = line.rstrip("\n")
            out.append(line)
            if line.startswith(prefix):
                return out
        raise RuntimeError(f"engine closed waiting for {prefix}")

    def setoption(self, name, value):
        self._send(f"setoption name {name} value {value}")

    def search(self, fen, nodes):
        # A cold table per position: otherwise a cell inherits whatever the
        # previous position taught it, and the order of the sample would matter.
        self._send("ucinewgame")
        self._send("isready")
        self._until("readyok")
        self._send(f"position fen {fen}")
        self._send(f"go nodes {nodes}")
        lines = self._until("bestmove")
        info = next((l for l in reversed(lines)
                     if l.startswith("info ") and " nodes " in l), None)
        best = lines[-1].split()
        depth = int(info.split(" depth ")[1].split()[0]) if info else None
        n = int(info.split(" nodes ")[1].split()[0]) if info else None
        return (best[1] if len(best) > 1 else None), depth, n

    def close(self):
        try:
            self._send("quit")
            self.p.wait(timeout=5)
        except Exception:
            self.p.kill()


def _init(engine_path, budget):
    global _eng, _budget
    _eng = Engine(engine_path)
    _budget = budget


def _run_cell(task):
    """One cell over the whole sample, on one engine process."""
    label, opts, fens = task
    for name, value in opts:
        _eng.setoption(name, value)
    out = []
    for fen in fens:
        try:
            move, depth, nodes = _eng.search(fen, _budget)
        except Exception as exc:
            out.append({"cell": label, "fen": fen, "error": repr(exc)})
            continue
        out.append({"cell": label, "fen": fen, "move": move,
                    "depth": depth, "nodes": nodes})
    return out


def sample_positions(directory, n, seed=20260914):
    """Undecided, post-book positions, stratified by game phase."""
    sf = {}
    for line in open(os.path.join(directory, "sf22.jsonl")):
        r = json.loads(line)
        if "error" in r:
            continue
        v = r.get("sf_cp")
        if v is None:
            m = r.get("sf_mate")
            v = None if m is None else (2000 if m > 0 else -2000)
        sf[r["fen"]] = v

    rows = []
    for line in open(os.path.join(directory, "positions.jsonl")):
        p = json.loads(line)
        v = sf.get(p["fen"])
        if v is None or abs(v) > 600:
            continue          # missing, or already decided: tells us nothing
        if p["ply"] < 16:
            continue          # opening book territory
        rows.append(p)

    # Stratify by piece count so the sample is not all middlegame.
    def bucket(fen):
        board = fen.split()[0]
        pieces = sum(c.isalpha() for c in board)
        return "opening" if pieces >= 26 else ("middlegame" if pieces >= 14 else "endgame")

    by = {}
    for p in rows:
        by.setdefault(bucket(p["fen"]), []).append(p)
    rng = random.Random(seed)
    out, per = [], max(1, n // len(by))
    for k in sorted(by):
        pool = by[k]
        rng.shuffle(pool)
        out += pool[:per]
    rng.shuffle(out)
    seen, uniq = set(), []
    for p in out:
        if p["fen"] not in seen:
            seen.add(p["fen"])
            uniq.append(p["fen"])
    return uniq[:n]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=DEFAULT_DIR)
    ap.add_argument("--engine", default=ENGINE)
    ap.add_argument("--positions", type=int, default=400)
    ap.add_argument("--nodes", type=int, default=2_000_000)
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    fens = sample_positions(args.dir, args.positions)
    print(f"{len(fens)} positions, {args.nodes:,} nodes each")

    cells = []
    for margin in (60, 90, 120, 180, 250, 350):
        for maxdepth in (2, 3, 4):
            cells.append((f"m{margin}_d{maxdepth}",
                          [("FutilityMargin", margin), ("FutilityMaxDepth", maxdepth)]))
    # Calibration: a setting known to be bad, and the off switch. If the metric
    # cannot rank these below the default, it is not measuring quality.
    cells.append(("CAL_margin0_d3", [("FutilityMargin", 0), ("FutilityMaxDepth", 3)]))
    cells.append(("CAL_off", [("Futility", "false")]))
    print(f"{len(cells)} cells = {len(cells) * len(fens):,} searches, "
          f"{len(cells) * len(fens) * args.nodes / 1e9:.1f} billion nodes")

    out_path = args.out or os.path.join(args.dir, "screen_futility.jsonl")
    tasks = [(label, opts, fens) for label, opts in cells]
    done = 0
    with open(out_path, "w", buffering=1) as fh, Pool(
            args.workers, initializer=_init,
            initargs=(args.engine, args.nodes)) as pool:
        for rows in pool.imap_unordered(_run_cell, tasks):
            for r in rows:
                fh.write(json.dumps(r) + "\n")
            done += 1
            print(f"  cell {done}/{len(cells)} done ({rows[0]['cell']})", flush=True)
    print(f"-> {out_path}")


if __name__ == "__main__":
    sys.exit(main())
