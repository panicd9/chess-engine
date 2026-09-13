//! UCI protocol, enough of it to run in a GUI or a match runner like
//! cutechess-cli.
//!
//! The search runs on a worker thread so the main thread can keep reading
//! stdin, which is what makes `stop` and a hard time limit work: both just set
//! [`search::STOP`] and let the worker unwind.

use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::chessboard::{Chessboard, Color};
use crate::move_gen::legal_moves;
use crate::notation::{describe_move, parse_move};
use crate::search::{self, History, nega_max_alpha_beta_best_move};

/// Set when `ponderhit` arrives: the opponent played the move we predicted, so
/// the ponder search converts into a normal timed one, keeping everything it
/// has already computed.
static PONDER_HIT: AtomicBool = AtomicBool::new(false);

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
            // Evaluation weights, exposed so they can be tuned by playing
            // matches with different values rather than rebuilding each time.
            // Search technique switches, for A/B testing each in isolation.
            other if crate::search::toggles::set(
                other,
                matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "on"),
            ) && matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "false" | "1" | "0" | "on" | "off"
            ) => {}
            other => {
                if let Ok(v) = value.parse::<i32>() {
                    // `set` reports whether it recognised the name; anything
                    // else is ignored, as the protocol requires.
                    let _ = crate::eval::weights::set(other, v);
                }
            }
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
    /// Thinking on the opponent's clock. No move may be reported until
    /// `ponderhit` or `stop` arrives.
    ponder: bool,
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
    // The worker hands the `History` back when it finishes, so the table it
    // built outlives the search that built it. See `History::adopt_game`.
    let mut worker: Option<thread::JoinHandle<History>> = None;

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
                // Declared only so the GUI will send `go ponder`: cutechess sets
                // its `m_canPonder` from the presence of this option and never
                // offers a ponder search without it, so "Thinking on opponent's
                // time" silently applied to the opponent alone. The value is
                // ignored -- `go ponder` is what starts a ponder search, and
                // python-chess (the lichess bot) sends it either way.
                println!("option name Ponder type check default false");
                for name in [
                    "CheckExtensions", "History", "Futility", "Delta", "LMR", "NullMove",
                ] {
                    println!("option name {name} type check default true");
                }
                for (name, default, min, max) in [
                    ("KnightMobility", 3, 0, 30),
                    ("BishopMobility", 3, 0, 30),
                    ("RookMobility", 4, 0, 30),
                    ("QueenMobility", 3, 0, 30),
                    ("KingShield", 5, 0, 100),
                    ("PassedPawnScale", 84, 0, 400),
                ] {
                    println!("option name {name} type spin default {default} min {min} max {max}");
                }
                println!("uciok");
            }
            "setoption" => {
                let previous = options.table_megabytes;
                options.apply(&line);
                // The table now lives across moves, so a size change has to be
                // acted on here; nothing else will notice.
                if options.table_megabytes != previous {
                    stop_search(&mut worker, &mut history);
                    history.resize_table(options.table_megabytes);
                }
            }
            "isready" => println!("readyok"),
            "ucinewgame" => {
                stop_search(&mut worker, &mut history);
                board = Chessboard::new_initial_board();
                history.new_game();
            }
            "position" => {
                stop_search(&mut worker, &mut history);
                match parse_position(&line) {
                    Ok((parsed, played)) => {
                        board = parsed;
                        history.adopt_game(played);
                    }
                    Err(err) => eprintln!("info string bad position: {err}"),
                }
            }
            "go" => {
                stop_search(&mut worker, &mut history);
                PONDER_HIT.store(false, Ordering::Relaxed);
                let limits = parse_go(&line);
                // Lend the search everything we have, including the table, and
                // take it back when it finishes. Every command that reads
                // `history` calls `stop_search` first, so the placeholder left
                // behind here is never seen.
                let lent = std::mem::replace(&mut history, History::new());
                let options = options;
                worker =
                    Some(thread::spawn(move || search_and_report(board, limits, lent, options)));
            }
            // The opponent played the move we predicted. The worker is already
            // searching the right position; it just starts its clock.
            "ponderhit" => PONDER_HIT.store(true, Ordering::Relaxed),
            "stop" => stop_search(&mut worker, &mut history),
            "quit" => {
                stop_search(&mut worker, &mut history);
                return Ok(());
            }
            // Unknown commands are ignored, as the protocol requires.
            _ => {}
        }
        io::stdout().flush()?;
    }

    stop_search(&mut worker, &mut history);
    Ok(())
}

/// Ask any running search to finish, wait for it, and clear the flag ready for
/// the next one.
fn stop_search(worker: &mut Option<thread::JoinHandle<History>>, history: &mut History) {
    search::STOP.store(true, Ordering::Relaxed);
    if let Some(handle) = worker.take() {
        // Take the table back. If the worker panicked there is nothing to
        // recover and the next search builds a fresh one.
        if let Ok(returned) = handle.join() {
            *history = returned;
        }
    }
    search::STOP.store(false, Ordering::Relaxed);
}

/// Iterative deepening. Each completed depth replaces the move to play; a depth
/// cut short by the clock is thrown away, because its score came from a
/// half-searched tree.
fn search_and_report(
    board: Chessboard,
    limits: Limits,
    mut history: History,
    options: Options,
) -> History {
    let started = Instant::now();
    let budget = limits.budget(board.side_to_move, options.move_overhead);

    // While pondering the clock has not started: we are searching on the
    // opponent's time. There is no deadline until `ponderhit` arrives, at which
    // point the budget begins from that moment and everything computed so far
    // is kept. The watchdog below therefore waits for the hit before it starts
    // counting, and a ponder search that is never hit simply runs until `stop`.
    let deadline = if limits.ponder {
        None
    } else {
        budget.map(|b| started + b)
    };

    // A watchdog stops the search when the budget runs out. It is woken early
    // when the search finishes on its own, so it never outlives the search.
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let pondering = limits.ponder;
    let watchdog = if pondering {
        // Pondering: no deadline until the opponent plays our predicted move.
        // Poll for the hit, then enforce the budget from that moment.
        budget.map(|budget| {
            thread::spawn(move || {
                loop {
                    if search::STOP.load(Ordering::Relaxed) {
                        return; // `stop` arrived: the search ends on its own.
                    }
                    if PONDER_HIT.load(Ordering::Relaxed) {
                        break;
                    }
                    match done_rx.recv_timeout(Duration::from_millis(2)) {
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                }
                // The clock starts now.
                let deadline = Instant::now() + budget;
                let wait = deadline.saturating_duration_since(Instant::now());
                if done_rx.recv_timeout(wait) == Err(RecvTimeoutError::Timeout) {
                    search::STOP.store(true, Ordering::Relaxed);
                }
            })
        })
    } else {
        deadline.map(|deadline| {
            thread::spawn(move || {
                let wait = deadline.saturating_duration_since(Instant::now());
                if done_rx.recv_timeout(wait) == Err(RecvTimeoutError::Timeout) {
                    search::STOP.store(true, Ordering::Relaxed);
                }
            })
        })
    };

    let is_white = board.side_to_move == Color::White;
    let max_depth = limits.depth.unwrap_or(MAX_DEPTH);
    search::reset_nodes();
    history.ensure_table(options.table_megabytes);

    let mut best: Option<Chessboard> = None;
    // The reply the last completed iteration expected, remembered as it was
    // reported. Set together with `best`, so the two always come from the same
    // iteration and the predicted reply answers the move actually played.
    let mut predicted_reply: Option<String> = None;

    // Aspiration windows: the score at depth N is usually close to the score at
    // N-1, so search a narrow band around it rather than the full range. A
    // narrow window prunes far more. When the true score falls outside it the
    // search reports a bound instead of a value, and has to be redone wider --
    // so the window is widened on each failure until it holds.
    const ASPIRATION_INITIAL: i32 = 40;
    let mut previous: Option<i32> = None;

    for depth in 1..=max_depth {
        let (mut alpha, mut beta) = match previous {
            // Below depth 4 the score is still moving too much to guess at.
            Some(p) if depth >= 4 => (p - ASPIRATION_INITIAL, p + ASPIRATION_INITIAL),
            _ => (i32::MIN + 1, i32::MAX - 1),
        };

        let (score, position) = loop {
            let (score, position) =
                nega_max_alpha_beta_best_move(&board, depth, is_white, alpha, beta, &mut history);

            if search::STOP.load(Ordering::Relaxed) {
                break (score, position);
            }
            // Outside the window: widen on the side that failed and search
            // again. Widening to the full range at once is simplest and costs
            // little, since failures are uncommon.
            if score <= alpha {
                alpha = i32::MIN + 1;
            } else if score >= beta {
                beta = i32::MAX - 1;
            } else {
                break (score, position);
            }
        };
        previous = Some(score);

        if search::STOP.load(Ordering::Relaxed) {
            break; // Result is from an abandoned tree; keep the previous depth.
        }

        // A result that names no move means the search had nothing to play;
        // keep whatever the last real iteration found.
        if describe_move(&board, &position).is_none() {
            break;
        }
        best = Some(position);
        predicted_reply = report_info(&board, &position, score, depth, started, &history);

        // A forced mate is as good as it gets; searching deeper cannot improve it.
        if search::mate_in_plies(score).is_some() {
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

    // A ponder search must not answer until the GUI asks: `ponderhit` means
    // play it, `stop` means the prediction was wrong and the move is discarded
    // anyway. Reporting early is what made the engine appear to move instantly
    // and then ignore the rest of the protocol.
    if pondering {
        while !PONDER_HIT.load(Ordering::Relaxed) && !search::STOP.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(1));
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

    match best.and_then(|position| describe_move(&board, &position).map(|mv| (position, mv))) {
        // The `ponder` token names the reply we expect, and without it no GUI
        // will ever ponder: cutechess and python-chess both start a ponder
        // search only when the bestmove line carries one, so `option name
        // Ponder` alone buys nothing.
        Some((position, mv)) => match predicted_reply
            .or_else(|| ponder_move(&position, &history))
        {
            Some(reply) => println!("bestmove {mv} ponder {reply}"),
            None => println!("bestmove {mv}"),
        },
        // No legal move: the game is over. UCI has no "resign", and GUIs accept
        // this null move as "nothing to play".
        None => println!("bestmove 0000"),
    }
    let _ = io::stdout().flush();
    history
}

/// The reply we expect, for the `ponder` token of `bestmove`. **Fallback only**
/// -- the search remembers the reply from the PV of its last completed
/// iteration, and this runs only when it has none to offer.
///
/// Asking the table at the end of the search is not reliable, because the entry
/// for `after` can be evicted between the iteration that found it and the end of
/// the search. Measured over one session that lost the token on 2 of 87 moves,
/// and pondering is worth roughly a third of the engine's total thinking time,
/// so each loss is expensive. The PV printed with the last `info` line already
/// named the reply in both cases.
///
/// `after` is the position the move being reported leads to, so the table's
/// best move *there* is the opponent's expected reply. Asking from `after`
/// rather than walking the root's stored line keeps the suggestion consistent
/// with the move actually being played: a search aborted mid-iteration leaves a
/// root entry naming a move the completed iteration did not choose, and
/// dropping the token whenever the two disagreed cost a third of them.
///
/// `None` when nothing is stored for `after` -- an evicted entry, or a mate or
/// stalemate, where there is no reply to expect.
fn ponder_move(after: &Chessboard, history: &History) -> Option<String> {
    let line = search::principal_variation(after, &history.table, 1);
    describe_move(after, line.first()?)
}

/// Prints one `info` line and returns the reply the PV expects -- the second
/// move of the line just reported, which is the move to ponder on.
///
/// Handing it back rather than looking it up again at the end of the search is
/// the point: the caller keeps it from the last *completed* iteration, while
/// the entry it came from may be evicted before the search finishes. See
/// `ponder_move`.
fn report_info(
    root: &Chessboard,
    chosen: &Chessboard,
    score: i32,
    depth: u32,
    started: Instant,
    history: &History,
) -> Option<String> {
    let elapsed = started.elapsed();
    let millis = elapsed.as_millis().max(1) as u64;
    let nodes = search::nodes_searched();
    let nps = nodes * 1000 / millis;

    let score_text = match search::mate_in_plies(score) {
        // UCI counts mate in moves, and signs it from the side to move: a
        // negative count says the side to move is the one being mated. The
        // distance is signed, so round the magnitude and carry the sign over --
        // `(plies + 1) / 2` on a negative value rounds the wrong way.
        Some(plies) => {
            let moves = (plies.abs() + 1) / 2;
            format!("mate {}", if plies < 0 { -moves } else { moves })
        }
        None => format!("cp {score}"),
    };
    // The whole line, not just the move: a GUI shows it as the engine's plan,
    // and pondering needs the opponent's expected reply from it.
    //
    // The line starts with the move actually chosen, and only the rest of it is
    // read from the table. Walking the whole line from the table instead does
    // not guarantee the two agree: the table lives for the game, so the root can
    // already hold a deeper entry from an earlier search, and a depth-preferred
    // store then refuses to replace it with this search's move. Measured over 19
    // bot games, the table's root move disagreed with the move played on 37 of
    // 1752 searches -- 20-34% of every search returning a *mate* score, because
    // the mate break leaves the deepening loop long before the root store could
    // win. `predicted` is the reply we publish as the `ponder` token, so a
    // disagreement published a reply to a move we were not making: 8 of those
    // were not even legal in the position the opponent would actually face.
    let mut pv = String::new();
    let mut predicted = None;
    if let Some(mv) = describe_move(root, chosen) {
        pv.push_str(&mv);
    }
    let mut from = *chosen;
    for (ply, position) in
        search::principal_variation(chosen, &history.table, (depth as usize).saturating_sub(1))
            .into_iter()
            .enumerate()
    {
        // A position the walk cannot name is not one move from its parent, so
        // the rest of the line is meaningless: stop rather than print it.
        let Some(mv) = describe_move(&from, &position) else { break };
        if !pv.is_empty() {
            pv.push(' ');
        }
        pv.push_str(&mv);
        // The first move after ours is the reply to it: what the opponent is
        // expected to answer, and what to ponder.
        if ply == 0 {
            predicted = Some(mv);
        }
        from = position;
    }

    println!(
        "info depth {depth} score {score_text} nodes {nodes} nps {nps} time {millis} pv {pv}"
    );
    let _ = io::stdout().flush();
    predicted
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
            "ponder" => limits.ponder = true,
            _ => {}
        }
    }
    limits
}
