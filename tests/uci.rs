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

/// An unfinished iteration has to be abandoned when the time runs out, so both
/// a fixed `movetime` and a clock have to come back well inside their budget.
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
