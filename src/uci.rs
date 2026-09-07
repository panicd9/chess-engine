//! UCI protocol, enough of it to run in a GUI or a match runner like
//! cutechess-cli.
//!
//! The search runs on a worker thread so the main thread can keep reading
//! stdin, which is what makes `stop` and a hard time limit work: both just set
//! [`search::STOP`] and let the worker unwind.

use std::io::{self, BufRead, Write};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError};
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

pub fn run() -> io::Result<()> {
    let mut board = Chessboard::new_initial_board();
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
                stop_search(&mut worker);
                board = Chessboard::new_initial_board();
            }
            "position" => {
                stop_search(&mut worker);
                match parse_position(&line) {
                    Ok(parsed) => board = parsed,
                    Err(err) => eprintln!("info string bad position: {err}"),
                }
            }
            "go" => {
                stop_search(&mut worker);
                let limits = parse_go(&line);
                worker = Some(thread::spawn(move || search_and_report(board, limits)));
            }
            "stop" => stop_search(&mut worker),
            "quit" => {
                stop_search(&mut worker);
                return Ok(());
            }
            // Unknown commands are ignored, as the protocol requires.
            _ => {}
        }
        io::stdout().flush()?;
    }

    stop_search(&mut worker);
    Ok(())
}

/// Ask any running search to finish, wait for it, and clear the flag ready for
/// the next one.
fn stop_search(worker: &mut Option<thread::JoinHandle<()>>) {
    search::STOP.store(true, Ordering::Relaxed);
    if let Some(handle) = worker.take() {
        let _ = handle.join();
    }
    search::STOP.store(false, Ordering::Relaxed);
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
        thread::spawn(move || {
            let wait = deadline.saturating_duration_since(Instant::now());
            if done_rx.recv_timeout(wait) == Err(RecvTimeoutError::Timeout) {
                search::STOP.store(true, Ordering::Relaxed);
            }
        })
    });

    let is_white = board.side_to_move == Color::White;
    let max_depth = limits.depth.unwrap_or(MAX_DEPTH);
    search::reset_nodes();

    let mut best: Option<Chessboard> = None;

    for depth in 1..=max_depth {
        let (score, position) =
            nega_max_alpha_beta_best_move(&board, depth, is_white, i32::MIN + 1, i32::MAX - 1);

        if search::STOP.load(Ordering::Relaxed) {
            break; // Result is from an abandoned tree; keep the previous depth.
        }

        best = Some(position);
        report_info(&board, &position, score, depth, started);

        // A forced mate is as good as it gets; searching deeper cannot improve it.
        if search::mate_in_plies(score, depth).is_some() {
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

    // Stopped before even depth 1 finished: answer with any legal move rather
    // than nothing.
    if best.is_none() {
        best = legal_moves(&board).first().map(|m| m.chessboard);
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

    let score_text = match search::mate_in_plies(score, depth) {
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

fn parse_position(line: &str) -> Result<Chessboard, String> {
    let rest = line.trim_start().trim_start_matches("position").trim();

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
