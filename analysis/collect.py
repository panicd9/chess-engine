#!/usr/bin/env python3
"""Build the post-mortem corpus: every position of every bot game, what the
engine thought at the time, and which build thought it.

Three sources, none of which needs the engine re-run:

- **Lichess** for the games themselves. The bulk export route
  `/api/games/user/{name}` 404s here; `/game/export/{id}` works and is cached to
  disk, so a re-run costs no requests.
- **`bot.log`** for the game ids and the colour we played.
- **`watch/events.jsonl`**, the UCI tap, for our own side. It holds every line
  the engine emitted for every move of every game -- depth, score, nodes, pv,
  time -- so "what did we think" is already on disk and does not need replaying.
  It also identifies the build, which the binary's mtime cannot: replacing a
  running executable creates a new inode and the game in progress keeps the old
  one.

Outputs (JSONL, one object per line) into --out:

    positions.jsonl   every ply of every game: fen, the move played, whose
    ours.jsonl        one record per search we ran: the position it was given,
                      the final `info` line, the move and ponder move returned

Run:  ~/projects/lichess-bot/.venv/bin/python analysis/collect.py
"""
import argparse
import bisect
import datetime
import json
import os
import re
import sys
import urllib.request

import chess
import chess.pgn

BOTLOG = os.path.expanduser("~/projects/lichess-bot/bot.log")
EVENTS = os.path.expanduser("~/projects/chess-engine-bot/watch/events.jsonl")
DEFAULT_OUT = os.path.expanduser("~/projects/chess-engine-games/analysis")
US = "LupanjeBetona"

# The instants the live engine binary was replaced, from the backup copies' mtimes:
# a backup is made from the old file immediately before the new one lands. Used to
# name the build when a game predates the tap's own md5 event.
SWAPS = [
    (0.0, "189e0b9f", "sep12-0211"),
    (datetime.datetime(2026, 9, 12, 20, 2, 0, 865602).timestamp(),
     "649b8053", "preponderfix"),
    (datetime.datetime(2026, 9, 12, 21, 49, 57, 606598).timestamp(),
     "6fc8cb2d", "HEAD 4a3566f"),
]

GAME_OVER = re.compile(
    r"^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d),\d+ .*"
    r"lichess\.org/([A-Za-z0-9]{8})/(white|black) Game over")


def games_from_log(path=BOTLOG):
    """Game id -> (end time, the colour we played). Later lines win; the bot
    logs `Game over` more than once for the same game."""
    out = {}
    with open(path, errors="replace") as fh:
        for line in fh:
            m = GAME_OVER.match(line)
            if not m:
                continue
            t = datetime.datetime.strptime(m.group(1), "%Y-%m-%d %H:%M:%S").timestamp()
            out[m.group(2)] = (t, m.group(3))
    return out


def fetch_pgn(gid, cache_dir):
    """The game, from disk if we already have it."""
    os.makedirs(cache_dir, exist_ok=True)
    path = os.path.join(cache_dir, f"{gid}.pgn")
    if os.path.exists(path) and os.path.getsize(path) > 0:
        return open(path).read()
    url = f"https://lichess.org/game/export/{gid}?clocks=true&evals=false"
    req = urllib.request.Request(url, headers={"Accept": "application/x-chess-pgn"})
    with urllib.request.urlopen(req, timeout=30) as r:
        text = r.read().decode("utf-8", "replace")
    with open(path, "w") as fh:
        fh.write(text)
    return text


def engine_sessions(path=EVENTS):
    """One record per engine process the tap saw: when it ran, its Hash, its
    md5 if it recorded one, and every (position, go, info..., bestmove) search.

    A search is delimited by `go` .. `bestmove`. The `position` line that most
    recently preceded the `go` is the position it was given. `go ponder` marks a
    search on the opponent's clock, which is not one of our moves.
    """
    procs = {}
    for line in open(path, errors="replace"):
        try:
            e = json.loads(line)
        except Exception:
            continue
        pid, d, L = e["pid"], e["d"], e["line"]
        p = procs.setdefault(pid, {
            "t0": e["t"], "t1": e["t"], "hash": None, "md5": None,
            "searches": [], "_pos": None, "_cur": None,
        })
        p["t1"] = e["t"]

        if d == "meta":
            if L.startswith("engine md5="):
                p["md5"] = L.split("=", 1)[1].split()[0][:8]
            continue

        if d == "in":
            if L.startswith("setoption name Hash"):
                p["hash"] = L.rsplit(None, 1)[-1]
            elif L.startswith("position "):
                p["_pos"] = L
            elif L.startswith("go"):
                p["_cur"] = {
                    "t": e["t"], "position": p["_pos"],
                    "ponder": " ponder" in L, "go": L, "info": [],
                }
            elif L.startswith("ponderhit") and p["_cur"] is not None:
                p["_cur"]["ponderhit"] = True
        elif d == "out":
            if L.startswith("info ") and p["_cur"] is not None:
                # Only the last info line per search matters: it is the deepest
                # completed iteration, i.e. what the move was actually chosen on.
                if " depth " in L and (" score " in L):
                    p["_cur"]["info"].append(L)
            elif L.startswith("bestmove") and p["_cur"] is not None:
                parts = L.split()
                p["_cur"]["bestmove"] = parts[1] if len(parts) > 1 else None
                p["_cur"]["ponder_move"] = parts[3] if len(parts) > 3 else None
                p["searches"].append(p["_cur"])
                p["_cur"] = None

    for p in procs.values():
        p.pop("_pos", None)
        p.pop("_cur", None)
    return procs


def build_at(t):
    cuts = [s[0] for s in SWAPS]
    return SWAPS[bisect.bisect_right(cuts, t) - 1][1:]


INFO = re.compile(r"\bdepth (\d+).*?\bscore (cp|mate) (-?\d+).*?\bnodes (\d+).*?\btime (\d+)")


def parse_info(line):
    m = INFO.search(line)
    if not m:
        return None
    pv = line.split(" pv ", 1)[1].split() if " pv " in line else []
    return {
        "depth": int(m.group(1)),
        "score_kind": m.group(2),
        "score": int(m.group(3)),   # centipawns or mate distance, side to move
        "nodes": int(m.group(4)),
        "time_ms": int(m.group(5)),
        "pv": pv,
    }


def position_of(position_line):
    """`position [startpos|fen ...] moves ...` -> (board, moves played)."""
    board = chess.Board()
    if position_line is None:
        return None, []
    rest = position_line[len("position "):]
    if rest.startswith("fen "):
        rest = rest[4:]
        fen, _, rest = rest.partition(" moves ")
        board = chess.Board(fen.strip())
        moves = rest.split()
    else:
        rest = rest[len("startpos"):].strip()
        moves = rest[len("moves"):].split() if rest.startswith("moves") else []
    for uci in moves:
        try:
            board.push_uci(uci)
        except Exception:
            break
    return board, moves


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--since", default="2026-09-12",
                    help="only games that ended on or after this date")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    pgn_dir = os.path.join(args.out, "pgn")

    since = datetime.datetime.strptime(args.since, "%Y-%m-%d").timestamp()
    games = {g: v for g, v in games_from_log().items() if v[0] >= since}
    procs = engine_sessions()
    windows = sorted((p["t0"], p["t1"], pid) for pid, p in procs.items())

    positions, ours = [], []
    for gid, (end_t, colour) in sorted(games.items(), key=lambda kv: kv[1][0]):
        try:
            pgn_text = fetch_pgn(gid, pgn_dir)
        except Exception as exc:
            print(f"{gid}: fetch failed: {exc}", file=sys.stderr)
            continue
        game = chess.pgn.read_game(__import__("io").StringIO(pgn_text))
        if game is None:
            print(f"{gid}: unparsable pgn", file=sys.stderr)
            continue

        # Which engine process played it, and therefore which build.
        w = next((w for w in windows if w[0] <= end_t <= w[1] + 2), None)
        proc = procs[w[2]] if w else None
        md5, label = build_at(w[0]) if w else ("?", "unknown")
        if proc and proc.get("md5"):
            md5 = proc["md5"]

        headers = game.headers
        board = game.board()
        for ply, move in enumerate(game.mainline_moves()):
            mover = "white" if board.turn == chess.WHITE else "black"
            positions.append({
                "game": gid, "ply": ply, "fen": board.fen(),
                "move": move.uci(), "san": board.san(move),
                "mover": mover, "ours": mover == colour,
                "we_played": colour, "result": headers.get("Result"),
                "opponent": headers.get("Black" if colour == "white" else "White"),
                "opponent_elo": headers.get("BlackElo" if colour == "white" else "WhiteElo"),
                "time_control": headers.get("TimeControl"),
                "build": md5, "build_label": label,
                "hash": proc["hash"] if proc else None,
            })
            board.push(move)

        if proc is None:
            continue
        for s in proc["searches"]:
            b, moves = position_of(s["position"])
            info = parse_info(s["info"][-1]) if s["info"] else None
            ours.append({
                "game": gid, "build": md5, "hash": proc["hash"],
                "ponder": s["ponder"], "ponderhit": s.get("ponderhit", False),
                "ply": len(moves), "fen": b.fen() if b else None,
                "bestmove": s.get("bestmove"), "ponder_move": s.get("ponder_move"),
                "go": s["go"], **(info or {}),
            })

    for name, rows in (("positions.jsonl", positions), ("ours.jsonl", ours)):
        path = os.path.join(args.out, name)
        with open(path, "w") as fh:
            for r in rows:
                fh.write(json.dumps(r) + "\n")
        print(f"{path}: {len(rows)} rows")

    builds = {}
    for r in positions:
        builds.setdefault((r["build"], r["build_label"]), set()).add(r["game"])
    for (md5, label), gs in sorted(builds.items(), key=lambda kv: kv[0][1]):
        print(f"  {md5} {label:<14} {len(gs):>2} games")


if __name__ == "__main__":
    main()
