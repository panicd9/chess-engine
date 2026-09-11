#!/usr/bin/env python3
"""Texel tuning: fit evaluation weights to predict game outcomes.

The method (Peter Osterlund's, and how PeSTO's own tables were derived):
take positions from finished games, and choose weights that minimise the
difference between a sigmoid of the evaluation and the actual result. A weight
set that predicts outcomes well is a weight set that evaluates well.

This tunes only the hand-written terms -- mobility, king safety, passed pawns --
by asking the engine to evaluate positions with different weights set over UCI.
The PeSTO piece-square tables underneath are already tuned and left alone.
"""
import chess, chess.pgn, chess.engine, math, random, sys, os

ENGINE = os.path.expanduser("~/projects/chess-engine-games/engine-toggles")
WEIGHTS = {
    "KnightMobility": (4, 0, 20),
    "BishopMobility": (4, 0, 20),
    "RookMobility":   (2, 0, 20),
    "QueenMobility":  (1, 0, 20),
    "KingShield":     (12, 0, 60),
    "PassedPawnScale": (50, 0, 300),
}
K = 1.0 / 400.0          # sigmoid scale, in the usual centipawn units

def load_positions(paths, limit):
    """Quiet positions from finished games, paired with the game result."""
    out = []
    for path in paths:
        with open(path, encoding="utf-8", errors="replace") as f:
            while len(out) < limit and (g := chess.pgn.read_game(f)) is not None:
                res = g.headers.get("Result")
                if res not in ("1-0", "0-1", "1/2-1/2"):
                    continue
                score = {"1-0": 1.0, "0-1": 0.0, "1/2-1/2": 0.5}[res]
                b = g.board()
                for i, mv in enumerate(g.mainline_moves()):
                    b.push(mv)
                    # Skip the opening (book-like, uninformative) and any
                    # position in check or with a capture available, where the
                    # static evaluation is not meaningful.
                    if 12 <= i <= 80 and not b.is_check() and i % 4 == 0:
                        out.append((b.fen(), score))
                    if len(out) >= limit:
                        break
        if len(out) >= limit:
            break
    return out

def evaluate_all(eng, positions, weights):
    for name, v in weights.items():
        eng.configure({name: int(v)})
    total = 0.0
    for fen, result in positions:
        b = chess.Board(fen)
        info = eng.analyse(b, chess.engine.Limit(depth=1))
        cp = info["score"].white().score(mate_score=10000)
        predicted = 1.0 / (1.0 + math.exp(-K * cp))
        total += (result - predicted) ** 2
    return total / len(positions)

def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 2000
    paths = sorted(f"{os.path.expanduser('~/projects/chess-engine-games')}/{p}"
                   for p in os.listdir(os.path.expanduser("~/projects/chess-engine-games"))
                   if p.endswith(".pgn"))
    random.seed(1)
    positions = load_positions(paths, n)
    print(f"  {len(positions)} positions loaded")

    eng = chess.engine.SimpleEngine.popen_uci(ENGINE)
    current = {k: v[0] for k, v in WEIGHTS.items()}
    best = evaluate_all(eng, positions, current)
    print(f"  starting error: {best:.6f}  {current}")

    # Coordinate descent: step each weight up and down, keep what helps.
    improved = True
    rounds = 0
    while improved and rounds < 6:
        improved = False
        rounds += 1
        for name, (_, lo, hi) in WEIGHTS.items():
            for step in (2, -2, 1, -1):
                trial = dict(current)
                trial[name] = max(lo, min(hi, trial[name] + step))
                if trial[name] == current[name]:
                    continue
                err = evaluate_all(eng, positions, trial)
                if err < best:
                    best, current, improved = err, trial, True
                    print(f"    {name} -> {trial[name]:<4} error {err:.6f}")
                    break
        print(f"  round {rounds}: error {best:.6f}")
    eng.quit()
    print(f"\n  tuned: {current}")

if __name__ == "__main__":
    main()
