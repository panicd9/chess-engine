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
use crate::search::{self, History, nega_max_alpha_beta_best_move};

const NAME: &str = concat!("chess-engine ", env!("CARGO_PKG_VERSION"));
const AUTHOR: &str = "Darko Panic";

/// Hard ceiling on iterative deepening. Depth 8 already takes about a minute,
/// so this is only reached when the caller gives no time limit at all.
const MAX_DEPTH: u32 = 64;

/// Assumed remaining moves when the GUI does not send `movestogo`.
const EXPECTED_MOVES_LEFT: u64 = 30;

/// Default transposition table size, overridable with `setoption name Hash`.
const DEFAULT_TABLE_MEGABYTES: usize = 64;
const MIN_TABLE_MEGABYTES: usize = 1;
const MAX_TABLE_MEGABYTES: usize = 1024;

/// Default time held back from every budget, overridable with
/// `setoption name Move Overhead`. GUIs raise it when the connection is slow.
const DEFAULT_MOVE_OVERHEAD_MS: u64 = 30;

/// Options a GUI may set. Anything else is accepted and ignored, as the
/// protocol requires.
#[derive(Clone, Copy)]
struct Options {
    table_megabytes: usize,
    move_overhead: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            table_megabytes: DEFAULT_TABLE_MEGABYTES,
            move_overhead: Duration::from_millis(DEFAULT_MOVE_OVERHEAD_MS),
        }
    }
}

impl Options {
    /// Parse `setoption name <name> value <value>`. Names may contain spaces,
    /// so the split is on the ` value ` separator rather than on whitespace.
    fn apply(&mut self, line: &str) {
        let Some(rest) = line.trim().strip_prefix("setoption") else { return };
        let Some(rest) = rest.trim_start().strip_prefix("name ") else { return };
        let (name, value) = match rest.split_once(" value ") {
            Some((name, value)) => (name.trim(), value.trim()),
            None => (rest.trim(), ""),
        };
        match name.to_ascii_lowercase().as_str() {
            "hash" => {
                if let Ok(mb) = value.parse::<usize>() {
                    self.table_megabytes = mb.clamp(MIN_TABLE_MEGABYTES, MAX_TABLE_MEGABYTES);
                }
            }
            "move overhead" => {
                if let Ok(ms) = value.parse::<u64>() {
                    self.move_overhead = Duration::from_millis(ms.min(5000));
                }
            }
            _ => {} // Unknown options are ignored rather than refused.
        }
    }
}



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
    fn budget(&self, side_to_move: Color, overhead: Duration) -> Option<Duration> {
        if self.infinite {
            return None;
        }
        if let Some(ms) = self.movetime {
            return Some(Duration::from_millis(ms).saturating_sub(overhead));
        }

        let (remaining, increment) = match side_to_move {
            Color::White => (self.wtime?, self.winc),
            Color::Black => (self.btime?, self.binc),
        };

        // Spend an even share of the remaining time plus most of the increment,
        // but never more than a third of the clock on a single move.
        //
        // These constants are conventional rather than tuned. A more aggressive
        // variant (share of 20, full increment) leaves 4% of the clock unused
        // instead of 12%, but measured +1.4 +/- 40.3 Elo over 240 games -- no
        // difference. Note that test could not have resolved the ~20 Elo the
        // extra time is theoretically worth; that needs ~1000 games. The bigger
        // win is probably not here at all but in spending unevenly: more when
        // the best move keeps changing between iterations, less when it does
        // not.
        let share = remaining / self.movestogo.unwrap_or(EXPECTED_MOVES_LEFT).max(1);
        let target = (share + increment * 3 / 4).min(remaining / 3);
        Some(Duration::from_millis(target).saturating_sub(overhead))
    }
}

pub fn run() -> io::Result<()> {
    let mut board = Chessboard::new_initial_board();
    let mut history = History::new();
    let mut options = Options::default();
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
                println!(
                    "option name Hash type spin default {DEFAULT_TABLE_MEGABYTES} \
                     min {MIN_TABLE_MEGABYTES} max {MAX_TABLE_MEGABYTES}"
                );
                println!(
                    "option name Move Overhead type spin default {DEFAULT_MOVE_OVERHEAD_MS} \
                     min 0 max 5000"
                );
                println!("uciok");
            }
            "setoption" => options.apply(&line),
            "isready" => println!("readyok"),
            "ucinewgame" => {
                stop_search(&mut worker);
                board = Chessboard::new_initial_board();
                history = History::new();
            }
            "position" => {
                stop_search(&mut worker);
                match parse_position(&line) {
                    Ok((parsed, played)) => {
                        board = parsed;
                        history = played;
                    }
                    Err(err) => eprintln!("info string bad position: {err}"),
                }
            }
            "go" => {
                stop_search(&mut worker);
                let limits = parse_go(&line);
                let history = history.clone();
                let options = options;
                worker =
                    Some(thread::spawn(move || search_and_report(board, limits, history, options)));
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
fn search_and_report(board: Chessboard, limits: Limits, mut history: History, options: Options) {
    let started = Instant::now();
    let budget = limits.budget(board.side_to_move, options.move_overhead);
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
    history.ensure_table(options.table_megabytes);

    let mut best: Option<Chessboard> = None;

    for depth in 1..=max_depth {
        let (score, position) = nega_max_alpha_beta_best_move(
            &board,
            depth,
            is_white,
            i32::MIN + 1,
            i32::MAX - 1,
            &mut history,
        );

        if search::STOP.load(Ordering::Relaxed) {
            break; // Result is from an abandoned tree; keep the previous depth.
        }

        // A result that names no move means the search had nothing to play;
        // keep whatever the last real iteration found.
        if describe_move(&board, &position).is_none() {
            break;
        }
        best = Some(position);
        report_info(&board, score, depth, started, &history);

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
    score: i32,
    depth: u32,
    started: Instant,
    history: &History,
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
    // The whole line, not just the move: a GUI shows it as the engine's plan,
    // and pondering needs the opponent's expected reply from it.
    let mut pv = String::new();
    let mut from = *root;
    for position in search::principal_variation(root, &history.table, depth as usize) {
        if let Some(mv) = describe_move(&from, &position) {
            if !pv.is_empty() {
                pv.push(' ');
            }
            pv.push_str(&mv);
        }
        from = position;
    }

    println!(
        "info depth {depth} score {score_text} nodes {nodes} nps {nps} time {millis} pv {pv}"
    );
    let _ = io::stdout().flush();
}

/// Parse a `position` command into the position and the history of everything
/// played to reach it, which is what repetition detection needs.
fn parse_position(line: &str) -> Result<(Chessboard, History), String> {
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

    let mut history = History::new();
    if let Some(moves) = after_position.trim_start().strip_prefix("moves") {
        for text in moves.split_whitespace() {
            let parsed = parse_move(text).map_err(|e| format!("{text}: {e}"))?;
            history.push(&board);
            board = board
                .make_move(parsed.from, parsed.to, parsed.promotion)
                .map_err(|e| format!("{text}: {e}"))?;
        }
    }

    Ok((board, history))
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
