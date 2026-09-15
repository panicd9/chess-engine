#!/usr/bin/env python3
"""Time two engine binaries at a fixed depth, interleaved per position.

A speed change has to be timed through the UCI binary: the engine searches on a
freshly spawned thread, and a cost that only shows there (the pawn cache's lazy
thread_local, 1.46x) is invisible to a harness that runs the search on the main
thread. Interleaving per position, alternating which binary goes first, means
load and thermal drift hit both alike.

Each (binary, position) is a fresh process, so the table starts cold for both.
When the two builds search the same tree the node counts match, and any time
difference is pure speed; the report says whether they did.

Run:  interleaved_speed.py A B positions.txt [--depth 10] [--rounds 3]
      [--options "Name=value,..."]  (applied to both)
"""
import argparse
import statistics
import subprocess


def search(binary, fen, depth, options):
    p = subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    cmds = ["uci"] + [f"setoption name {k} value {v}" for k, v in options] + [
        "setoption name Hash value 64", "isready", f"position fen {fen}", f"go depth {depth}"]
    p.stdin.write("\n".join(cmds) + "\n")
    p.stdin.flush()
    last = None
    for line in p.stdout:
        if line.startswith("info depth") and " nodes " in line and " time " in line:
            last = line.split()
        if line.startswith("bestmove"):
            best = line.split()[1]
            break
    p.stdin.write("quit\n")
    p.stdin.flush()
    p.wait()
    return int(last[last.index("nodes") + 1]), int(last[last.index("time") + 1]), best


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("positions")
    ap.add_argument("--depth", type=int, default=10)
    ap.add_argument("--rounds", type=int, default=3)
    ap.add_argument("--options", default="")
    args = ap.parse_args()
    options = [kv.split("=", 1) for kv in args.options.split(",") if kv]
    fens = [l.strip() for l in open(args.positions) if l.strip()]

    ratios, same_nodes, total = [], 0, 0
    ta = tb = 0
    for r in range(args.rounds):
        for i, fen in enumerate(fens):
            order = [(args.a, "a"), (args.b, "b")]
            if (i + r) % 2:
                order.reverse()
            res = {}
            for binary, tag in order:
                res[tag] = search(binary, fen, args.depth, options)
            (na, tma, ba), (nb, tmb, bb) = res["a"], res["b"]
            total += 1
            same_nodes += na == nb
            ta += tma
            tb += tmb
            if tma > 0:
                ratios.append(tmb / tma)
            print(f"r{r} p{i:>2}  a {na:>9} {tma:>6}ms {ba:<6}  b {nb:>9} {tmb:>6}ms {bb:<6}  "
                  f"{'same' if na == nb else 'DIFF'}  b/a {tmb / max(tma, 1):.3f}", flush=True)
    print(f"\nidentical nodes {same_nodes}/{total}; total time b/a {tb / ta:.4f}; "
          f"median per-position b/a {statistics.median(ratios):.4f}")


if __name__ == "__main__":
    main()
