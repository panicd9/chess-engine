#!/usr/bin/env python3
"""Score every position in the corpus with Stockfish 19, as the oracle.

Stockfish 16 is the calibrated yardstick and must be used for any strength
anchor; 19 is the strongest analyser and is what to use when Stockfish is the
*judge* rather than the opponent. This is the latter.

Scores are recorded from **White's point of view**, the same convention as our
`evaluate()` and as the two public Texel corpora, so nothing needs a sign flip
later. Mate scores are kept as a signed distance in plies rather than being
flattened into a centipawn number.

Incremental: results are keyed by FEN and appended, so re-running only scores
what is missing. Stockfish 19 terminates the process on a malformed position, so
positions are handed over through python-chess rather than assembled by hand.

Run:  ~/projects/lichess-bot/.venv/bin/python analysis/sf_eval.py --depth 22
"""
import argparse
import json
import os
import sys
import time
from multiprocessing import Pool

import chess
import chess.engine

SF19 = os.path.expanduser("~/.local/bin/stockfish19")
DEFAULT_DIR = os.path.expanduser("~/projects/chess-engine-games/analysis")

_engine = None
_depth = None


def _init(depth, threads, hash_mb):
    global _engine, _depth
    _depth = depth
    _engine = chess.engine.SimpleEngine.popen_uci(SF19)
    _engine.configure({"Threads": threads, "Hash": hash_mb})


def _score(fen):
    """(fen, cp from white, mate distance from white, best move) or an error."""
    try:
        board = chess.Board(fen)
        info = _engine.analyse(board, chess.engine.Limit(depth=_depth))
        pov = info["score"].white()
        pv = info.get("pv") or []
        return {
            "fen": fen,
            "sf_cp": pov.score(),                 # None when it is a mate score
            "sf_mate": pov.mate(),                # signed plies-to-mate, or None
            "sf_best": pv[0].uci() if pv else None,
            "sf_depth": info.get("depth"),
        }
    except Exception as exc:
        return {"fen": fen, "error": repr(exc)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=DEFAULT_DIR)
    ap.add_argument("--depth", type=int, default=22)
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--threads", type=int, default=1, help="Threads per worker")
    ap.add_argument("--hash", type=int, default=256, help="MB per worker")
    ap.add_argument("--limit", type=int, default=0, help="score at most N (a benchmark)")
    ap.add_argument("--src", default=None,
                    help="jsonl with a \"fen\" per line (default: positions.jsonl). "
                         "Used to score positions that did not occur in the games, "
                         "e.g. those a screened parameter setting moves into.")
    args = ap.parse_args()

    src = args.src or os.path.join(args.dir, "positions.jsonl")
    out = os.path.join(args.dir, f"sf{args.depth}.jsonl")

    fens, seen = [], set()
    for line in open(src):
        f = json.loads(line)["fen"]
        if f not in seen:
            seen.add(f)
            fens.append(f)

    done = set()
    if os.path.exists(out):
        for line in open(out):
            try:
                done.add(json.loads(line)["fen"])
            except Exception:
                pass
    todo = [f for f in fens if f not in done]
    if args.limit:
        todo = todo[:args.limit]
    print(f"{len(fens)} unique positions, {len(done)} already scored, {len(todo)} to do")
    if not todo:
        return

    t0 = time.time()
    n = 0
    with open(out, "a", buffering=1) as fh, Pool(
            args.workers, initializer=_init,
            initargs=(args.depth, args.threads, args.hash)) as pool:
        for rec in pool.imap_unordered(_score, todo, chunksize=1):
            fh.write(json.dumps(rec) + "\n")
            n += 1
            if n % 50 == 0 or n == len(todo):
                el = time.time() - t0
                print(f"  {n}/{len(todo)}  {n/el:.2f} pos/s  "
                      f"eta {(len(todo)-n)/max(n/el, 1e-9)/60:.1f} min", flush=True)
    errs = 0
    for line in open(out):
        if "error" in json.loads(line):
            errs += 1
    print(f"done in {(time.time()-t0)/60:.1f} min; {errs} errors")


if __name__ == "__main__":
    sys.exit(main())
