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

/// Smallest budget a clock may produce. Without a floor, `budget` returns zero
/// whenever the overhead swallows the whole share -- with no increment that is
/// `remaining <= EXPECTED_MOVES_LEFT * overhead`, i.e. below 3s of clock at the
/// bot's `Move Overhead: 100`. A zero budget puts the deadline at `started`, so
/// the watchdog fires before the first node and `bestmove` falls through to an
/// unsearched move: `go wtime 3000 btime 3000` answered `a2a3` with no `info`
/// line at all. Stockfish guards the same way (`std::max(1.0, ...)` on its
/// optimum), and below one move's overhead the game is lost regardless -- an
/// unsearched move only makes it lost faster.
///
/// 10ms rather than 1ms because a completed iteration is what actually matters:
/// measured on this machine, 2ms reaches depth 1 from a middlegame position and
/// 1ms reaches nothing.
const MIN_BUDGET_MS: u64 = 10;

/// Default time held back from every budget, overridable with
/// `setoption name Move Overhead`. GUIs raise it when the connection is slow.
const DEFAULT_MOVE_OVERHEAD_MS: u64 = 30;

/// What share of the even slice the soft bound gets, in percent.
///
/// Not 100. Finishing the iteration in flight rather than abandoning it costs
/// real time -- measured over eight positions at a 1970ms slice, spend went
/// 1791ms -> 2517ms (+41%) for +0.74 depth. That is worth having, but it is a
/// decision about *how much* to allocate, which belongs with the allocation
/// curve and not here: mixing the two would mean an A/B that cannot say which
/// half did the work. Scaling the soft bound down holds mean spend at the old
/// figure, so the split can be measured as what it is -- the same time, spent
/// on completed iterations instead of discarded ones.
///
/// 60 is where the spend matches. Measured over the same eight positions:
///
/// ```text
/// one bound          1806 ms/move   depth 11.25
/// soft 60 / max 300  1826 ms/move   depth 11.75
/// soft 70 / max 300  2065 ms/move   depth 12.00
/// soft 80 / max 300  2393 ms/move   depth 12.12
/// ```
///
/// so +0.5 depth for +1% time. Raising it buys more depth by buying more time,
/// which is phase 4's question, not this one's.
///
/// Raised 60 -> 63 when the pacing factors arrived: they scale this bound, and
/// over 103 real games at 10+0.1 they spent 5.5% less than the unpaced build
/// (206.9 vs 219.0 ms/move). The same reasoning as above -- the A/B has to price
/// the *shape* of the spending, not a 5% cut in the amount of it.
///
/// Note the probes said -24% and the games said -5.5%. In a quiet probe position
/// the score barely moves, so `fallingEval` sits on its floor; in a real game it
/// swings and the factor spends most of its range. Calibrate this from match
/// PGNs, never from probes.
/// Trimmed 63 -> 62 when the stability factor arrived. The three factors nearly
/// cancel: phase 2 alone ran 5.5% under the unpaced build, stability pushes back
/// up, and together they measured +1.6% over 140 games at 10+0.1 (221.3 vs
/// 217.9 ms/move, depth 10.37 both sides). Probes had said +10% for the same
/// build -- the third time in this work that probes overstated a spend
/// difference that games then measured small.
const DEFAULT_SOFT_SCALE_PERCENT: u64 = 62;

/// How far past `optimum` one iteration may run before the hard stop, in
/// percent. Overridable with `setoption name Max Scale` so the value can be
/// swept by playing matches rather than rebuilding, the way the evaluation
/// weights are.
///
/// Still conservative next to Stockfish, which allows up to 687%, but it has to
/// clear the cost of one iteration or the split buys nothing. Measured here over
/// 40 iterations from six positions, an iteration costs a **median 2.31x** the
/// one before it (mean 2.81, p75 3.10, p90 5.33). At the moment the soft bound
/// is passed the last iteration accounts for about (r-1)/r of everything spent,
/// so finishing the next one lands near `r * optimum` -- which means a 200%
/// bound would cut off the median iteration just before it completed, leaving
/// exactly the wasted work the split exists to avoid. 300% clears the median
/// with room and reaches p75.
const DEFAULT_MAX_SCALE_PERCENT: u64 = 300;

/// How much of `optimum` each recent root-move change adds, in percent. 0 turns
/// the instability factor off, which is how it is A/B'd.
///
/// Stockfish uses `1.077 + 2.229 * bestMoveChanges`, counting every time a root
/// move overtakes the best *within* one iteration. Nothing here can see inside
/// an iteration -- `nega_max_alpha_beta_best_move` returns only the winner, and
/// exposing more means hoisting the root loop into the driver, which is phase 6.
/// What is visible is whether the move changed *between* iterations, which is
/// the chessprogramming.org formulation ("how often did the best move change
/// during the last N previous iterations") and a coarser, rarer signal. The
/// count is over the last [`INSTABILITY_WINDOW`] iterations, so the factor is
/// bounded at `1 + 3 * gain/100`.
const DEFAULT_INSTABILITY_GAIN: u64 = 30;

/// How many recent iterations the root-move-change count looks back over.
const INSTABILITY_WINDOW: usize = 3;

/// Weight on the falling-score factor, in percent. 0 turns it off, 100 applies
/// it in full; values between interpolate from 1.0.
const DEFAULT_PANIC_SCALE: u64 = 100;

/// Weight on the stability factor, in percent. 0 turns it off, 100 applies it
/// in full.
///
/// This is the half of uneven spending that the falling-score factor cannot
/// supply. Measured over self-games, the score-drop signal is loudest at depths
/// 1-4, where `elapsed` is near zero and no decision is being made, and has gone
/// quiet by the iteration that actually stops the search: at that moment the
/// score has converged toward the previous move's, so 48% of moves sat on the
/// factor's floor and only 5% reached its ceiling.
///
/// Move *stability* has the opposite profile -- the longer a search runs without
/// the root move changing, the stronger the evidence that it will not change --
/// so it is loudest exactly when the decision is made. Stockfish pairs it with a
/// carry (`previousTimeReduction`): a move that settled early hands its unspent
/// time to the next move, rather than letting it dissolve back into the clock
/// divided by the remaining-move estimate.
const DEFAULT_STABILITY_SCALE: u64 = 100;

/// How much of the allocation comes from the ply-scaled curve rather than the
/// even slice, in percent. 0 is the historical behaviour exactly; 100 is the
/// curve alone; values between blend the two, so it sweeps as one knob.
///
/// The even slice -- `remaining / 30 + 3/4 of the increment` -- is proportional
/// to what is left on the clock and therefore only ever falls. Measured over a
/// real match it drops monotonically from the first move: 393 ms at moves 0-4,
/// 241 at 20-24, 139 at 50-54. Stockfish instead *rises* into the middlegame and
/// only then declines -- simulated over the same 10+0.1 game, both start near
/// 225 ms, but at move 20 Stockfish is still at 102% of its opening spend where
/// we are at 79%.
///
/// That matters because the middlegame is where this engine plays worst: the
/// game post-mortem measured ACPL 36 there against 23 in the opening. We
/// underspend exactly where the errors are. And unlike the pacing factors of
/// phases 2 and 3, this is a fixed curve rather than a per-move judgement, so it
/// shifts every move in a band instead of one move in three -- iteration
/// granularity cannot swallow it.
const DEFAULT_CURVE_PERCENT: u64 = 100;

/// Ceiling on thinking when there is only one legal move, in milliseconds.
///
/// There is nothing to decide -- the move is forced -- but the search is still
/// worth a moment: it fills the transposition table for the positions after it
/// and produces a move to ponder on. Stockfish caps the same case at 500ms.
///
/// This does nothing at fast time controls, where the whole budget is already
/// under the cap. It matters at the bot's 5+3, where a forced recapture would
/// otherwise be handed seven seconds it cannot use.
const FORCED_MOVE_CEILING_MS: u64 = 500;

/// Extra soft bound granted when the GUI has pondering enabled, in percent.
///
/// Thinking continues on the opponent's clock, so time spent now is partly
/// recovered later -- Stockfish adds a quarter for the same reason. It applies
/// only when `setoption name Ponder value true` has been received, so it is
/// inert in cutechess (which sets it false) and live for the bot, which runs
/// pondering at a measured 47% hit rate.
const PONDER_BONUS_PERCENT: u64 = 25;

/// Ply at which the curve is 1.0, so the allocation there is unchanged. Move 15
/// -- roughly where a game leaves the book and the middlegame starts.
const CURVE_REFERENCE_PLY: u32 = 30;

/// The two bounds a clock implies.
///
/// `optimum` is what the search expects to need, and is read **only between
/// iterations**: passing it means "do not start another", never "stop now".
/// `maximum` is the hard stop, enforced on every node by the watchdog.
///
/// The split is what makes uneven spending possible at all. With one bound
/// serving as both, an iteration that would have finished just past it is
/// abandoned and thrown away -- and because a half-searched tree tells us
/// nothing, that time is spent for no result. Stockfish has had the two apart
/// since Glaurung; the chessprogramming.org name for them is the soft and hard
/// bound.
#[derive(Clone, Copy, Debug)]
struct Budget {
    optimum: Duration,
    maximum: Duration,
}

/// Per-search bookkeeping for uneven spending: how settled the root move is, and
/// which way the score is moving.
///
/// Both are read only between iterations, and both only ever scale `optimum`.
/// `maximum` is untouched, so however these are tuned no single move can spend
/// more than the hard bound already allowed -- the factors cannot cause a time
/// loss, only shift time between moves.
#[derive(Default)]
struct Pacing {
    /// Zobrist key of the position the last completed iteration chose, i.e. the
    /// identity of the root move. `Chessboard` is not `PartialEq`, and a hash of
    /// the resulting position distinguishes root moves for nothing: it is one
    /// hash per iteration, a handful per move.
    last_root: Option<u64>,
    /// Whether each completed iteration changed the root move, most recent last.
    changed: Vec<bool>,
    /// Scores of the last few completed iterations, most recent last.
    scores: Vec<i32>,
    /// Depth at which the root move last changed. Stockfish's
    /// `lastBestMoveDepth`: `depth - last_change_depth` is how long the current
    /// move has survived, which is the stability signal.
    last_change_depth: u32,
}

impl Pacing {
    /// Record a completed iteration.
    fn push(&mut self, root: u64, score: i32, depth: u32) {
        if let Some(previous) = self.last_root {
            let changed = previous != root;
            self.changed.push(changed);
            if changed {
                self.last_change_depth = depth;
            }
        } else {
            self.last_change_depth = depth;
        }
        self.last_root = Some(root);
        self.scores.push(score);
    }

    /// How settled the root move is, in percent, after Stockfish's
    /// `timeReduction`: interpolate the number of iterations the move has
    /// survived over 4.96..18.79 into 0.639..1.712, clamped to 0.629..1.544.
    /// Higher means more settled. Folded to integer percent:
    /// `25 + 7.76 * survived`.
    fn stability_percent(&self, depth: u32) -> u64 {
        let survived = depth.saturating_sub(self.last_change_depth) as i64;
        (25 + 776 * survived / 100).clamp(63, 154) as u64
    }

    /// How many of the last [`INSTABILITY_WINDOW`] iterations changed the root
    /// move.
    fn recent_changes(&self) -> u64 {
        self.changed
            .iter()
            .rev()
            .take(INSTABILITY_WINDOW)
            .filter(|&&c| c)
            .count() as u64
    }

    /// The factor to scale `optimum` by, in percent.
    ///
    /// Falling-score half, after Stockfish's `fallingEval`: a score that is
    /// dropping -- against the previous move and against a few iterations back
    /// -- buys more time, a stable or rising one buys less. Stockfish's
    /// coefficients are in *its* centipawns, and ours are not the same unit:
    /// regressing our search score on Stockfish over 681 paired positions gave
    /// `ours = 0.437 * sf + 51.6`, so a swing of one of our centipawns is worth
    /// about 2.29 of Stockfish's. The deltas are scaled by that before Stockfish's
    /// coefficients are applied, or the factor would be roughly half as
    /// responsive as intended. See the eval-attribution study.
    fn scale_percent(
        &self,
        previous_move: Option<i32>,
        previous_reduction: Option<u64>,
        depth: u32,
        options: &Options,
    ) -> u64 {
        let Some(&current) = self.scores.last() else { return 100 };

        // Mate scores are not on the centipawn scale at all -- they are
        // `MATED_AT_ROOT + ply` -- so a delta against one is meaningless. The
        // deepening loop already stops on a mate, so this only guards the
        // iteration that first finds one.
        let sane = |v: i32| search::mate_in_plies(v).is_none();

        let mut falling = 100i64;
        if options.panic_scale > 0 && sane(current) {
            // Positive means the score has dropped since.
            let drop_move = previous_move
                .filter(|&v| sane(v))
                .map_or(0, |v| (v - current) as i64);
            // Four iterations back, Stockfish's `iterValue` ring, or the oldest
            // we have.
            let back = self.scores.len().saturating_sub(5);
            let drop_iter = self
                .scores
                .get(back)
                .filter(|&&v| sane(v))
                .map_or(0, |&v| (v - current) as i64);

            // Stockfish: (11.48 + 2.30*d1 + 1.1*d2) / 100, clamped [0.576, 1.728].
            // In percent, with the deltas converted to Stockfish centipawns
            // (x 229/100) and folded into the coefficients: 2.30 -> 5.27,
            // 1.1 -> 2.52.
            let raw = 11 + (527 * drop_move + 252 * drop_iter) / 100;
            falling = raw.clamp(58, 173);
            // Weight it: 0 leaves the factor at 1.0, 100 applies it in full.
            falling = 100 + (falling - 100) * options.panic_scale as i64 / 100;
        }

        let instability = 100 + options.instability_gain as i64 * self.recent_changes() as i64;

        // Stockfish: reduction = (1.468 + previousTimeReduction) / (2.284 * timeReduction).
        // Note the inversion -- a *more* settled move (higher stability) divides
        // by more and so spends less, while a high *previous* reduction adds to
        // the numerator and spends more, which is the carry: last move finished
        // early, so this one may take longer.
        let mut stability = 100i64;
        if options.stability_scale > 0 {
            let tr = self.stability_percent(depth) as i64;
            let prev = previous_reduction.unwrap_or(100) as i64;
            // 100 * (1.468 + prev/100) / (2.284 * tr/100), scaled by 1000 to
            // keep it in integers: (14_680_000 + 100_000*prev) / (2284 * tr).
            // With prev = tr = 100 this is 108, Stockfish's neutral value.
            let raw = (14_680_000 + 100_000 * prev) / (2284 * tr);
            stability = raw.clamp(50, 250);
            stability = 100 + (stability - 100) * options.stability_scale as i64 / 100;
        }

        (falling * instability / 100 * stability / 100).clamp(10, 400) as u64
    }

    /// The stability factor alone, as it should be carried into the next move.
    fn carry(&self, depth: u32) -> u64 {
        self.stability_percent(depth)
    }
}

/// Options a GUI may set. Anything else is accepted and ignored, as the
/// protocol requires.
#[derive(Clone, Copy)]
struct Options {
    table_megabytes: usize,
    move_overhead: Duration,
    ponder_enabled: bool,
    soft_scale_percent: u64,
    max_scale_percent: u64,
    curve_percent: u64,
    instability_gain: u64,
    panic_scale: u64,
    stability_scale: u64,
    /// Keep a move from an iteration the clock stopped part-way. See
    /// [`may_salvage`]. An option so the change can be A/B'd in one binary.
    salvage: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            table_megabytes: DEFAULT_TABLE_MEGABYTES,
            move_overhead: Duration::from_millis(DEFAULT_MOVE_OVERHEAD_MS),
            ponder_enabled: false,
            soft_scale_percent: DEFAULT_SOFT_SCALE_PERCENT,
            max_scale_percent: DEFAULT_MAX_SCALE_PERCENT,
            curve_percent: DEFAULT_CURVE_PERCENT,
            instability_gain: DEFAULT_INSTABILITY_GAIN,
            panic_scale: DEFAULT_PANIC_SCALE,
            stability_scale: DEFAULT_STABILITY_SCALE,
            salvage: true,
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
            "ponder" => {
                self.ponder_enabled =
                    matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "on");
            }
            "curve" => {
                if let Ok(percent) = value.parse::<u64>() {
                    self.curve_percent = percent.min(200);
                }
            }
            "instability gain" => {
                if let Ok(percent) = value.parse::<u64>() {
                    self.instability_gain = percent.min(200);
                }
            }
            "stability scale" => {
                if let Ok(percent) = value.parse::<u64>() {
                    self.stability_scale = percent.min(200);
                }
            }
            "panic scale" => {
                if let Ok(percent) = value.parse::<u64>() {
                    self.panic_scale = percent.min(200);
                }
            }
            "soft scale" => {
                if let Ok(percent) = value.parse::<u64>() {
                    self.soft_scale_percent = percent.clamp(10, 200);
                }
            }
            "salvage" => {
                self.salvage = matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "on");
            }
            "max scale" => {
                if let Ok(percent) = value.parse::<u64>() {
                    self.max_scale_percent = percent.clamp(100, 1000);
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

/// Stockfish's allocation *shape*, normalised to 1.0 at [`CURVE_REFERENCE_PLY`].
///
/// Only the shape is taken, not the magnitude. Stockfish's `optimum` is about
/// half ours at the same clock -- it is calibrated against its own search, not
/// this one -- so importing the absolute value would change how much time the
/// engine spends as well as when, and the A/B could not separate the two. Our
/// own even slice supplies the magnitude; this supplies only the rise into the
/// middlegame.
///
/// The shape is `0.012112 + (ply + 3.22713)^0.46866 * optConstant` from
/// `timeman.cpp`, where the leading constant dilutes the ply term. Normalised
/// at ply 30, it runs about 0.64 in the opening to 1.4 late, so multiplied into
/// a slice that falls with the clock it gives a curve that rises into the
/// middlegame and declines after -- which is the whole point.
///
/// `optConstant` depends on the clock, so the curve is slightly steeper at
/// longer time controls. That is Stockfish's design and is kept.
fn ply_shape(ply: u32, remaining_ms: u64) -> f64 {
    let log_time = (remaining_ms as f64 / 1000.0).max(1e-9).log10();
    let opt_constant = (0.0029869 + 0.00033554 * log_time).min(0.004905);
    let at = |p: f64| 0.012112 + (p + 3.22713).powf(0.46866) * opt_constant;
    at(ply as f64) / at(CURVE_REFERENCE_PLY as f64)
}

impl Limits {
    /// The bounds to think within, or `None` to search until told to stop.
    fn budget(
        &self,
        side_to_move: Color,
        options: &Options,
        ply: u32,
        forced: bool,
    ) -> Option<Budget> {
        if self.infinite {
            return None;
        }
        let overhead_ms = options.move_overhead.as_millis() as u64;
        if let Some(ms) = self.movetime {
            // An explicit `movetime` is an instruction rather than a share of a
            // clock, so the floor may not exceed it -- a GUI asking for 1ms gets
            // 1ms -- and there is no headroom to grant either: exceeding what
            // was asked for would be a protocol violation, so the two bounds
            // coincide and the watchdog enforces the number given.
            let target = ms.saturating_sub(overhead_ms).max(MIN_BUDGET_MS.min(ms));
            let target = Duration::from_millis(target);
            return Some(Budget { optimum: target, maximum: target });
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
        let target = target.saturating_sub(overhead_ms);
        // Blend the even slice with the ply-scaled curve, then apply the soft
        // scale to whatever came out. Scaling only one of the two would make
        // `Curve` change the *amount* of time as well as its distribution, and
        // the A/B could not then say which produced the result -- the same trap
        // phases 1 and 2 each had to be calibrated out of.
        let blended = if options.curve_percent == 0 {
            target
        } else {
            // Weight the shape: 100 applies it in full, 0 leaves the slice flat.
            let shape = 1.0
                + (ply_shape(ply, remaining) - 1.0) * options.curve_percent as f64 / 100.0;
            (target as f64 * shape.max(0.05)) as u64
        };
        let mut optimum = (blended * options.soft_scale_percent / 100).max(MIN_BUDGET_MS);

        // Headroom for an iteration already in flight, bounded twice: by the
        // multiplier, and by half of what is left on the clock so that no single
        // move can put the game in danger however the multiplier is set.
        //
        // Computed from the *unadjusted* optimum, before the ponder bonus below.
        // Stockfish orders it the same way, and the reason matters: the bonus is
        // a licence to think longer because time comes back on the opponent's
        // clock, not a licence to raise the ceiling on a single move. Applying it
        // first lifted the hard bound from 18.7s to 23.3s at 5+3 -- more risk
        // than was asked for, and a unit test caught it.
        let mut maximum = (optimum * options.max_scale_percent / 100)
            .min(remaining.saturating_sub(overhead_ms) / 2)
            .max(optimum);

        // Pondering keeps the search running on the opponent's clock, so time
        // spent here is partly recovered.
        if options.ponder_enabled {
            optimum += optimum * PONDER_BONUS_PERCENT / 100;
        }
        // Nothing to decide when the move is forced. Bring the hard bound down
        // with it -- otherwise an iteration started just under the ceiling could
        // still run to the full hard bound on a move with no alternatives.
        if forced {
            optimum = optimum.min(FORCED_MOVE_CEILING_MS).max(MIN_BUDGET_MS);
            maximum = maximum
                .min(optimum * options.max_scale_percent / 100)
                .max(optimum);
        }
        // The soft bound may never exceed the hard one, however it was adjusted.
        let optimum = optimum.min(maximum);

        Some(Budget {
            optimum: Duration::from_millis(optimum),
            maximum: Duration::from_millis(maximum),
        })
    }
}

/// Whether a root search the clock stopped part-way may replace the move the
/// last completed iteration chose.
///
/// A stopped iteration used to be discarded whole. For the moves it did not
/// finish that is right, but the ones it did finish carry genuine scores at the
/// new depth, and an iteration costs a median 2.31x the one before it -- so
/// discarding it throws away most of the time spent on the move. Measured over
/// 790 self-play moves at 10+0.1: 19% ended in a stopped iteration, and on 2.0%
/// of all moves a finished root move had beaten the previous choice at the new
/// depth. Those are the moves this changes.
///
/// Every condition is there because the comparison means nothing without it:
///
/// - `salvaged` must itself be a finished move. A root with nothing finished
///   hands back its own position.
/// - The previous choice must be among the finished moves, or the new move was
///   never compared with it at this depth. It is usually searched first but not
///   always: the table outlives the search, and a depth-preferred store can
///   keep an older, deeper root entry naming a different move.
/// - The score must be above the window's lower edge. At or below it, it is only
///   an upper bound -- everything finished was failing low -- and a bound says
///   nothing about which move is better.
/// - Replacing a move with itself changes nothing, and would re-report a
///   partial depth for no gain.
///
/// With no previous choice at all, any finished move beats the fallback of
/// taking the best-ordered legal move unsearched.
fn may_salvage(
    previous: Option<u64>,
    completed: &[u64],
    salvaged: u64,
    score: i32,
    window_alpha: i32,
) -> bool {
    if !completed.contains(&salvaged) || score <= window_alpha {
        return false;
    }
    match previous {
        None => true,
        Some(previous) => previous != salvaged && completed.contains(&previous),
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
                println!(
                    "option name Soft Scale type spin default {DEFAULT_SOFT_SCALE_PERCENT} \
                     min 10 max 200"
                );
                println!(
                    "option name Curve type spin default {DEFAULT_CURVE_PERCENT} \
                     min 0 max 200"
                );
                println!(
                    "option name Instability Gain type spin default \
                     {DEFAULT_INSTABILITY_GAIN} min 0 max 200"
                );
                println!(
                    "option name Panic Scale type spin default {DEFAULT_PANIC_SCALE} \
                     min 0 max 200"
                );
                println!(
                    "option name Stability Scale type spin default \
                     {DEFAULT_STABILITY_SCALE} min 0 max 200"
                );
                println!(
                    "option name Max Scale type spin default {DEFAULT_MAX_SCALE_PERCENT} \
                     min 100 max 1000"
                );
                println!("option name Salvage type check default true");
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
                    // Adopted after +31.0 at 10+0.1 and +36.4 at 40+0.4; see
                    // `eval::weights`. Must match the statics there.
                    ("DoubledPawn", 18, 0, 100),
                    ("IsolatedHalfOpenPawn", 12, 0, 100),
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
    // Before the clock starts: sizing the table is setup, not thinking, and
    // charging it to the move is a straight loss. Measured at 17ms on the first
    // search of a game at the default `Hash 64` -- 4.5% of a whole move at
    // 10+0.1, and fatal next to a floor of a few milliseconds. It is a no-op on
    // every later search, so only the first move of a game ever paid it.
    search::reset_nodes();
    history.ensure_table(options.table_megabytes);

    let started = Instant::now();
    // Game ply from the board itself rather than the move list, so it is still
    // right when a GUI hands us a mid-game FEN with no moves after it.
    let ply = (board.fullmove_counter.saturating_sub(1)) * 2
        + u32::from(board.side_to_move == Color::Black);
    // One legal move means nothing to choose; the search still runs, briefly,
    // to fill the table and name a ponder move. Generating once here costs a
    // single move generation per `go`, against a search of millions of nodes.
    let forced = legal_moves(&board).len() == 1;
    let budget = limits.budget(board.side_to_move, &options, ply, forced);

    // While pondering the clock has not started: we are searching on the
    // opponent's time. There is no deadline until `ponderhit` arrives, at which
    // point the budget begins from that moment and everything computed so far
    // is kept. The watchdog below therefore waits for the hit before it starts
    // counting, and a ponder search that is never hit simply runs until `stop`.
    let deadline = if limits.ponder {
        None
    } else {
        budget.map(|b| started + b.maximum)
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
                let deadline = Instant::now() + budget.maximum;
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

    // How settled the root move is and which way the score is going. Read only
    // between iterations, and only ever scales `optimum`.
    let mut pacing = Pacing::default();
    let previous_move_score = history.previous_move_score;
    let previous_reduction = history.previous_time_reduction;
    // Deepest iteration that finished, for the stability carry.
    let mut reached_depth = 0u32;

    for depth in 1..=max_depth {
        let (mut alpha, mut beta) = match previous {
            // Below depth 4 the score is still moving too much to guess at.
            Some(p) if depth >= 4 => (p - ASPIRATION_INITIAL, p + ASPIRATION_INITIAL),
            _ => (i32::MIN + 1, i32::MAX - 1),
        };

        let (score, position, window_alpha) = loop {
            history.root_completed.clear();
            let (score, position) =
                nega_max_alpha_beta_best_move(&board, depth, is_white, alpha, beta, &mut history);

            if search::STOP.load(Ordering::Relaxed) {
                break (score, position, alpha);
            }
            // Outside the window: widen on the side that failed and search
            // again. Widening to the full range at once is simplest and costs
            // little, since failures are uncommon.
            if score <= alpha {
                alpha = i32::MIN + 1;
            } else if score >= beta {
                beta = i32::MAX - 1;
            } else {
                break (score, position, alpha);
            }
        };
        previous = Some(score);

        if search::STOP.load(Ordering::Relaxed) {
            // Stopped part-way. Keep the previous depth's move -- unless a move
            // this iteration finished has already beaten it. See `may_salvage`.
            if options.salvage {
                let previous_choice = best.map(|b| crate::zobrist::hash(&b));
                let salvaged = crate::zobrist::hash(&position);
                if may_salvage(
                    previous_choice,
                    &history.root_completed,
                    salvaged,
                    score,
                    window_alpha,
                ) && describe_move(&board, &position).is_some()
                {
                    best = Some(position);
                    // A finished move beating the previous choice *is* a root
                    // move change at this depth. Record it, or the carry and the
                    // score handed to the next move describe the choice that was
                    // just replaced rather than the move being played.
                    pacing.push(crate::zobrist::hash(&position), score, depth);
                    reached_depth = depth;
                    // Re-derived from the move actually kept, never carried over:
                    // the principal variation must start with the move played,
                    // and the ponder token must answer *that* move. A reply to
                    // the previous choice is exactly the shipped bug that made
                    // ponder tokens illegal.
                    predicted_reply =
                        report_info(&board, &position, score, depth, started, &history);
                }
            }
            break;
        }

        // A result that names no move means the search had nothing to play;
        // keep whatever the last real iteration found.
        if describe_move(&board, &position).is_none() {
            break;
        }
        best = Some(position);
        pacing.push(crate::zobrist::hash(&position), score, depth);
        reached_depth = depth;
        predicted_reply = report_info(&board, &position, score, depth, started, &history);

        // A forced mate is as good as it gets; searching deeper cannot improve it.
        if search::mate_in_plies(score).is_some() {
            break;
        }

        // The soft bound. Past `optimum`, do not start another iteration --
        // but leave alone whatever is already running, which the watchdog holds
        // to `maximum`. An iteration abandoned part-way is discarded whole, so
        // once one is under way the choice is between finishing it and having
        // spent the time for nothing.
        //
        // This replaces `remaining < elapsed / 2`, which asked whether the next
        // iteration would fit inside the *hard* deadline and then let the
        // watchdog kill it when the guess was wrong. That guess is only a ply
        // count while each iteration costs exactly twice the last, and the cost
        // ratio is neither constant nor 2.
        //
        // `deadline` rather than `budget` is the guard because a ponder search
        // has bounds but no deadline: it is on the opponent's clock, so nothing
        // limits it until `ponderhit`, after which the watchdog counts from the
        // hit and this loop has no way to measure from there.
        if deadline.is_some() {
            if let Some(budget) = budget {
                // Spend unevenly: an unsettled root move or a score that is
                // falling earns more of the clock, a settled one less. This only
                // moves the *soft* bound -- `maximum` is untouched, so the
                // factors can shift time between moves but can never overrun the
                // hard limit or lose on time.
                let scale =
                    pacing.scale_percent(previous_move_score, previous_reduction, depth, &options);
                let want = (budget.optimum.as_millis() as u64 * scale / 100).max(1);
                let soft = Duration::from_millis(want).min(budget.maximum);
                if started.elapsed() >= soft {
                    break;
                }
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

    // Hand this move's conclusion to the next one, for the falling-score
    // factor. `History` is what survives the search, so it is what carries it.
    //
    // Not from a ponder search that was never hit. That search was of the
    // position after the reply we *predicted*, and when the prediction is wrong
    // -- measured at 53% of the bot's ponder searches -- that position never
    // occurs. Filing its score away would make the next real search compare
    // against a line that never existed and read a swing that never happened.
    // A ponder search that *was* hit is the real search for the move being
    // played, so its score is kept like any other.
    let ponder_missed = pondering && !PONDER_HIT.load(Ordering::Relaxed);
    if !ponder_missed {
        if let Some(&settled) = pacing.scores.last() {
            history.previous_move_score = Some(settled);
        }
        // The carry: how settled this move turned out to be sets how much the
        // next one may take. Same reasoning as the score above -- a ponder that
        // was never hit describes a position that did not happen.
        history.previous_time_reduction = Some(pacing.carry(reached_depth));
    }

    // Stopped before even depth 1 finished: answer with any legal move rather
    // than nothing.
    if best.is_none() {
        // Ordered by the same score the search would have used, so even this
        // path plays a winning capture rather than whatever the generator
        // happened to emit first -- which was `a2a3` from the opening position.
        best = legal_moves(&board)
            .into_iter()
            .max_by_key(|m| m.score)
            .map(|m| m.chessboard);
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

#[cfg(test)]
mod curve_tests {
    use super::*;

    /// The curve must rise with ply, sit at exactly 1.0 at the reference, and
    /// stay inside sane bounds -- it multiplies the whole allocation, so a
    /// runaway value would hand one move the entire clock.
    #[test]
    fn ply_shape_rises_and_is_normalised() {
        let clock = 60_000;
        assert!((ply_shape(CURVE_REFERENCE_PLY, clock) - 1.0).abs() < 1e-9);

        let mut previous = 0.0;
        for ply in [0, 10, 20, 30, 40, 60, 80, 120, 200] {
            let shape = ply_shape(ply, clock);
            assert!(shape > previous, "ply {ply} did not rise: {shape} <= {previous}");
            previous = shape;
        }
        assert!(ply_shape(0, clock) > 0.5, "opening starved: {}", ply_shape(0, clock));
        assert!(ply_shape(200, clock) < 2.0, "late game runaway: {}", ply_shape(200, clock));
    }

    fn clock(ms: u64) -> Limits {
        Limits { wtime: Some(ms), btime: Some(ms), winc: 100, binc: 100, ..Limits::default() }
    }

    /// Pondering continues on the opponent's clock, so the soft bound rises. The
    /// hard bound is untouched: the bonus may not buy a time loss.
    #[test]
    fn ponder_raises_the_soft_bound_only() {
        let mut options = Options::default();
        let plain = clock(300_000).budget(Color::White, &options, 30, false).unwrap();
        options.ponder_enabled = true;
        let pondering = clock(300_000).budget(Color::White, &options, 30, false).unwrap();

        let ratio = pondering.optimum.as_millis() as f64 / plain.optimum.as_millis() as f64;
        assert!(
            (ratio - 1.25).abs() < 0.02,
            "expected a quarter more soft bound, got {ratio}"
        );
        assert_eq!(plain.maximum, pondering.maximum, "the hard bound must not move");
    }

    /// A forced move has nothing to decide, so the soft bound is capped -- but
    /// never below the floor that guarantees a completed iteration.
    #[test]
    fn a_forced_move_is_capped_but_still_searched() {
        let options = Options::default();
        let free = clock(300_000).budget(Color::White, &options, 30, false).unwrap();
        let forced = clock(300_000).budget(Color::White, &options, 30, true).unwrap();
        assert!(
            free.optimum.as_millis() > forced.optimum.as_millis(),
            "the cap did nothing: {:?} vs {:?}", free.optimum, forced.optimum
        );
        assert!(forced.optimum.as_millis() as u64 <= FORCED_MOVE_CEILING_MS);
        assert!(forced.optimum.as_millis() as u64 >= MIN_BUDGET_MS);

        // At a fast control the whole budget is already under the ceiling, so
        // the cap must be inert rather than a further cut.
        let fast_free = clock(10_000).budget(Color::White, &options, 30, false).unwrap();
        let fast_forced = clock(10_000).budget(Color::White, &options, 30, true).unwrap();
        assert_eq!(fast_free.optimum, fast_forced.optimum);
    }

    /// Stockfish makes the curve slightly steeper at longer time controls, and
    /// that falls out of `optConstant` depending on the clock. Keep it.
    #[test]
    fn curve_is_steeper_at_longer_time_controls() {
        let steep = |clock| ply_shape(80, clock) / ply_shape(0, clock);
        assert!(
            steep(300_000) > steep(10_000),
            "expected a steeper curve at 300s than at 10s: {} vs {}",
            steep(300_000),
            steep(10_000)
        );
    }
}

#[cfg(test)]
mod salvage_tests {
    use super::may_salvage;

    const PREV: u64 = 0xA;
    const NEW: u64 = 0xB;
    const OTHER: u64 = 0xC;
    const ALPHA: i32 = -40;

    #[test]
    fn keeps_a_finished_move_that_beat_the_previous_choice() {
        assert!(may_salvage(Some(PREV), &[PREV, NEW], NEW, 25, ALPHA));
        // Finish order does not matter, only that both finished.
        assert!(may_salvage(Some(PREV), &[OTHER, NEW, PREV], NEW, 25, ALPHA));
    }

    #[test]
    fn nothing_finished_keeps_the_previous_depth() {
        // With nothing finished the root hands back its own position.
        assert!(!may_salvage(Some(PREV), &[], NEW, 25, ALPHA));
        assert!(!may_salvage(None, &[], NEW, 25, ALPHA));
    }

    #[test]
    fn the_new_move_must_itself_have_finished() {
        assert!(!may_salvage(Some(PREV), &[PREV, OTHER], NEW, 25, ALPHA));
    }

    #[test]
    fn never_compared_with_the_previous_choice_keeps_it() {
        // The previous choice was not searched first and had not finished.
        assert!(!may_salvage(Some(PREV), &[NEW, OTHER], NEW, 25, ALPHA));
    }

    #[test]
    fn a_fail_low_is_only_a_bound_and_keeps_the_previous_depth() {
        assert!(!may_salvage(Some(PREV), &[PREV, NEW], NEW, ALPHA, ALPHA));
        assert!(!may_salvage(Some(PREV), &[PREV, NEW], NEW, ALPHA - 300, ALPHA));
    }

    #[test]
    fn the_same_move_changes_nothing() {
        assert!(!may_salvage(Some(PREV), &[PREV], PREV, 25, ALPHA));
    }

    #[test]
    fn with_no_previous_choice_any_finished_move_beats_the_fallback() {
        assert!(may_salvage(None, &[NEW], NEW, 25, i32::MIN + 1));
    }
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
