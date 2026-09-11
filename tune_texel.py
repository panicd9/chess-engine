#!/usr/bin/env python3
"""Texel tuning: fit evaluation weights to predict game outcomes.

The method (Peter Osterlund's, and how PeSTO's own tables were derived):
take positions from finished games, and choose weights that minimise the
difference between a sigmoid of the evaluation and the actual result. A weight
set that predicts outcomes well is a weight set that evaluates well.

This tunes only the hand-written terms -- mobility, king safety, passed pawns --
by asking the engine to evaluate positions with different weights set over UCI.
The PeSTO piece-square tables underneath are already tuned and left alone.

Four things here are load-bearing, and the first cost a whole run:

* **The baseline comes from the engine, not from this file.** An earlier version
  hard-coded a starting value for each weight, and its PassedPawnScale said 50
  where the engine's default is 100 -- a percentage, so the run began by halving
  every passed pawn bonus and, stepping by 2, could never have climbed back.
  The defaults are parsed out of the engine's own `uci` output now, so they
  cannot drift from the binary again.

* **K is fitted, not assumed.** K converts centipawns to a win probability. Fix
  it at the wrong value and the sigmoid is too flat or too steep, which flattens
  the very error differences the descent is trying to read.

* **Steps scale with the parameter.** A mobility weight lives in 0..30 and a
  percentage in 0..400; one step schedule for both either crawls over the second
  or overshoots the first. Each parameter gets a coarse-to-fine schedule sized
  to its own range.

* **A held-out split says whether it generalised.** Coordinate descent will
  always drive the training error down. The validation error is the part worth
  believing.

Two traps in the plumbing, both silent if got wrong:

* **A slice is named by the job, not by the worker.** `Pool.map` hands the next
  task to whichever worker is free, so a worker that finished early takes a
  second task while another takes none. Workers must therefore not evaluate
  "their own" positions -- each job says which slice it is for, and the totals
  are summed over slices.

* **The transposition table** lives for the game (see CLAUDE.md), so a score
  cached under the previous weight set could in principle be returned after a
  change. Measured, it is not: ply 0 never returns a table score and a depth-1
  search re-derives the rest. We pass a per-trial `game=` anyway, which makes
  python-chess send `ucinewgame`, because it costs one command per pass and the
  failure would have been invisible.
"""
import chess, chess.pgn, chess.engine, math, os, sys, json, time, random, subprocess
from multiprocessing import Pool

GAMES_DIR = os.path.expanduser("~/projects/chess-engine-games")
ENGINE = os.environ.get("TEXEL_ENGINE", os.path.join(GAMES_DIR, "engine-texel"))
CACHE = os.environ.get("TEXEL_CACHE", "/tmp/texel_positions.jsonl")
WORKERS = int(os.environ.get("TEXEL_WORKERS", "12"))

TUNED = ["KnightMobility", "BishopMobility", "RookMobility",
         "QueenMobility", "KingShield", "PassedPawnScale"]


def engine_defaults(path):
    """Starting value and bounds per weight, from the engine's own `uci` reply.

    Parsed by keyword rather than by field position: option names may contain
    spaces ("Move Overhead"), so counting fields from the left gets the wrong
    token as soon as one does.
    """
    p = subprocess.run([path], input="uci\nquit\n", capture_output=True,
                       text=True, timeout=30)
    out = {}
    for line in p.stdout.splitlines():
        f = line.split()
        if not f or f[0] != "option" or "type" not in f:
            continue
        t = f.index("type")
        if f[t + 1] != "spin":
            continue
        name = " ".join(f[1 + 1:t]) if f[1] == "name" else None
        if name not in TUNED:
            continue
        kv = {f[i]: f[i + 1] for i in range(t, len(f) - 1)
              if f[i] in ("default", "min", "max")}
        out[name] = (int(kv["default"]), int(kv["min"]), int(kv["max"]))
    if missing := [w for w in TUNED if w not in out]:
        sys.exit(f"engine does not expose these as spin options: {missing}")
    return out


def load_positions(limit):
    """Quiet positions from finished games, paired with the game result.

    Sampled every 4th ply, out of the opening and before the late endgame, and
    only where the side to move is neither in check nor holding a capture: a
    static evaluation of a position with a hanging queen says nothing about the
    weights we are trying to read.
    """
    if os.path.exists(CACHE):
        with open(CACHE) as f:
            rows = [json.loads(l) for l in f]
        if len(rows) >= limit:
            print(f"  {limit} positions from cache ({CACHE})")
            return rows[:limit]

    out, scores = [], {"1-0": 1.0, "0-1": 0.0, "1/2-1/2": 0.5}
    game_id = 0
    paths = sorted(os.path.join(GAMES_DIR, p) for p in os.listdir(GAMES_DIR)
                   if p.endswith(".pgn"))
    t0 = time.time()
    for path in paths:
        with open(path, encoding="utf-8", errors="replace") as f:
            while len(out) < limit and (g := chess.pgn.read_game(f)) is not None:
                if (res := g.headers.get("Result")) not in scores:
                    continue
                score, b = scores[res], g.board()
                game_id += 1
                for i, mv in enumerate(g.mainline_moves()):
                    b.push(mv)
                    if (12 <= i <= 80 and i % 4 == 0 and not b.is_check()
                            and not any(b.generate_legal_captures())):
                        out.append({"fen": b.fen(), "result": score,
                                    "game": game_id})
                        if len(out) >= limit:
                            break
        print(f"  {len(out):>7} positions after {os.path.basename(path)}", flush=True)
        if len(out) >= limit:
            break
    with open(CACHE, "w") as f:
        for r in out:
            f.write(json.dumps(r) + "\n")
    print(f"  loaded {len(out)} in {time.time()-t0:.0f}s")
    return out


# --- workers -----------------------------------------------------------------
# Each worker owns an engine process and the full slice table. A job names the
# slice it wants scored, so it does not matter which worker picks it up.
_W = {}


def _init(slices):
    _W["eng"] = chess.engine.SimpleEngine.popen_uci(ENGINE)
    # 1 MB is plenty at depth 1 and keeps twelve engines out of each other's
    # way in the shared cache.
    _W["eng"].configure({"Hash": 1})
    _W["slices"] = slices
    _W["boards"] = {}


def _boards(idx):
    """Boards for a slice, parsed once per worker that is asked for it."""
    if idx not in _W["boards"]:
        _W["boards"][idx] = [(chess.Board(r["fen"]), r["result"])
                             for r in _W["slices"][idx]]
    return _W["boards"][idx]


def _scores(job):
    """Raw (centipawn, result) pairs for one slice under one weight set."""
    weights, tag, idx = job
    eng = _W["eng"]
    eng.configure({k: int(v) for k, v in weights.items()})
    limit = chess.engine.Limit(depth=1)
    return [(eng.analyse(b, limit, game=tag)["score"].white().score(mate_score=10000), r)
            for b, r in _boards(idx)]


def mean_error(samples, K):
    return sum((r - 1.0 / (1.0 + math.exp(-K * cp))) ** 2
               for cp, r in samples) / len(samples)


def make_scorer(pool, n_slices):
    """Score every slice under one weight set, returning all sample pairs."""
    counter = {"n": 0}

    def collect(weights):
        counter["n"] += 1
        tag = f"t{counter['n']}"
        jobs = [(weights, tag, i) for i in range(n_slices)]
        return [s for part in pool.map(_scores, jobs, chunksize=1) for s in part]
    return collect


def fit_K(samples):
    """The sigmoid scale that best predicts the results we actually have."""
    lo, hi = 1.0 / 1000.0, 1.0 / 80.0
    for _ in range(60):                      # ternary search; unimodal in K
        a, b = lo + (hi - lo) / 3, hi - (hi - lo) / 3
        if mean_error(samples, a) < mean_error(samples, b):
            hi = b
        else:
            lo = a
    K = (lo + hi) / 2
    return K, mean_error(samples, K)


def steps_for(lo, hi):
    """Coarse-to-fine step sizes, sized to the parameter's own range."""
    out = []
    for frac in (0.08, 0.03, 0.01):
        s = max(1, int(round((hi - lo) * frac)))
        if s not in out:
            out.append(s)
    while len(out) < 3:
        out.append(1)
    return out


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 30000
    spec = engine_defaults(ENGINE)
    rows = load_positions(n)

    # Hold out a fifth of the *games*. Two reasons it is games and not
    # positions: positions from one game share a result label and are highly
    # correlated, so splitting inside a game leaks the answer across the split;
    # and the rows arrive grouped by PGN file, so taking a contiguous tail would
    # validate against a different experiment's games and measure distribution
    # shift rather than generalisation.
    ids = sorted({r["game"] for r in rows})
    random.Random(1).shuffle(ids)
    held = set(ids[:max(1, len(ids) // 5)])
    train = [r for r in rows if r["game"] not in held]
    val = [r for r in rows if r["game"] in held]
    print(f"  {len(train)} train / {len(val)} validation positions "
          f"from {len(ids)} games")

    defaults = {k: v[0] for k, v in spec.items()}
    current = dict(defaults)
    print(f"  engine defaults: {defaults}\n")

    def slices(rs):
        return [rs[i::WORKERS] for i in range(WORKERS)]

    t0 = time.time()
    with Pool(WORKERS, initializer=_init, initargs=(slices(train),)) as pool:
        collect = make_scorer(pool, WORKERS)

        K, kerr = fit_K(collect(defaults))
        print(f"  fitted K = {K:.6f}  (1 / {1/K:.0f}), error {kerr:.6f}")

        best = base_train = mean_error(collect(current), K)
        print(f"  start train error: {best:.6f}  ({time.time()-t0:.0f}s)\n")

        for step_round in range(3):
            improved = True
            while improved:
                improved = False
                for name in TUNED:
                    _, lo, hi = spec[name]
                    step = steps_for(lo, hi)[step_round]
                    for delta in (step, -step):
                        trial = dict(current)
                        trial[name] = max(lo, min(hi, trial[name] + delta))
                        if trial[name] == current[name]:
                            continue
                        e = mean_error(collect(trial), K)
                        if e < best - 1e-9:
                            best, current, improved = e, trial, True
                            print(f"    {name:<16} {current[name]:>4}   "
                                  f"train {e:.6f}", flush=True)
                            break
            print(f"  -- step round {step_round+1} settled: {best:.6f} "
                  f"({time.time()-t0:.0f}s)", flush=True)

    with Pool(WORKERS, initializer=_init, initargs=(slices(val),)) as pool:
        collect = make_scorer(pool, WORKERS)
        v_default = mean_error(collect(defaults), K)
        v_tuned = mean_error(collect(current), K)

    print("\n" + "=" * 62)
    print(f"{'weight':<18}{'default':>9}{'tuned':>9}{'change':>10}")
    print("-" * 62)
    for k in TUNED:
        print(f"{k:<18}{defaults[k]:>9}{current[k]:>9}{current[k]-defaults[k]:>+10}")
    print("-" * 62)
    print(f"train error  {base_train:.6f} -> {best:.6f}  "
          f"({(best-base_train)/base_train*100:+.2f}%)")
    print(f"val   error  {v_default:.6f} -> {v_tuned:.6f}  "
          f"({(v_tuned-v_default)/v_default*100:+.2f}%)")
    print("=" * 62)
    for k in TUNED:
        print(f"setoption name {k} value {current[k]}")
    with open(os.environ.get("TEXEL_OUT", "/tmp/texel_result.json"), "w") as f:
        json.dump({"K": K, "tuned": current, "defaults": defaults,
                   "train": [base_train, best], "val": [v_default, v_tuned],
                   "positions": len(rows)}, f, indent=2)


if __name__ == "__main__":
    main()
