//! Drives the actual binary over stdin/stdout, the way a GUI does.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

struct Engine {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Engine {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_chess-engine"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to start engine");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Engine { child, stdin, stdout }
    }

    fn send(&mut self, command: &str) {
        writeln!(self.stdin, "{command}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Read until a line starts with `prefix`, returning everything read.
    fn read_until(&mut self, prefix: &str) -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("read failed");
            assert!(read > 0, "engine closed stdout waiting for `{prefix}`; got {lines:?}");
            let line = line.trim().to_string();
            let done = line.starts_with(prefix);
            lines.push(line);
            if done {
                return lines;
            }
        }
    }

    fn handshake(&mut self) {
        self.send("uci");
        let lines = self.read_until("uciok");
        assert!(lines.iter().any(|l| l.starts_with("id name")), "no id name: {lines:?}");
        self.send("isready");
        self.read_until("readyok");
    }

    fn bestmove(&mut self, position: &str, go: &str) -> (String, Vec<String>) {
        self.send(position);
        self.send(go);
        let lines = self.read_until("bestmove");
        let mv = lines.last().unwrap().split_whitespace().nth(1).unwrap().to_string();
        (mv, lines)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

#[test]
fn handshake_and_fixed_depth_search() {
    let mut engine = Engine::start();
    engine.handshake();
    let (mv, lines) = engine.bestmove("position startpos", "go depth 4");
    assert_eq!(mv.len(), 4, "expected a UCI move, got `{mv}`");
    let infos: Vec<_> = lines.iter().filter(|l| l.starts_with("info depth")).collect();
    assert!(!infos.is_empty(), "no info lines: {lines:?}");
    for field in ["score", "nodes", "nps", "time", "pv"] {
        assert!(infos.last().unwrap().contains(field), "info missing {field}: {infos:?}");
    }
}

#[test]
fn reports_forced_mate() {
    let mut engine = Engine::start();
    engine.handshake();
    let (mv, lines) = engine.bestmove(
        "position fen 6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1",
        "go depth 4",
    );
    assert_eq!(mv, "a1a8", "should find the back-rank mate");
    assert!(
        lines.iter().any(|l| l.contains("score mate 1")),
        "should report mate in 1: {lines:?}"
    );
}

/// The mirror of `reports_forced_mate`, and a bug that shipped: the mate
/// distance came back unsigned, so a position the engine was *losing* by force
/// reported `mate 1` -- "I mate in one" -- instead of `mate -1`, and every
/// consumer of the score read a loss as a win. The search was never wrong; only
/// the number it published was.
///
/// The position is from the game that found it: LupanjeBetona-ariadne-bot,
/// lichess 81n8PlYz, before White's 57th move. White is mated next move.
#[test]
fn reports_being_mated_with_a_negative_score() {
    let mut engine = Engine::start();
    engine.handshake();
    let (_, lines) = engine.bestmove(
        "position fen 8/1p3pk1/6pp/2p5/2P4P/3n1qP1/7K/3r4 w - - 2 57",
        "go depth 4",
    );
    let mates: Vec<_> = lines.iter().filter(|l| l.contains("score mate")).collect();
    assert!(!mates.is_empty(), "no mate score reported at all: {lines:?}");
    assert!(
        mates.iter().all(|l| l.contains("score mate -1")),
        "being mated in 1 must report `mate -1`, never a winning score: {mates:?}"
    );
}

/// Mate *distance* used to be measured in leftover search budget rather than in
/// plies. A check extension hands the search an extra unit of budget, and a
/// mating attack is a string of checks, so the distance came back short: this
/// position is a mate in 4 and the engine announced `mate 3`. Quiescence had
/// the same fault from the other end -- it is entered at depth 0 and never
/// decrements, so every mate found there scored as a mate at the root.
///
/// From LupanjeBetona-ariadne-bot, lichess 81n8PlYz, after White's 54th move.
/// Stockfish 19 puts the fastest mate at 4 (`Ne1+`) and the line this search
/// actually picks, `Nf4+`, at 5. Either is an honest report; the old encoding
/// announced that same `Nf4+` line as **mate 3**, which is what this guards
/// against -- so assert the distance is never understated rather than pinning a
/// number the search could legitimately improve on.
#[test]
fn reports_the_true_mate_distance_through_checks() {
    let mut engine = Engine::start();
    engine.handshake();
    let (_, lines) = engine.bestmove(
        "position fen 8/1p3pk1/6pp/2p2q2/2P4P/r2n2P1/3R2K1/8 b - - 3 54",
        "go depth 8",
    );
    let mates: Vec<i32> = lines
        .iter()
        .filter_map(|l| l.split("score mate ").nth(1))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter_map(|n| n.parse().ok())
        .collect();
    assert!(!mates.is_empty(), "no mate score reported: {lines:?}");
    for m in &mates {
        assert!(
            *m >= 4,
            "mate distance understated: reported {m}, but no mate here is faster \
             than 4 (checks along the line used to inflate the count): {lines:?}"
        );
    }
}

/// An unfinished iteration has to be abandoned when the time runs out, so both
/// a fixed `movetime` and a clock have to come back well inside their budget.
/// The principal variation must begin with the move the engine actually plays,
/// and the `ponder` token must be the reply to *that* move.
///
/// It did not. `report_info` walked the whole line out of the transposition
/// table, independently of the position the search returned, and the two can
/// disagree: the table lives for the whole game, so the root can already hold a
/// deeper entry from an earlier search, and a depth-preferred store then refuses
/// to replace it. Every search that returns a *mate* score is exposed, because
/// the mate break leaves the deepening loop long before the root store could
/// win.
///
/// Measured over 19 of the bot's games: the table's root move disagreed with the
/// move played on 37 of 1752 searches -- 20-34% of all mate scores, and none of
/// the 1620 others. Taking the ponder move from the PV (which is otherwise
/// right, and fixed a third of the tokens being wrong) turned that into a reply
/// to a move the engine was not making: 8 were not even legal in the position
/// the opponent would face.
///
/// The position is from the bot's game against follychess-com, lichess
/// M4v72Yic, replayed from move 30. The table has to be warmed by the game to
/// reach the state that fails -- the same position searched cold does not
/// reproduce it -- so the test replays the last five of its own moves in order.
#[test]
fn the_principal_variation_starts_with_the_move_played() {
    // The game, in full. The engine had Black; it is replayed from ply 58 so the
    // table reaches the state that exposed the bug, which takes five searches.
    const MOVES: &str = "d2d4 d7d5 e2e4 d5e4 c1e3 g8f6 f1b5 c7c6 b5c4 b8d7 g1e2 d7b6 \
b1d2 g7g6 e1g1 b6c4 d2c4 f8g7 c4d2 h7h5 e3g5 c8f5 d2c4 h5h4 c4e3 h4h3 e3f5 g6f5 g2h3 h8h3 \
g1g2 h3h8 c2c4 g7h6 g5f6 e7f6 d1c2 d8c7 h2h3 e8c8 f1g1 d8g8 g2h1 h6g5 h1g2 g5e3 g2f1 g8g1 \
f1g1 h8h3 f2e3 c7h2 g1f1 h3e3 a1d1 e3f3 f1e1 e4e3 e2f4 h2g3 e1e2 f3f2 e2d3 e3e2 d3d2 e2d1q \
d2d1 g3g1";
    let moves: Vec<&str> = MOVES.split_whitespace().collect();

    let mut engine = Engine::start();
    engine.handshake();
    // The size the bot plays with matters: the entry has to survive to be read
    // back, and a smaller table may evict it before the disagreement can happen.
    engine.send("setoption name Hash value 256");
    engine.send("ucinewgame");
    engine.send("isready");
    engine.read_until("readyok");

    let mut checked = 0;
    // Black moves on the odd plies. Ply 59 onwards is the last five of them.
    for ply in (59..moves.len()).step_by(2) {
        let position = format!("position startpos moves {}", moves[..ply].join(" "));
        let (mv, lines) = engine.bestmove(&position, "go depth 6");

        let info = lines
            .iter()
            .rev()
            .find(|l| l.starts_with("info ") && l.contains(" pv "))
            .unwrap_or_else(|| panic!("no info line with a pv at ply {ply}: {lines:?}"));
        let pv: Vec<&str> = info.split(" pv ").nth(1).unwrap().split_whitespace().collect();
        assert_eq!(
            pv.first().copied(),
            Some(mv.as_str()),
            "ply {ply}: the pv does not start with the move played\n  {info}\n  bestmove {mv}"
        );

        // The `ponder` token is the second move of that line, so it is the reply
        // to the move being played rather than to some other one.
        let best = lines.last().unwrap();
        if let Some(ponder) = best.split_whitespace().nth(3) {
            assert_eq!(
                pv.get(1).copied(),
                Some(ponder),
                "ply {ply}: ponder token is not the pv's reply\n  {info}\n  {best}"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 5, "expected five searches, ran {checked}");
}

#[test]
fn respects_time_limits() {
    let mut engine = Engine::start();
    engine.handshake();
    for (go, limit_ms) in [("go movetime 400", 2500), ("go wtime 2000 btime 2000", 1000)] {
        let started = Instant::now();
        let (mv, _) = engine.bestmove("position startpos", go);
        let elapsed = started.elapsed();
        assert_eq!(mv.len(), 4, "`{go}` did not return a move");
        assert!(
            elapsed.as_millis() < limit_ms,
            "`{go}` took {elapsed:?}; the search is not being interrupted"
        );
    }
}

/// Every search must report at least one completed iteration. It did not: with
/// no increment, `budget` reaches zero at `remaining <= 30 * overhead` -- 3000ms
/// at the bot's `Move Overhead: 100` -- which puts the deadline at the instant
/// the search starts, so the watchdog fires before the first node, no `info`
/// line is ever printed and `bestmove` falls through to an unsearched move.
///
/// From the position below the engine answered `a2a3`. lichess-bot subtracts its
/// own `move_overhead: 2000` before the engine sees the clock, so in a rated
/// no-increment game this began under ~5s of real clock and lasted to the end.
#[test]
fn low_clock_without_increment_still_searches() {
    let mut engine = Engine::start();
    engine.handshake();
    engine.send("setoption name Move Overhead value 100");
    for clock in [3000, 2000, 1000, 500] {
        let (mv, lines) = engine.bestmove(
            "position startpos moves e2e4 e7e5 g1f3 b8c6",
            &format!("go wtime {clock} btime {clock}"),
        );
        assert_eq!(mv.len(), 4, "no move at wtime {clock}");
        assert!(
            lines.iter().any(|l| l.starts_with("info depth")),
            "wtime {clock} reported no completed iteration, so `{mv}` was never \
             searched: {lines:?}"
        );
        assert_ne!(mv, "a2a3", "wtime {clock} played the generator's first move");
    }
}

/// An explicit `movetime` is an instruction, not a share of a clock, so the
/// floor that fixes the above must not override it.
#[test]
fn tiny_movetime_is_still_honoured() {
    let mut engine = Engine::start();
    engine.handshake();
    engine.send("setoption name Move Overhead value 100");
    let started = Instant::now();
    let (mv, _) = engine.bestmove("position startpos", "go movetime 1");
    assert_eq!(mv.len(), 4);
    assert!(
        started.elapsed().as_millis() < 200,
        "`go movetime 1` took {:?}; the floor is overriding an explicit request",
        started.elapsed()
    );
}

/// The pacing factors scale the *soft* bound only. Whatever they compute, a
/// search may never run past the hard bound -- so an explicit `movetime`, where
/// the two bounds coincide, must be honoured exactly as it was before they
/// existed. Set both factors to their maximum to make them push as hard as they
/// can.
#[test]
fn pacing_factors_never_overrun_the_hard_bound() {
    let mut engine = Engine::start();
    engine.handshake();
    engine.send("setoption name Instability Gain value 200");
    engine.send("setoption name Panic Scale value 200");
    for budget in [100u128, 200, 400] {
        let started = Instant::now();
        let (mv, _) = engine.bestmove(
            "position startpos moves e2e4 e7e5 g1f3 b8c6 f1b5",
            &format!("go movetime {budget}"),
        );
        let elapsed = started.elapsed().as_millis();
        assert_eq!(mv.len(), 4);
        assert!(
            elapsed < budget + 120,
            "movetime {budget} took {elapsed}ms with the pacing factors at maximum"
        );
    }
}

/// With both factors off the engine must pace exactly as it did before they
/// were added, so that turning them off in a match is a true baseline.
#[test]
fn pacing_factors_can_be_turned_off() {
    let mut engine = Engine::start();
    engine.handshake();
    engine.send("setoption name Instability Gain value 0");
    engine.send("setoption name Panic Scale value 0");
    let (mv, lines) = engine.bestmove(
        "position startpos moves e2e4 e7e5 g1f3 b8c6",
        "go wtime 3000 btime 3000",
    );
    assert_eq!(mv.len(), 4);
    assert!(
        lines.iter().any(|l| l.starts_with("info depth")),
        "no completed iteration with the factors off: {lines:?}"
    );
}

#[test]
fn understands_fen_and_move_lists() {
    let mut engine = Engine::start();
    engine.handshake();
    // Castling, en passant and promotion all have to survive `position ... moves`.
    let (mv, _) = engine.bestmove(
        "position fen r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1 moves e1g1 e8c8",
        "go depth 3",
    );
    assert_eq!(mv.len(), 4, "castling moves were not applied: got `{mv}`");

    let (mv, _) = engine.bestmove(
        "position fen r1r5/1P6/8/8/8/8/8/4K2k w - - 0 1 moves b7c8q",
        "go depth 2",
    );
    assert_eq!(mv.len(), 4, "promotion was not applied: got `{mv}`");
}

#[test]
fn go_infinite_stops_on_command() {
    let mut engine = Engine::start();
    engine.handshake();
    engine.send("position startpos");
    engine.send("go infinite");
    std::thread::sleep(std::time::Duration::from_millis(300));
    let started = Instant::now();
    engine.send("stop");
    let lines = engine.read_until("bestmove");
    assert!(
        started.elapsed().as_secs() < 10,
        "stop took {:?}: {lines:?}",
        started.elapsed()
    );
}

/// The abort has to unwind the tree, not trickle through it. When the stop flag
/// was only read on every 2048th node the other nodes carried on searching, and
/// a short budget overran by ~100ms -- enough to lose on time in a fast game.
#[test]
fn short_budgets_do_not_overrun() {
    let mut engine = Engine::start();
    engine.handshake();
    for budget in [60u128, 100, 200] {
        let started = Instant::now();
        let (mv, _) = engine.bestmove(
            "position startpos moves e2e4 e7e5 g1f3 b8c6 f1b5",
            &format!("go movetime {budget}"),
        );
        let elapsed = started.elapsed().as_millis();
        assert_eq!(mv.len(), 4);
        assert!(
            elapsed < budget + 120,
            "movetime {budget} took {elapsed}ms"
        );
    }
}

/// `go ponder` searches on the opponent's clock and must stay silent until the
/// GUI asks for a move: `ponderhit` (they played what we predicted, start the
/// clock) or `stop` (they did not, discard it). Answering early is a protocol
/// violation -- it was the old behaviour, and it made the engine appear to move
/// instantly and then ignore everything that followed.
#[test]
fn ponder_waits_for_the_gui() {
    let mut engine = Engine::start();
    engine.handshake();

    // Pondering: no bestmove, but it should be searching.
    engine.send("position startpos");
    engine.send("go ponder wtime 60000 btime 60000");
    std::thread::sleep(std::time::Duration::from_millis(1500));
    engine.send("isready");
    let lines = engine.read_until("readyok");
    assert!(
        !lines.iter().any(|l| l.starts_with("bestmove")),
        "answered while still pondering: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("info depth")),
        "pondering should be searching, saw no info lines: {lines:?}"
    );

    // stop: the prediction was wrong, but a move must still come back.
    engine.send("stop");
    let lines = engine.read_until("bestmove");
    let mv = lines.last().unwrap().split_whitespace().nth(1).unwrap();
    assert_eq!(mv.len(), 4, "expected a move after stop, got `{mv}`");
}

/// `ponderhit` converts a ponder search into a timed one, keeping what it has
/// already computed, and the move must arrive within the budget.
#[test]
fn ponderhit_starts_the_clock() {
    let mut engine = Engine::start();
    engine.handshake();
    engine.send("position startpos");
    engine.send("go ponder wtime 8000 btime 8000");
    std::thread::sleep(std::time::Duration::from_millis(800));

    let started = Instant::now();
    engine.send("ponderhit");
    let lines = engine.read_until("bestmove");
    let elapsed = started.elapsed();

    let mv = lines.last().unwrap().split_whitespace().nth(1).unwrap();
    assert_eq!(mv.len(), 4);
    // 8s clock over ~30 moves is roughly 270ms; allow generous slack.
    assert!(
        elapsed.as_millis() < 3000,
        "took {elapsed:?} after ponderhit; the clock did not start"
    );
}

/// The `ponder` token must be the reply from the PV of the last completed
/// iteration, not whatever the table holds once the search has finished.
///
/// The entry for the position after our move can be evicted between the
/// iteration that found it and the end of the search, and asking the table
/// then produced no token at all -- which costs the GUI a whole ponder search.
#[test]
fn ponder_token_is_the_pv_second_move() {
    let mut engine = Engine::start();
    engine.handshake();
    let (mv, lines) = engine.bestmove("position startpos", "go depth 8");

    let last_info = lines
        .iter()
        .filter(|l| l.starts_with("info depth"))
        .next_back()
        .expect("no info lines");
    let pv: Vec<&str> = last_info
        .split(" pv ")
        .nth(1)
        .expect("info line has no pv")
        .split_whitespace()
        .collect();
    assert!(pv.len() >= 2, "PV too short to predict a reply: {last_info}");

    let best_line = lines.last().unwrap();
    let fields: Vec<&str> = best_line.split_whitespace().collect();
    assert_eq!(
        fields.get(2),
        Some(&"ponder"),
        "bestmove carried no ponder token: {best_line}"
    );
    assert_eq!(mv, pv[0], "bestmove disagrees with the PV it just reported");
    assert_eq!(
        fields.get(3),
        Some(&pv[1]),
        "ponder token is not the PV's second move: {best_line} vs pv {pv:?}"
    );
}
