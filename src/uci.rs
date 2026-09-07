//! UCI protocol, enough of it to run in a GUI or a match runner like
//! cutechess-cli.
//!
//! The search runs on a worker thread so the main thread can keep reading
//! stdin, which is what makes `stop` and a hard time limit work: both just set
//! [`search::STOP`] and let the worker unwind.

use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::chessboard::{Chessboard, Color};
use crate::move_gen::legal_moves;
use crate::notation::{describe_move, parse_move};
use crate::search::{self, nega_max_alpha_beta_best_move};

const NAME: &str = concat!("chess-engine ", env!("CARGO_PKG_VERSION"));
const AUTHOR: &str = "Darko Panic";

/// Hard ceiling on iterative deepening. Depth 8 already takes about a minute,
/// so this is only reached when the caller gives no time limit at all.
const MAX_DEPTH: u32 = 64;

/// Assumed remaining moves when the GUI does not send `movestogo`.
const EXPECTED_MOVES_LEFT: u64 = 30;

/// Held back from every time budget for process and I/O jitter.
const SAFETY_MARGIN: Duration = Duration::from_millis(30);

/// What `go` asked for.
#[derive(Debug, Default, Clone)]
struct Limits {
    depth: Option<u32>,
    movetime: Option<u64>,
    wtime: Option<u64>,
    btime: Option<u64>,
    winc: u64,
    binc: u64,
    movestogo: Option<u64>,
    infinite: bool,
}

impl Limits {
    /// How long to think, or `None` to search until told to stop.
    fn budget(&self, side_to_move: Color) -> Option<Duration> {
        if self.infinite {
            return None;
        }
        if let Some(ms) = self.movetime {
            return Some(Duration::from_millis(ms).saturating_sub(SAFETY_MARGIN));
        }

        let (remaining, increment) = match side_to_move {
            Color::White => (self.wtime?, self.winc),
            Color::Black => (self.btime?, self.binc),
        };

        // Spend an even share of the remaining time plus most of the increment,
        // but never more than a third of the clock on a single move.
        let share = remaining / self.movestogo.unwrap_or(EXPECTED_MOVES_LEFT).max(1);
        let target = (share + increment * 3 / 4).min(remaining / 3);
        Some(Duration::from_millis(target).saturating_sub(SAFETY_MARGIN))
    }
}

/// The position the GUI has set up, shared with the search thread.
#[derive(Clone)]
struct Game {
    board: Chessboard,
}

impl Default for Game {
    fn default() -> Self {
        Game { board: Chessboard::new_initial_board() }
    }
}

pub fn run() -> io::Result<()> {
    let game = Arc::new(Mutex::new(Game::default()));
    let searching = Arc::new(AtomicBool::new(false));
    let mut worker: Option<thread::JoinHandle<()>> = None;

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let mut words = line.split_whitespace();
        let Some(command) = words.next() else { continue };

        match command {
            "uci" => {
                println!("id name {NAME}");
                println!("id author {AUTHOR}");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => {
                stop_search(&searching, &mut worker);
                *game.lock().unwrap() = Game::default();
            }
            "position" => {
                stop_search(&searching, &mut worker);
                match parse_position(&line) {
                    Ok(board) => game.lock().unwrap().board = board,
                    Err(err) => eprintln!("info string bad position: {err}"),
                }
            }
            "go" => {
                stop_search(&searching, &mut worker);
                let limits = parse_go(&line);
                let board = game.lock().unwrap().board;
                worker = Some(spawn_search(board, limits, Arc::clone(&searching)));
            }
            "stop" => stop_search(&searching, &mut worker),
            "quit" => {
                stop_search(&searching, &mut worker);
                return Ok(());
            }
            // Unknown commands are ignored, as the protocol requires.
            _ => {}
        }
        io::stdout().flush()?;
    }

    stop_search(&searching, &mut worker);
    Ok(())
}

fn stop_search(searching: &Arc<AtomicBool>, worker: &mut Option<thread::JoinHandle<()>>) {
    search::STOP.store(true, Ordering::Relaxed);
    if let Some(handle) = worker.take() {
        let _ = handle.join();
    }
    searching.store(false, Ordering::Relaxed);
    search::STOP.store(false, Ordering::Relaxed);
}

fn spawn_search(
    board: Chessboard,
    limits: Limits,
    searching: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    searching.store(true, Ordering::Relaxed);
    thread::spawn(move || {
        search_and_report(board, limits);
        searching.store(false, Ordering::Relaxed);
    })
}

/// Iterative deepening. Each completed depth replaces the move to play; a depth
/// cut short by the clock is thrown away, because its score came from a
/// half-searched tree.
fn search_and_report(board: Chessboard, limits: Limits) {
    let started = Instant::now();
    let budget = limits.budget(board.side_to_move);
    let deadline = budget.map(|b| started + b);

    // A watchdog stops the search when the budget runs out. It is woken early
    // when the search finishes on its own, so it never outlives the search.
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let watchdog = deadline.map(|deadline| {
        thread::spawn(move || loop {
            let now = Instant::now();
            if now >= deadline {
                search::STOP.store(true, Ordering::Relaxed);
                return;
            }
            match done_rx.recv_timeout(deadline - now) {
                Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {
                    search::STOP.store(true, Ordering::Relaxed);
                    return;
                }
            }
        })
    });

    let is_white = board.side_to_move == Color::White;
    let max_depth = limits.depth.unwrap_or(MAX_DEPTH);
    search::reset_nodes();

    // Fall back on any legal move so we always answer with something legal.
    let mut best: Option<Chessboard> = legal_moves(&board).first().map(|m| m.chessboard);

    for depth in 1..=max_depth {
        let (score, position) =
            nega_max_alpha_beta_best_move(&board, depth, is_white, i32::MIN + 1, i32::MAX - 1);

        if search::STOP.load(Ordering::Relaxed) {
            break; // Result is from an abandoned tree; keep the previous depth.
        }

        best = Some(position);
        report_info(&board, &position, score, depth, started);

        // A forced mate is as good as it gets; searching deeper cannot improve it.
        if mate_distance_plies(score, depth).is_some() {
            break;
        }

        // Stop if the next iteration plainly cannot fit: it costs several times
        // the last one, and an unfinished depth is wasted work.
        if let Some(deadline) = deadline {
            let elapsed = started.elapsed();
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining < elapsed / 2 {
                break;
            }
        }
    }

    let _ = done_tx.send(());
    if let Some(watchdog) = watchdog {
        let _ = watchdog.join();
    }

    match best.and_then(|position| describe_move(&board, &position)) {
        Some(mv) => println!("bestmove {mv}"),
        // No legal move: the game is over. UCI has no "resign", and GUIs accept
        // this null move as "nothing to play".
        None => println!("bestmove 0000"),
    }
    let _ = io::stdout().flush();
}

fn report_info(
    root: &Chessboard,
    best: &Chessboard,
    score: i32,
    depth: u32,
    started: Instant,
) {
    let elapsed = started.elapsed();
    let millis = elapsed.as_millis().max(1) as u64;
    let nodes = search::nodes_searched();
    let nps = nodes * 1000 / millis;

    let score_text = match mate_distance_plies(score, depth) {
        // UCI counts mate in moves, and signs it from the side to move.
        Some(plies) => format!("mate {}", (plies + 1) / 2),
        None => format!("cp {score}"),
    };
    let pv = describe_move(root, best).unwrap_or_default();

    println!(
        "info depth {depth} score {score_text} nodes {nodes} nps {nps} time {millis} pv {pv}"
    );
    let _ = io::stdout().flush();
}

/// Plies to mate encoded in `score`, if it is a mate score.
///
/// `terminal_score` returns `i32::MIN + (1000 - depth_remaining)` for being
/// mated, so a mate found for the side to move comes back negated as
/// `i32::MAX - 999 + depth_remaining`, and the plies used to reach it are
/// `depth - depth_remaining`.
fn mate_distance_plies(score: i32, depth: u32) -> Option<u32> {
    const WINDOW: i32 = 2000;
    if score > i32::MAX - WINDOW {
        let depth_remaining = score - (i32::MAX - 999);
        return Some(depth.saturating_sub(depth_remaining.max(0) as u32));
    }
    if score < i32::MIN + WINDOW {
        let depth_remaining = (i32::MIN + 1000) - score;
        return Some(depth.saturating_sub(depth_remaining.max(0) as u32));
    }
    None
}

fn parse_position(line: &str) -> Result<Chessboard, String> {
    let rest = line.strip_prefix("position").ok_or("not a position command")?.trim();

    let (mut board, after_position) = if let Some(rest) = rest.strip_prefix("startpos") {
        (Chessboard::new_initial_board(), rest)
    } else if let Some(rest) = rest.strip_prefix("fen") {
        // A FEN is six space-separated fields; `moves` ends it.
        let rest = rest.trim_start();
        let (fen, after) = match rest.find(" moves") {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        (Chessboard::from_fen(fen.trim())?, after)
    } else {
        return Err(format!("expected `startpos` or `fen`, got `{rest}`"));
    };

    if let Some(moves) = after_position.trim_start().strip_prefix("moves") {
        for text in moves.split_whitespace() {
            let parsed = parse_move(text).map_err(|e| format!("{text}: {e}"))?;
            board = board
                .make_move(parsed.from, parsed.to, parsed.promotion)
                .map_err(|e| format!("{text}: {e}"))?;
        }
    }

    Ok(board)
}

fn parse_go(line: &str) -> Limits {
    let mut limits = Limits::default();
    let mut words = line.split_whitespace();
    while let Some(word) = words.next() {
        match word {
            "depth" => limits.depth = words.next().and_then(|v| v.parse().ok()),
            "movetime" => limits.movetime = words.next().and_then(|v| v.parse().ok()),
            "wtime" => limits.wtime = words.next().and_then(|v| v.parse().ok()),
            "btime" => limits.btime = words.next().and_then(|v| v.parse().ok()),
            "winc" => limits.winc = words.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "binc" => limits.binc = words.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "movestogo" => limits.movestogo = words.next().and_then(|v| v.parse().ok()),
            "infinite" => limits.infinite = true,
            _ => {}
        }
    }
    limits
}
