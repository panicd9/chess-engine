#!/usr/bin/env python3
"""Attribute the engine's mistakes: is it the evaluation, the horizon, or time?

"We lose because of positional play" is a hypothesis, and it has been wrong here
before -- the flagship positional error of game 81n8PlYz turned out to be a
search-horizon problem, and the outpost hypothesis it generated was refuted over
66k positions. So classify before implementing.

For every move the engine chose (its search is recorded in `ours.jsonl`), with
Stockfish 19 at depth 22 as the oracle:

    loss = eval(before) - eval(after)      both from the mover's point of view

Moves losing at least --threshold centipawns are classified by what the engine
itself thought, which the wire log recorded at the time:

  seen      our own search already scored the position at least as badly as
            Stockfish does. We knew, and played it anyway: the move was forced,
            or everything else was worse. Not an evaluation gap.
  horizon   we did not see it when we moved, but our own next search -- two plies
            later and no deeper -- does. The information was one ply beyond the
            tree, not missing from the evaluation.
  blind     our own score stays comfortable for --persist further moves of ours
            while Stockfish says the position is lost. This is the only class an
            extra evaluation term can fix.

Mate scores are treated as +/- MATE_CP so a mate does not wash out the average,
and positions already decided (either side beyond --decided cp before the move)
are skipped: a mistake in a position that is already winning or lost is not
evidence about anything.

Run:  ~/projects/lichess-bot/.venv/bin/python analysis/attribute.py
"""
import argparse
import collections
import json
import os

DEFAULT_DIR = os.path.expanduser("~/projects/chess-engine-games/analysis")
MATE_CP = 2000


def sf_cp(rec):
    """Stockfish's score in centipawns from White's point of view."""
    if rec is None:
        return None
    if rec.get("sf_cp") is not None:
        return rec["sf_cp"]
    m = rec.get("sf_mate")
    if m is None:
        return None
    return MATE_CP if m > 0 else -MATE_CP


def our_cp(search):
    """Our own score in centipawns, from the side to move -- the same convention
    as `loss` below, since we are the side to move in every search."""
    if search.get("score_kind") == "mate":
        s = search["score"]
        return MATE_CP if s > 0 else -MATE_CP
    return search.get("score")


def calibrate(pairs):
    """Put our centipawns on Stockfish's scale.

    They are not the same unit. Regressing our *search* score on Stockfish 19 at
    depth 22 over this corpus gives `ours = a*sf + b` with a ~ 0.44 -- our
    evaluation is about 2.3x compressed, the same factor an independent fit of
    the *static* evaluation over 66k quiet positions found (0.417). Comparing the
    two in raw centipawns therefore makes our engine look as if it never agrees
    with Stockfish, and classifies every error as an evaluation gap.

    The intercept matters too, and is not a nuisance term: it comes out around
    +50 of our centipawns at sf = 0, i.e. the engine is systematically optimistic
    about the position it is about to move in.

    Returns (a, b); use `(ours - b) / a` to read our score in Stockfish units.
    """
    n = len(pairs)
    mx = sum(x for x, _ in pairs) / n
    my = sum(y for _, y in pairs) / n
    sxx = sum((x - mx) ** 2 for x, _ in pairs)
    a = sum((x - mx) * (y - my) for x, y in pairs) / sxx
    return a, my - a * mx


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=DEFAULT_DIR)
    ap.add_argument("--sf", default=None, help="sf<depth>.jsonl (default: sf22)")
    ap.add_argument("--threshold", type=int, default=100, help="cp loss that counts as an error")
    ap.add_argument("--decided", type=int, default=600,
                    help="skip positions already this far from equal")
    ap.add_argument("--persist", type=int, default=3,
                    help="our moves we must stay wrong for to count as blind")
    ap.add_argument("--build", default=None, help="restrict to one build md5 prefix")
    ap.add_argument("--dump", default=None, help="write the blind positions here")
    args = ap.parse_args()

    sf_path = args.sf or os.path.join(args.dir, "sf22.jsonl")
    sf = {}
    for line in open(sf_path):
        r = json.loads(line)
        if "error" not in r:
            sf[r["fen"]] = r

    positions = [json.loads(l) for l in open(os.path.join(args.dir, "positions.jsonl"))]
    ours = [json.loads(l) for l in open(os.path.join(args.dir, "ours.jsonl"))]

    # Our search, keyed by the position it was given. A ponder search that was
    # hit is a real decision: the predicted reply came, so the engine moved on
    # the strength of that search.
    search_at = {}
    for o in ours:
        if o["ponder"] and not o.get("ponderhit"):
            continue
        if o.get("fen"):
            search_at[(o["game"], o["fen"])] = o

    # Games in order, so "our next move" is well defined.
    by_game = collections.defaultdict(list)
    for p in positions:
        by_game[p["game"]].append(p)
    for g in by_game.values():
        g.sort(key=lambda p: p["ply"])

    # ---- calibration: our centipawns are not Stockfish's ---------------------
    pairs = []
    for o in ours:
        if o["ponder"] and not o.get("ponderhit"):
            continue
        if o.get("score_kind") != "cp" or not o.get("fen"):
            continue
        p = next((q for q in by_game.get(o["game"], []) if q["fen"] == o["fen"]), None)
        rec = sf.get(o["fen"])
        if p is None or rec is None:
            continue
        v = sf_cp(rec)
        if v is None:
            continue
        v *= 1 if p["mover"] == "white" else -1
        if abs(v) <= args.decided:
            pairs.append((v, o["score"]))
    scale, offset = calibrate(pairs)
    print(f"calibration over {len(pairs)} paired scores: "
          f"ours = {scale:.3f} * sf {offset:+.1f}  "
          f"(our evaluation is {1/scale:.1f}x compressed; "
          f"{offset/scale:+.0f} sf-cp optimistic at equality)\n")

    def in_sf_units(v):
        return None if v is None else (v - offset) / scale

    errors, per_game = [], collections.defaultdict(lambda: {"loss": [], "n": 0})
    for game, plies in by_game.items():
        # The positions we faced, in order, with our search and Stockfish's view.
        ours_idx = [i for i, p in enumerate(plies) if p["ours"]]
        for k, i in enumerate(ours_idx):
            p = plies[i]
            if args.build and not p["build"].startswith(args.build):
                continue
            before, after = sf.get(p["fen"]), None
            if i + 1 < len(plies):
                after = sf.get(plies[i + 1]["fen"])
            else:
                continue
            b, a = sf_cp(before), sf_cp(after)
            if b is None or a is None:
                continue
            sign = 1 if p["mover"] == "white" else -1
            b_pov, a_pov = b * sign, a * sign
            if abs(b_pov) > args.decided:
                continue                      # already decided; not evidence
            loss = b_pov - a_pov
            per_game[game]["loss"].append(max(loss, 0))
            per_game[game]["n"] += 1
            if loss < args.threshold:
                continue

            s = search_at.get((game, p["fen"]))
            # What did we think, when we moved and afterwards?
            ours_score = our_cp(s) if s else None
            # Our next search, two plies later.
            nxt = search_at.get((game, plies[ours_idx[k + 1]]["fen"])) if k + 1 < len(ours_idx) else None
            next_score = our_cp(nxt) if nxt else None
            # And the following --persist of our moves.
            later = []
            for j in range(k + 1, min(k + 1 + args.persist, len(ours_idx))):
                sj = search_at.get((game, plies[ours_idx[j]]["fen"]))
                if sj is not None:
                    later.append(our_cp(sj))

            agrees = a_pov + args.threshold / 2   # "we scored it as badly as SF"
            ours_sf = in_sf_units(ours_score)
            next_sf = in_sf_units(next_score)
            later_sf = [in_sf_units(v) for v in later]

            if ours_score is None:
                kind = "no-search"          # book move, or the pre-tap game
            elif ours_sf <= agrees:
                kind = "seen"               # we already scored it this badly
            elif next_sf is not None and next_sf <= agrees:
                kind = "horizon"            # our very next search sees it
            elif later_sf and min(later_sf) <= agrees:
                kind = "horizon"            # we see it within --persist moves
            elif later:
                kind = "blind"              # we never see it
            else:
                kind = "unclassified"       # game ended too soon to tell

            errors.append({
                "game": game, "ply": p["ply"], "san": p["san"], "move": p["move"],
                "fen": p["fen"], "build": p["build"], "loss": loss,
                "sf_before": b_pov, "sf_after": a_pov, "sf_best": (before or {}).get("sf_best"),
                "ours": ours_score, "ours_next": next_score, "ours_later": later,
                "depth": (s or {}).get("depth"), "kind": kind,
                "result": p["result"], "we_played": p["we_played"],
                "opponent": p["opponent"], "opponent_elo": p["opponent_elo"],
            })

    # ---- report ------------------------------------------------------------
    print(f"threshold {args.threshold}cp, skipping positions beyond +/-{args.decided}cp\n")
    kinds = collections.Counter(e["kind"] for e in errors)
    total = sum(kinds.values())
    print(f"{total} errors >= {args.threshold}cp over "
          f"{sum(v['n'] for v in per_game.values())} scored moves\n")
    print(f"{'class':<15}{'n':>5}{'share':>8}{'median loss':>13}{'mean loss':>11}")
    for k in ["blind", "horizon", "seen", "no-search", "unclassified"]:
        rows = [e for e in errors if e["kind"] == k]
        if not rows:
            continue
        ls = sorted(e["loss"] for e in rows)
        print(f"{k:<15}{len(rows):>5}{100*len(rows)/total:>7.0f}%"
              f"{ls[len(ls)//2]:>13}{sum(ls)/len(ls):>11.0f}")

    print("\nper game (acpl over undecided positions, our moves only):")
    print(f"{'game':<10}{'build':<10}{'n':>4}{'acpl':>7}{'err':>5}{'blind':>7}{'horiz':>7}{'seen':>6}  result")
    for game, v in sorted(per_game.items(), key=lambda kv: -sum(kv[1]["loss"]) / max(kv[1]["n"], 1)):
        if not v["n"]:
            continue
        es = [e for e in errors if e["game"] == game]
        b = es[0]["build"] if es else next((p["build"] for p in by_game[game]), "?")
        res = next((p["result"] for p in by_game[game]), "?")
        we = next((p["we_played"] for p in by_game[game]), "?")
        outcome = "draw" if res == "1/2-1/2" else ("win" if (res == "1-0") == (we == "white") else "LOSS")
        c = collections.Counter(e["kind"] for e in es)
        print(f"{game:<10}{b:<10}{v['n']:>4}{sum(v['loss'])/v['n']:>7.0f}{len(es):>5}"
              f"{c['blind']:>7}{c['horizon']:>7}{c['seen']:>6}  {outcome}")

    if args.dump:
        with open(args.dump, "w") as fh:
            for e in errors:
                if e["kind"] == "blind":
                    fh.write(json.dumps(e) + "\n")
        print(f"\nblind positions -> {args.dump}")


if __name__ == "__main__":
    main()
