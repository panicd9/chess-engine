# chess-engine

A bitboard chess engine in Rust, with a UCI interface and a terminal UI.

## Commands

```sh
cargo build --release
cargo test --release            # ~2s; do NOT run tests in debug unless testing overflow
cargo test --release -- --ignored   # deep perft, minutes to hours
cargo bench                     # criterion: perft and search

./target/release/chess-engine            # speak UCI on stdin/stdout
./target/release/chess-engine play [FEN] # terminal board
```

Always benchmark and play in `--release`. Debug builds are ~20x slower and the
engine is unusable in them.

## Layout

- `chessboard.rs` — `Chessboard` (the position), FEN parsing, and the `make_*`
  helpers that build a new board per move. Copy-make: every move produces a
  whole new 184-byte board.
- `move_gen/` — one module per piece, each with `*_legal_moves_into(cb, &mut Vec<Move>)`
  writing into a buffer the caller owns. Legality is checked by making the move
  and testing `is_*_king_under_attack`. `move_gen.rs` also has
  `*_captures_into`, the capture-only path quiescence uses, and
  `has_any_legal_move`, which tells a quiet position from stalemate.
- `move_gen/check_and_make_move.rs` — validates a move given from/to, for
  `Chessboard::make_move`. See "Known traps".
- `search.rs` — negamax + alpha-beta + quiescence, killer ordering, repetition
  and fifty-move detection, the `STOP`/`NODES` statics.
- `eval.rs` + `piece_square_tables.rs` — tapered PeSTO-style eval, from white's
  point of view.
- `uci.rs` — the UCI protocol; `playing_ui.rs` — the terminal game.
- `notation.rs` — UCI move parsing and rendering.
- `zobrist.rs` — position hashing, used by repetition detection.
- `perft.rs` — move generation correctness.

## Invariants worth knowing

**Move generation returns positions, not moves.** `legal_moves()` yields
`move_list::Move { chessboard, score }` — a whole board plus an ordering score.
There is no from/to anywhere. To name a move you diff two boards
(`notation::describe_move`); to identify one cheaply inside the search you XOR
the mover's occupancy before and after. Both are workarounds for the missing
from/to.

**`piece_square` must agree with the bitboards.** It is a redundant 64-entry
view, and the capture helpers read it to decide which piece to remove. When the
two drift, evaluation silently reads a piece that is not there and captures can
remove the wrong one. Two shipped bugs were exactly this. `tests/regressions.rs`
asserts the invariant over a move tree — keep it passing.

**`evaluate()` is from white's point of view.** Negamax needs it relative to the
side to move; the search negates for black. Getting this wrong is silent and
only shows up at odd depths.

**The transposition table lives for the game.** The worker thread returns its
`History` and `stop_search` takes it back, so the table survives from one `go`
to the next — which is worth -25% nodes over a game. Two consequences: nothing
may return a score from the table at ply 0 (that path has no move to report, and
`bestmove` would fall through to whatever `legal_moves` yields first), and
anything cached must be valid for the whole game, not just this search.

**Search scores are relative to the side to move.** Mate scores are relative to
**ply**: being mated at ply `p` scores `MATED_AT_ROOT + p`, delivering mate
there scores the negation. Ply is absolute, so a mate score means the same
thing wherever it surfaces and needs no adjusting as it propagates.
`search::mate_in_plies` is the only place that decodes them, it needs no
context to do so, and it returns a *signed* distance -- negative when the side
to move is the one being mated. Do not reimplement that arithmetic elsewhere.

This replaced an encoding relative to the *remaining depth*, which is only a
ply count while depth falls by exactly one per ply -- and it does not: a check
extension adds to it, and quiescence is entered at depth 0 and never
decrements. Mating lines are checking lines, so reported distances came out
short (a real mate in 4 announced as `mate 3`), and every mate found in
quiescence scored identically. Both are covered by `tests/uci.rs`.

## Known traps

- `check_and_make_move.rs` duplicates the move generator, with `if x != to { continue }`
  filters injected. Two bugs came from a branch forgetting its filter: a
  capture-promotion returned a quiet promotion on the wrong square, and en
  passant answered any request that reached it. **If you touch a branch there,
  check it compares against the requested destination and tests legality.**
- `*_captures_into` duplicates `*_legal_moves_into` with the target mask
  narrowed to enemy occupancy, and the pawn capture generator duplicates the
  capture, promotion and en passant branches of the full pawn generator. This is
  the same hazard as `check_and_make_move.rs` above: when the two drift,
  quiescence silently searches the wrong set of moves and perft cannot see it,
  because perft never calls the capture path. **Whatever the full generator
  produces that scores at or above the capture threshold, the capture generator
  must produce too.** All four quiet promotions score 6-9 and qualify; castling
  scores 3 and does not. A first cut dropped the quiet promotions and lost a
  mate; `tests/regressions.rs` caught it.
- Quiescence must not read an empty move list as mate or stalemate. With
  captures only, empty means "nothing to capture". In check it generates
  everything, so empty really is mate; otherwise `has_any_legal_move` settles
  it. Getting this wrong scores a stalemate as the static evaluation.
- `Chessboard` is `Copy` and cloned at every node, so its size is on the hot
  path. `ColoredPiece` must stay `#[repr(u8)]` — `repr(usize)` made
  `piece_square` 512 bytes and cost a third of move generation throughput.
  `tests/regressions.rs::board_stays_small` guards this.
- The stop flag is read on **every** node. Sampling it (every Nth node) only
  gates the check, not the work, so an abort trickles through the tree instead
  of unwinding it and time limits overshoot by ~100ms.
- `from_fen` rejects positions with a missing king or the waiting side in check.
  Without that the move generator indexes `KING_ATTACKS[64]` and panics.
- Files are CRLF; `.gitattributes` pins `eol=lf` for new work. Existing files
  are not renormalised, so avoid whole-file rewrites — they show up as
  thousands of changed lines.

## Testing

`cargo test --release` runs in ~2s and is the gate for everything:

- `src/perft.rs` — 26 assertions over the 6 standard perft positions. This is
  the oracle for move generation; if it passes, generation is almost certainly
  correct.
- `tests/regressions.rs` — one test per bug ever fixed, plus the
  `piece_square` invariant and a describe/parse/make round trip over every legal
  move in seven positions.
- `tests/uci.rs` — drives the real binary over stdin/stdout.
- `tests/fen_positions.rs` — billion-node perft, `#[ignore]`d.

`perft` only exercises move *generation*. `make_move` is a separate path used
only by UCI and the terminal UI, and both bugs found there were invisible to
perft — cover it with round-trip tests instead.

## Measuring changes

Search changes must be measured, not argued. Save the current binary, make the
change, then compare nodes-to-fixed-depth and run an A/B match with
`cutechess-cli` (installed in `~/.local/bin`). Node counts and Elo do not scale
together — a 47% node reduction bought +51 Elo.

`.cargo/config.toml` pins `target-cpu=native`; without it the engine loses ~20%
to BSF-plus-branch instead of TZCNT. Do not benchmark a build that bypassed it.

**Pin the match to one class of core.** This machine is hybrid — cpu 0-3 are
Zen5 at 5158 MHz, cpu 4-11 are Zen5c at 3289 MHz, and the identical search takes
1736 ms on one and 2394 ms on the other. Unpinned, per-process times across eight
concurrent searches spread 16-30%, which is one engine getting up to 38% more
thinking time by scheduler luck. Prefix the match with `taskset -c 4-11`
(affinity is inherited by every child) and the spread drops to 1-2%. Eight is
therefore the maximum homogeneous concurrency.

**Ponder only where the change can meet it.** The bot ponders; cutechess does
not unless each `-engine` is given `ponder`. With pondering both engines of a
game search at once, so the match needs `-concurrency 4` on `taskset -c 4-11`,
which halves throughput: 441-463 games/h at 10+0.1 against 935 without.

- **Ponder on** for time management (budgets, the soft/hard bound, pacing,
  `Ponder Charge`), the ponder path (`ponderhit`, `stop`, the ponder move), and
  anything that changes the reported PV the ponder token is read from. A
  ponderhit bug that ran every hit move to the hard bound survived every match
  that set those bounds, because none of them pondered.
- **Ponder off** for eval, search, pruning, move generation and speed. Both sides
  get the same ponder benefit, and it costs double.

Before a build goes to the bot, run the ponder tests in `tests/uci.rs` and a short
smoke match with `chess-engine-bot/watch/uciwatch.py` as the engine command,
checking the hit rate, the clock taken after a hit against a plain move, and
that nothing hangs. The ponderhit bug was plain in that timing data and invisible
to Elo at 10+0.1.

Ponder self-play is still not the bot's regime. Hits run ~68% in self-play
against ~42% live, so a ponder A/B exaggerates anything on the hit path; and
10+0.1 hides long-control behaviour -- the same bug took 2.1x the clock in the
bot's rapid games and ~5% at 10+0.1. For time-management changes the time
control matters as much as pondering does.

A short principal variation means the table is too small, not that the walk is
broken: `principal_variation` follows stored moves, and a position whose entry
has been evicted ends the line. At depth 11 from a Ruy Lopez the PV is 2 plies
at `Hash 8`, 11 plies at `Hash 64`. `examples/pvdiag` reports which of the three
reasons stopped the walk.

`examples/` holds the harnesses: `bench_all` (perft and fixed-depth search, with
an allocation counter), `equiv` (nodes, score and best move over twelve
positions — the check that a change is behaviour-preserving), `tune` (hash size
and depth sweeps) and `micro` (per-function timings: eval, zobrist, the
king-attack test, one generation call).
