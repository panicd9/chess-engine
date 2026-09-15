#!/usr/bin/env python3
"""Find the games we did not win from a won position, and the moves that let go.

`attribute.py` skips every position beyond +/-600cp as "already decided". That
is right for asking why the engine loses equal positions, and exactly wrong for
this question: a won position that is drawn or lost is where the damage was
done, and it is invisible to that script by construction.

For each game, with Stockfish 19 as the oracle and every score from OUR point of
view:

  peak       the best Stockfish score of any position we were to move in
  thrown     peak >= --won and the game was not won

and within every game, a *release* is one of our moves that takes a position of
at least --from cp and loses at least --loss cp of it. For each release the tap
tells us what the engine thought: its score when it moved and over the next
--persist of our moves, the depth it reached, and the clock it had.

Run:  ~/projects/lichess-bot/.venv/bin/python analysis/conversion.py --dir <analysis dir>
"""
import argparse
import collections
import json
import os

import chess

MATE_CP = 2000


def sf_cp(rec):
    if rec is None or "error" in rec:
        return None
    if rec.get("sf_cp") is not None:
        return rec["sf_cp"]
    m = rec.get("sf_mate")
    if m is None:
        return None
    return MATE_CP if m > 0 else -MATE_CP


def our_cp(search):
    if search is None or search.get("score") is None:
        return None
    if search.get("score_kind") == "mate":
        return MATE_CP if search["score"] > 0 else -MATE_CP
    return search["score"]


def signature(fen):
    """Material as `KRBPPPvKRPP`, strongest piece first."""
    b = chess.Board(fen)
    order = "QRBNP"
    def side(color):
        return "K" + "".join(ch * len(b.pieces(chess.Piece.from_symbol(ch).piece_type, color))
                             for ch in order)
    return side(chess.WHITE) + "v" + side(chess.BLACK)


def clock_of(go_line, white):
    if not go_line:
        return None
    parts = go_line.split()
    key = "wtime" if white else "btime"
    if key in parts:
        return int(parts[parts.index(key) + 1]) / 1000
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", required=True)
    ap.add_argument("--sf", default="sf22.jsonl")
    ap.add_argument("--won", type=int, default=250, help="peak that counts as won")
    ap.add_argument("--from", dest="frm", type=int, default=200,
                    help="a release starts from at least this")
    ap.add_argument("--loss", type=int, default=150, help="and loses at least this")
    ap.add_argument("--persist", type=int, default=4)
    ap.add_argument("--dump", default=None, help="write releases as jsonl")
    args = ap.parse_args()

    sf = {}
    for line in open(os.path.join(args.dir, args.sf)):
        r = json.loads(line)
        sf[r["fen"]] = r
    positions = [json.loads(l) for l in open(os.path.join(args.dir, "positions.jsonl"))]
    ours = [json.loads(l) for l in open(os.path.join(args.dir, "ours.jsonl"))]

    search_at = {}
    for o in ours:
        if o["ponder"] and not o.get("ponderhit"):
            continue
        if o.get("fen"):
            search_at[(o["game"], o["fen"])] = o

    by_game = collections.defaultdict(list)
    for p in positions:
        by_game[p["game"]].append(p)
    for g in by_game.values():
        g.sort(key=lambda p: p["ply"])

    releases = []
    summary = []
    for game, plies in by_game.items():
        we = plies[0]["we_played"]
        res = plies[0]["result"]
        outcome = "draw" if res == "1/2-1/2" else ("win" if (res == "1-0") == (we == "white") else "loss")
        sign = 1 if we == "white" else -1
        ours_idx = [i for i, p in enumerate(plies) if p["ours"]]
        peak, peak_ply = None, None
        for i in ours_idx:
            v = sf_cp(sf.get(plies[i]["fen"]))
            if v is not None and (peak is None or v * sign > peak):
                peak, peak_ply = v * sign, plies[i]["ply"]
        summary.append((game, outcome, peak, peak_ply, plies[0]["opponent"],
                        plies[0]["opponent_elo"], plies[0]["time_control"], len(plies)))

        for k, i in enumerate(ours_idx):
            if i + 1 >= len(plies):
                continue
            p = plies[i]
            b, a = sf_cp(sf.get(p["fen"])), sf_cp(sf.get(plies[i + 1]["fen"]))
            if b is None or a is None:
                continue
            b, a = b * sign, a * sign
            if b < args.frm or b - a < args.loss:
                continue
            s = search_at.get((game, p["fen"]))
            later = []
            for j in range(k + 1, min(k + 1 + args.persist, len(ours_idx))):
                sj = search_at.get((game, plies[ours_idx[j]]["fen"]))
                later.append(our_cp(sj))
            releases.append({
                "game": game, "outcome": outcome, "ply": p["ply"],
                "move_no": p["ply"] // 2 + 1, "san": p["san"], "fen": p["fen"],
                "sf_before": b, "sf_after": a, "loss": b - a,
                "sf_best": (sf.get(p["fen"]) or {}).get("sf_best"),
                "ours": our_cp(s), "depth": (s or {}).get("depth"),
                "pv": " ".join((s or {}).get("pv", [])[:8]),
                "ours_later": later, "clock": clock_of((s or {}).get("go"), we == "white"),
                "material": signature(p["fen"]),
                "pieces": len(chess.Board(p["fen"]).piece_map()),
            })

    print(f"{'game':<10}{'result':<7}{'peak':>6}{'@ply':>6}  opponent")
    for game, outcome, peak, peak_ply, opp, elo, tc, n in sorted(summary, key=lambda r: -(r[2] or -9999)):
        flag = "  <-- thrown" if peak is not None and peak >= args.won and outcome != "win" else ""
        print(f"{game:<10}{outcome:<7}{peak if peak is not None else '-':>6}{peak_ply if peak_ply is not None else '-':>6}  {opp} {elo} {tc}{flag}")

    thrown = {g for g, o, pk, *_ in summary if pk is not None and pk >= args.won and o != "win"}
    print(f"\n{len(thrown)} of {len(summary)} games reached >= +{args.won} and were not won\n")
    print(f"releases: our moves from >= +{args.frm} losing >= {args.loss} (* = in a thrown game)")
    for r in sorted(releases, key=lambda r: (r["game"], r["ply"])):
        star = "*" if r["game"] in thrown else " "
        later = ",".join("-" if v is None else str(v) for v in r["ours_later"])
        print(f"{star}{r['game']} {r['move_no']:>3}{'.' if r['ply'] % 2 == 0 else '...'}{r['san']:<7}"
              f" sf {r['sf_before']:+5} -> {r['sf_after']:+5}  best {r['sf_best'] or '-':<6}"
              f" ours {r['ours'] if r['ours'] is not None else '-':>5} d{r['depth'] or '-':<3}"
              f" later [{later}]  clk {r['clock'] if r['clock'] is not None else '-'}"
              f"  {r['material']} ({r['outcome']})")
    if args.dump:
        with open(args.dump, "w") as fh:
            for r in releases:
                r["thrown_game"] = r["game"] in thrown
                fh.write(json.dumps(r) + "\n")


if __name__ == "__main__":
    main()
