#!/usr/bin/env python3
"""Positional features our evaluation does not represent, counted per position.

These are the candidates for "what is our evaluation missing". Each is a net
count from **White's point of view** (white minus black), matching the sign
convention of `evaluate()` and of the Stockfish scores we regress against, so no
conversion is needed anywhere.

Definitions follow the 2026-09-12 study so the rankings are comparable. The two
that are easy to get wrong:

* **isolated_half_open / isolated_closed.** A file holding your own pawn is never
  "half-open" for you, so the distinction is about the *enemy*: an isolated pawn
  on a file with no enemy pawn can be attacked directly by an enemy rook and can
  never be defended by a friendly pawn. That split is what the earlier study found
  mattered -- plain isolation came out *positive* (harmless), the half-open case
  negative.
* **king_zone_attackers.** Enemy pieces attacking the squares around a king, in
  *our* favour when the enemy king is attacked. Note the earlier fit gave this a
  sign no chess player would write down, which was one of the clues that matching
  Stockfish's evaluation is the wrong objective.

Run standalone to emit one JSON object per FEN on stdin.
"""
import json
import sys

import chess

NAMES = [
    "king_zone_attackers", "passed_pawn", "doubled", "isolated_half_open",
    "isolated_closed", "backward_half_open", "rook_open", "rook_half_open",
    "bishop_pair", "knight_outpost", "bishop_outpost",
]


def _pawn_files(board, colour):
    files = [0] * 8
    for sq in board.pieces(chess.PAWN, colour):
        files[chess.square_file(sq)] += 1
    return files


def _side(board, colour):
    """Every feature for one colour, as raw counts."""
    them = not colour
    mine = _pawn_files(board, colour)
    theirs = _pawn_files(board, them)
    forward = 1 if colour == chess.WHITE else -1
    f = dict.fromkeys(NAMES, 0)

    # --- pawns ------------------------------------------------------------
    for file in range(8):
        if mine[file] > 1:
            f["doubled"] += mine[file] - 1

    for sq in board.pieces(chess.PAWN, colour):
        file, rank = chess.square_file(sq), chess.square_rank(sq)
        neighbours = [x for x in (file - 1, file + 1) if 0 <= x <= 7]
        enemy_free = theirs[file] == 0

        if not any(mine[x] for x in neighbours):
            if enemy_free:
                f["isolated_half_open"] += 1
            else:
                f["isolated_closed"] += 1
        else:
            # Backward: no friendly pawn alongside or behind on a neighbouring
            # file, and the square ahead is covered by an enemy pawn.
            behind = False
            for x in neighbours:
                for other in board.pieces(chess.PAWN, colour):
                    if chess.square_file(other) != x:
                        continue
                    ahead_of_me = (chess.square_rank(other) - rank) * forward
                    if ahead_of_me <= 0:
                        behind = True
            if not behind:
                stop = chess.square(file, rank + forward) if 0 <= rank + forward <= 7 else None
                if stop is not None and enemy_free:
                    attacked = any(
                        stop in board.attacks(p)
                        for x in neighbours
                        for p in board.pieces(chess.PAWN, them)
                        if chess.square_file(p) == x
                    )
                    if attacked:
                        f["backward_half_open"] += 1

        # Passed: nothing of theirs on this file or either neighbour, ahead.
        blocked = False
        for x in [file] + neighbours:
            for p in board.pieces(chess.PAWN, them):
                if chess.square_file(p) == x and (chess.square_rank(p) - rank) * forward > 0:
                    blocked = True
        if not blocked:
            f["passed_pawn"] += 1

    # --- pieces -----------------------------------------------------------
    for sq in board.pieces(chess.ROOK, colour):
        file = chess.square_file(sq)
        if mine[file] == 0:
            if theirs[file] == 0:
                f["rook_open"] += 1
            else:
                f["rook_half_open"] += 1

    if len(board.pieces(chess.BISHOP, colour)) >= 2:
        f["bishop_pair"] = 1

    for piece, name in ((chess.KNIGHT, "knight_outpost"), (chess.BISHOP, "bishop_outpost")):
        for sq in board.pieces(piece, colour):
            rank = chess.square_rank(sq)
            advanced = rank >= 3 if colour == chess.WHITE else rank <= 4
            if not advanced:
                continue
            defended = any(
                sq in board.attacks(p) for p in board.pieces(chess.PAWN, colour)
            )
            if not defended:
                continue
            file = chess.square_file(sq)
            evictable = False
            for x in (file - 1, file + 1):
                if not 0 <= x <= 7:
                    continue
                for p in board.pieces(chess.PAWN, them):
                    if chess.square_file(p) == x and (chess.square_rank(p) - rank) * forward > 0:
                        evictable = True
            if not evictable:
                f[name] += 1

    # --- king zone --------------------------------------------------------
    king = board.king(colour)
    if king is not None:
        zone = {king} | set(board.attacks(king))
        attackers = 0
        for piece in (chess.KNIGHT, chess.BISHOP, chess.ROOK, chess.QUEEN):
            for sq in board.pieces(piece, them):
                if board.attacks(sq) & chess.SquareSet(zone):
                    attackers += 1
        # Counted for the side *being* attacked, so it enters the net figure
        # negatively below -- a feature in our favour is the enemy king attacked.
        f["king_zone_attackers"] = attackers
    return f


def features(board):
    """Net feature counts, white minus black."""
    w = _side(board, chess.WHITE)
    b = _side(board, chess.BLACK)
    # king_zone_attackers is counted on the attacked side, so flip it: positive
    # means Black's king is the one under pressure.
    out = {k: w[k] - b[k] for k in NAMES}
    out["king_zone_attackers"] = b["king_zone_attackers"] - w["king_zone_attackers"]
    return out


def main():
    for line in sys.stdin:
        fen = line.strip()
        if not fen:
            continue
        print(json.dumps({"fen": fen, **features(chess.Board(fen))}))


if __name__ == "__main__":
    main()
