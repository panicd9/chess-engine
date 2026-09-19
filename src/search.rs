use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::{
    chessboard::{Chessboard, Color},
    eval::evaluate,
    move_gen::{black_legal_moves, legal_moves, white_legal_moves},
    move_list::{Move, MoveList},
    tt::TranspositionTable,
    zobrist,
};

/// Identifies a move by the squares the moving side vacated and filled. Small
/// enough to store, cheap enough to compute per move. See [`move_key`].
pub type MoveKey = u64;

/// What a stored score tells us about the true value of a position.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    /// The search returned a value inside its window: this is the real score.
    Exact,
    /// The search cut off: the true score is at least this.
    Lower,
    /// No move beat alpha: the true score is at most this.
    Upper,
}

/// Scores at least this extreme encode a mate rather than an evaluation.
///
/// Mate scores are relative to **ply** -- the distance from the root -- which is
/// an absolute property of where the mate is, so a score means the same thing
/// wherever it surfaces and needs no adjusting as it propagates. They are still
/// never stored in or returned from the table; only the move is kept.
///
/// They used to be relative to the *remaining depth* instead, which is only a
/// ply count while depth falls by exactly one per ply. It does not: a check
/// extension adds to it, and quiescence is entered at depth 0 and never
/// decrements. Mating lines are checking lines, so distances came out short --
/// a real mate in 4 reported as `mate 3`.
const MATE_SCORE_THRESHOLD: i32 = i32::MAX - 2 * 1000;

/// The score for being checkmated *at* the root, which every other mate score
/// is an offset from: being mated at ply `p` scores `MATED_AT_ROOT + p`, and
/// delivering mate there scores the negation. Later mates against us score
/// higher (prefer the slowest loss) and earlier mates by us score higher still
/// once negated (prefer the fastest win).
const MATED_AT_ROOT: i32 = i32::MIN + MATE_BOUND;

fn is_mate_score(score: i32) -> bool {
    score > MATE_SCORE_THRESHOLD || score < -MATE_SCORE_THRESHOLD
}

/// Half-moves without a capture or a pawn move after which the game is drawn.
pub const FIFTY_MOVE_PLIES: u32 = 100;

/// Deepest ply the killer table covers. Searches never get near this.
const MAX_PLY: usize = 64;

/// Zobrist keys of the positions on the path from the game's start to the node
/// being searched, plus the killer moves found at each ply.
///
/// Repetition is a property of the whole game, not of the current position, so
/// the search cannot detect it from the board alone -- it needs the positions
/// that came before. The UCI layer seeds this from `position ... moves` and the
/// search pushes and pops as it descends.
#[derive(Clone)]
pub struct History {
    keys: Vec<u64>,
    /// Two killer moves per ply. A killer is a quiet move that caused a beta
    /// cutoff somewhere else at the same depth: if it refuted one line it will
    /// often refute a sibling, so it is worth trying early. Captures are
    /// already ordered by MVV-LVA, so this is what orders the quiet moves.
    ///
    /// A move is identified by the squares it vacated and filled -- see
    /// [`move_key`] -- because `Move` carries a whole board rather than a
    /// from/to pair.
    killers: Box<[[u64; 2]; MAX_PLY]>,
    /// How often each quiet move has caused a cutoff anywhere in this search,
    /// indexed by the squares it touched. Killers only help at the ply they
    /// were found; this generalises across the whole tree, so a move that keeps
    /// working gets tried earlier everywhere.
    ///
    /// Indexed by from-square and to-square, recovered from the move key.
    history_scores: Box<[[i32; 64]; 64]>,
    /// What earlier searches -- including shallower iterative-deepening
    /// iterations -- concluded about positions seen along the way.
    pub table: TranspositionTable,
    /// Zobrist keys of the positions reached by root moves whose search ran to
    /// completion in the current iteration, in the order they finished. The
    /// driver clears it before each root search.
    ///
    /// A search the clock stops part-way is normally discarded whole. That is
    /// right for the moves it did not finish, but the root's *finished* moves
    /// carry genuine scores at this iteration's depth, and throwing them away
    /// throws away the most expensive work of the move. This is what lets the
    /// driver tell which of them it may trust.
    pub root_completed: Vec<u64>,
    /// Set while the score currently being computed depends on the moves played
    /// to reach it rather than on the position alone. See [`History::table`].
    path_dependent: bool,
    /// Name each root move over UCI as it is started, once a search is long
    /// enough to want it. Only the UCI driver sets this, so the harnesses and
    /// tests that call the search directly print nothing. See
    /// [`ROOT_MOVE_OUTPUT_NODES`].
    pub announce_root_moves: bool,
    /// The score the previous `go` settled on, from the side to move's point of
    /// view at that move. Time management compares the current score against it
    /// to notice a position going wrong -- see `uci::Pacing`. `None` at the
    /// start of a game and after `ucinewgame`.
    ///
    /// It lives here because `History` is what survives from one `go` to the
    /// next: the worker thread hands it back and `stop_search` takes it.
    pub previous_move_score: Option<i32>,
    /// The stability factor the previous `go` ended on, in percent. Carrying it
    /// forward is what turns time saved on a settled move into time spent on the
    /// next one -- Stockfish's `previousTimeReduction`. `None` at the start of a
    /// game and after `ucinewgame`.
    pub previous_time_reduction: Option<u64>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        History {
            keys: Vec::with_capacity(64),
            killers: Box::new([[0; 2]; MAX_PLY]),
            history_scores: Box::new([[0; 64]; 64]),
            // Left empty so cloning a game history stays cheap; the search
            // driver sizes it once before searching.
            table: TranspositionTable::new(0),
            root_completed: Vec::with_capacity(64),
            path_dependent: false,
            announce_root_moves: false,
            previous_move_score: None,
            previous_time_reduction: None,
        }
    }

    /// Give this search a transposition table, if it has not got one already.
    pub fn ensure_table(&mut self, megabytes: usize) {
        if !self.table.is_enabled() {
            self.table = TranspositionTable::new(megabytes);
        }
    }

    /// Throw the table away and build one of a different size. Only for a
    /// `setoption name Hash`, which a GUI sends before the game starts.
    pub fn resize_table(&mut self, megabytes: usize) {
        self.table = TranspositionTable::new(megabytes);
    }

    /// Replace the moves played so far, keeping everything the search has
    /// learned.
    ///
    /// `position` arrives before every `go`, and the only thing about it that
    /// changes between moves is the list of positions played. The table, the
    /// killers and the history scores belong to the session, and carrying them
    /// across moves is most of what makes the next search cheap: after the
    /// opponent replies, the position in front of us is one the last search
    /// already examined, and its whole subtree is still in the table.
    pub fn adopt_game(&mut self, played: History) {
        self.keys = played.keys;
    }

    /// Forget the game and everything learned from it, but keep the table's
    /// allocation -- sizing it again costs tens of milliseconds.
    pub fn new_game(&mut self) {
        self.keys.clear();
        for slot in self.killers.iter_mut() {
            *slot = [0; 2];
        }
        for row in self.history_scores.iter_mut() {
            row.fill(0);
        }
        self.table.clear();
        self.path_dependent = false;
        self.previous_move_score = None;
        self.previous_time_reduction = None;
    }

    /// Remember a quiet move that caused a cutoff at this ply.
    fn store_killer(&mut self, ply: usize, key: u64) {
        if ply >= MAX_PLY || key == 0 {
            return;
        }
        let slot = &mut self.killers[ply];
        if slot[0] != key {
            slot[1] = slot[0]; // Keep the previous one as the second choice.
            slot[0] = key;
        }
    }

    fn is_killer(&self, ply: usize, key: u64) -> bool {
        ply < MAX_PLY && (self.killers[ply][0] == key || self.killers[ply][1] == key)
    }

    /// Credit a quiet move that caused a cutoff. Deeper cutoffs count for more,
    /// since they were harder to find and prove more.
    fn credit_history(&mut self, key: u64, depth: u32) {
        let Some((from, to)) = squares_of(key) else { return };
        let score = &mut self.history_scores[from][to];
        *score += (depth * depth) as i32;
        // Keep the table from saturating: once any entry gets large, halve them
        // all so recent cutoffs still move the ordering.
        if *score > 1 << 20 {
            for row in self.history_scores.iter_mut() {
                for entry in row.iter_mut() {
                    *entry /= 2;
                }
            }
        }
    }

    fn history_score(&self, key: u64) -> i32 {
        match squares_of(key) {
            Some((from, to)) => self.history_scores[from][to],
            None => 0,
        }
    }

    pub fn push(&mut self, cb: &Chessboard) {
        self.keys.push(zobrist::hash(cb));
    }

    pub fn pop(&mut self) {
        self.keys.pop();
    }

    /// Has this position occurred before?
    ///
    /// One earlier occurrence is enough to return a draw score. Requiring a
    /// true threefold would let the search walk into a repetition believing it
    /// was still winning, and every repetition inside the search is one the
    /// side to move can choose to reach.
    ///
    /// Only positions since the last irreversible move can repeat, so the scan
    /// is bounded by the halfmove clock rather than the length of the game.
    pub fn repeats(&self, cb: &Chessboard) -> bool {
        let reversible = cb.halfmove_clock as usize;
        if reversible < 4 {
            return false; // Too recent for a position to have come back.
        }
        let key = zobrist::hash(cb);
        let window = reversible.min(self.keys.len());
        // A position can only recur with the same side to move. The most recent
        // entry is the position just before this one, so it has the *opposite*
        // side to move: skip it, then take every second entry going back.
        self.keys[self.keys.len() - window..]
            .iter()
            .rev()
            .skip(1)
            .step_by(2)
            .any(|&seen| seen == key)
    }
}

/// Set to ask an in-flight search to give up as soon as it can. The scores it
/// returns after this point are meaningless, so whoever sets it must discard the
/// current iteration and fall back on the last completed one.
pub static STOP: AtomicBool = AtomicBool::new(false);

/// Nodes visited since [`reset_nodes`]. Reported as `nodes`/`nps` over UCI.
static NODES: AtomicU64 = AtomicU64::new(0);

pub fn reset_nodes() {
    NODES.store(0, Ordering::Relaxed);
}

pub fn nodes_searched() -> u64 {
    NODES.load(Ordering::Relaxed)
}

/// Deepest ply any node has reached since [`reset_seldepth`], counting the root
/// as 1. Reported as `seldepth` over UCI.
///
/// Every node counts, quiescence included, so this is how far the longest line
/// actually ran. Stockfish counts only PV nodes, so its figure is not the same
/// measurement and the two should not be compared.
static SELDEPTH: AtomicU32 = AtomicU32::new(0);

pub fn reset_seldepth() {
    SELDEPTH.store(0, Ordering::Relaxed);
}

pub fn seldepth() -> u32 {
    SELDEPTH.load(Ordering::Relaxed)
}

/// Record that a node was searched at this ply. A load and a compare: the store
/// only happens when a line goes deeper than any before it.
#[inline]
fn note_ply(ply: usize) {
    let reached = ply as u32 + 1;
    if reached > SELDEPTH.load(Ordering::Relaxed) {
        SELDEPTH.store(reached, Ordering::Relaxed);
    }
}

/// Nodes a search must have visited before the root names each move it starts.
/// Stockfish's `NODES_LIMIT_OUTPUT`: below it a search is over too quickly for
/// the lines to be read, and they would only flood the GUI.
pub const ROOT_MOVE_OUTPUT_NODES: u64 = 10_000_000;

/// `info depth <d> currmove <move> currmovenumber <n>`, as Stockfish sends it.
#[cold]
fn announce_root_move(root: &Chessboard, child: &Chessboard, depth: u32, number: usize) {
    if let Some(mv) = crate::notation::describe_move(root, child) {
        println!("info depth {depth} currmove {mv} currmovenumber {number}");
        let _ = std::io::stdout().flush();
    }
}

/// Count this node and report whether the search has been asked to stop.
///
/// Every node reads the flag. Sampling it (say, every 2048th node) would only
/// gate the *check*, not the work: the other nodes would carry on generating
/// moves and recursing, so an abort would trickle through the tree instead of
/// unwinding it, and a time limit would overshoot by ~100ms. A relaxed load is
/// a few tenths of a nanosecond against a node cost in the hundreds.
///
/// The counter is a read-modify-write on a shared line. If the search ever runs
/// on more than one thread, that wants to become a per-thread count published
/// periodically -- but the flag read should stay on every node.
#[inline]
fn count_node_and_should_stop() -> bool {
    let visited = NODES.fetch_add(1, Ordering::Relaxed) + 1;
    // Tests stop the search at an exact node, from inside the search thread, so
    // what happens after a stop is reproducible and free of races. Compiled out
    // of every non-test build, so the hot path is unchanged.
    #[cfg(test)]
    {
        let limit = test_hooks::NODE_LIMIT.load(Ordering::Relaxed);
        if limit != 0 && visited >= limit {
            STOP.store(true, Ordering::Relaxed);
        }
    }
    #[cfg(not(test))]
    let _ = visited;
    STOP.load(Ordering::Relaxed)
}

#[cfg(test)]
mod test_hooks {
    use std::sync::atomic::AtomicU64;
    /// Stop the search once this many nodes have been visited. 0 is no limit.
    pub static NODE_LIMIT: AtomicU64 = AtomicU64::new(0);
    /// Table stores made while the stop flag was set.
    pub static STORES_WHILE_STOPPED: AtomicU64 = AtomicU64::new(0);
}

/// PROTOTYPE: reuse move buffers instead of allocating one per node.
mod pool {
    use crate::move_list::Move;
    use std::cell::RefCell;
    thread_local! {
        static POOL: RefCell<Vec<Vec<Move>>> = const { RefCell::new(Vec::new()) };
    }
    pub fn take() -> Vec<Move> {
        POOL.with(|p| p.borrow_mut().pop()).unwrap_or_else(|| Vec::with_capacity(64))
    }
    pub fn give(mut v: Vec<Move>) {
        v.clear();
        POOL.with(|p| p.borrow_mut().push(v));
    }
}

pub fn nega_max_alpha_beta_best_move(
    cb: &Chessboard,
    depth: u32,
    is_white_turn: bool,
    mut alpha: i32,
    beta: i32,
    history: &mut History,
) -> (i32, Chessboard) {
    search_node(cb, depth, 0, is_white_turn, alpha, beta, history)
}

/// `ply` is the distance from the root, which is what the killer table is
/// indexed by: a move that refutes something at ply 4 is only relevant to other
/// nodes at ply 4.
fn search_node(
    cb: &Chessboard,
    depth: u32,
    ply: usize,
    is_white_turn: bool,
    mut alpha: i32,
    beta: i32,
    history: &mut History,
) -> (i32, Chessboard) {
    // Quiescence counts its own nodes, so leave the leaf to it rather than
    // counting this position twice.
    if depth == 0 {
        return quiescence_search_best_move(cb, is_white_turn, ply, alpha, beta);
    }

    if count_node_and_should_stop() {
        // Unwind immediately. The caller discards this iteration's result.
        return (0, *cb);
    }
    note_ply(ply);

    // What do we already know about this position? A usable score ends the
    // node outright; otherwise the stored move still tells us what to try first.
    let key = zobrist::hash(cb);
    let mut tt_move: MoveKey = 0;
    if let Some(hit) = history.table.probe(key, depth, alpha, beta) {
        tt_move = hit.best_move;
        // Never cut off at the root. This path returns the position unchanged
        // because it has no move to report, which the caller at ply 0 needs --
        // it would answer `bestmove` with whatever `legal_moves` happened to
        // yield first. Latent while the table lasted only one search, because
        // iterative deepening always probes the root deeper than it stored it;
        // a table that outlives the search has a deep root entry waiting.
        if ply > 0 {
            if let Some(score) = hit.score {
                if !is_mate_score(score) {
                    return (score, *cb);
                }
            }
        }
    }

    // Null-move pruning. Hand the opponent a free move: if our position is so
    // strong that they still cannot pull it below beta, then a real move will
    // be at least as good and this whole node can be cut.
    //
    // Skipped at the root, because there we must return an actual move; when in
    // check, because passing would be illegal; and with only pawns left, where
    // the "having the move helps" assumption fails.
    if toggles::on(&toggles::NULL_MOVE)
        && ply > 0
        && depth > NULL_MOVE_REDUCTION
        && beta < MATE_SCORE_THRESHOLD
        && has_pieces(cb, is_white_turn)
        && !in_check(cb, is_white_turn)
    {
        let mut passed = *cb;
        passed.change_side_to_move();
        passed.en_passant = 0; // The chance to capture en passant does not survive a pass.
        let score = -search_node(
            &passed,
            depth - 1 - NULL_MOVE_REDUCTION,
            ply + 1,
            !is_white_turn,
            -beta,
            -beta + 1, // Null window: we only care whether it beats beta.
            history,
        )
        .0;
        if score >= beta && !is_mate_score(score) {
            return (score, *cb);
        }
    }

    // We'll track the best score and best resulting position.
    let mut best_score = i32::MIN;
    let mut best_pos: Chessboard = *cb;

    // Generate in two stages. The noisy moves -- captures, en passant and
    // promotions -- are built first, because they are where cutoffs come from:
    // measured over a search, every cutoff on kiwipete, 96% on position 6, 72%
    // in a Ruy Lopez. When one of them cuts, the quiet moves are never built at
    // all, and four boards in five built at an interior node were never
    // searched.
    let parent_occupancy = if is_white_turn {
        cb.get_white_occupancy()
    } else {
        cb.get_black_occupancy()
    };
    let legality = if is_white_turn {
        crate::move_gen::legality::white_legality(cb)
    } else {
        crate::move_gen::legality::black_legality(cb)
    };

    let mut buf = pool::take();
    if is_white_turn {
        crate::move_gen::white_captures_with(cb, &mut buf, &legality);
    } else {
        crate::move_gen::black_captures_with(cb, &mut buf, &legality);
    }
    let mut legal_moves = MoveList::new(buf);
    let tt_move_is_noisy =
        order_stage(&mut legal_moves.moves, parent_occupancy, is_white_turn, tt_move, ply, history);

    // Two reasons not to defer the quiet moves. There may be no noisy ones, in
    // which case there is nothing to defer them behind and this node may be
    // terminal. Or the table's move is a quiet one: it has to be searched
    // first, or the shallower iteration that found it has bought nothing.
    let mut quiets_generated = false;
    if legal_moves.moves.is_empty() || (tt_move != 0 && !tt_move_is_noisy) {
        let first_quiet = legal_moves.moves.len();
        if is_white_turn {
            crate::move_gen::white_quiets_with(cb, &mut legal_moves.moves, &legality);
        } else {
            crate::move_gen::black_quiets_with(cb, &mut legal_moves.moves, &legality);
        }
        order_stage(
            &mut legal_moves.moves[first_quiet..],
            parent_occupancy,
            is_white_turn,
            tt_move,
            ply,
            history,
        );
        quiets_generated = true;
    }

    if legal_moves.moves.is_empty() {
        pool::give(legal_moves.into_inner());
        return (terminal_score(cb, is_white_turn, ply as u32), *cb);
    }


    let original_alpha = alpha;
    let mut best_move: MoveKey = 0;
    let mut cutoff = false;
    // Save and reset so the flag reports only what happened below this node.
    let outer_path_dependent = history.path_dependent;
    history.path_dependent = false;

    // The masks counted the checkers already; asking again would repeat a
    // whole attack test on the same board.
    let in_check_here = legality.in_check();
    let mut moves_searched = 0usize;

    // In check, search a ply deeper. Forcing sequences have few legal replies so
    // the extra ply is cheap, and stopping in the middle of one is how an engine
    // walks into a mate it was one move from seeing.
    let depth = if in_check_here && ply > 0 && toggles::on(&toggles::CHECK_EXTENSIONS) {
        depth + CHECK_EXTENSION
    } else {
        depth
    };

    // Futility pruning: close to the leaves and already far below alpha, a quiet
    // move is unlikely to recover. The margin grows with the remaining depth.
    // Never applied while in check, where any move may be forced.
    let futile = toggles::on(&toggles::FUTILITY)
        && !in_check_here
        && depth <= FUTILITY_MAX_DEPTH
        && beta < MATE_SCORE_THRESHOLD
        && (if is_white_turn { evaluate(cb) } else { -evaluate(cb) })
            + FUTILITY_MARGIN_PER_PLY * depth as i32
            <= alpha;

    // Iterate over moves, a stage at a time.
    'stages: loop {
    while let Some(next_move) = legal_moves.next_move() {
        // Extract the new position from the move.
        let pos = next_move.chessboard;
        if ply == 0
            && history.announce_root_moves
            && NODES.load(Ordering::Relaxed) > ROOT_MOVE_OUTPUT_NODES
        {
            announce_root_move(cb, &pos, depth, moves_searched + 1);
        }
        // Negamax: invert alpha and beta for the recursive call.
        let safe_beta = safe_neg(beta);
        let safe_alpha = safe_neg(alpha);

        // Score the position this move leads to. A move into a repetition or
        // past the fifty-move limit is a draw however good the position looks,
        // and both are cheaper to detect than to search. The mate test lives in
        // the child, so mating on the hundredth half-move still wins.
        history.push(cb);
        let score = if history.repeats(&pos) || pos.halfmove_clock >= FIFTY_MOVE_PLIES {
            // Whether this is a draw depends on the moves played to get here,
            // not on the position, so nothing on this path may be cached.
            history.path_dependent = true;
            DRAW
        } else {
            // Late move reductions. Once the promising moves have been tried,
            // search what is left a ply shallower with a null window -- just
            // enough to ask "could this beat what we already have?". Usually it
            // cannot and the saving stands. When it might, the reduced result
            // is untrustworthy, so the move is searched again properly.
            //
            // Only quiet moves late in the list are reduced: the table move,
            // captures and killers are ordered first precisely because they are
            // likely best, and positions in check are too sharp to skim.
            let quiet = next_move.score < FIRST_CAPTURE_SCORE;
            let gives_check = in_check(&pos, !is_white_turn);

            // Skip quiet moves that cannot realistically reach alpha. One move
            // is always searched, so the node still returns something.
            if futile && quiet && !gives_check && moves_searched > 0 {
                history.pop();
                moves_searched += 1;
                continue;
            }

            let reduce = toggles::on(&toggles::LMR)
                && depth >= LMR_MIN_DEPTH
                && moves_searched >= LMR_FIRST_REDUCED_MOVE
                && quiet
                && !in_check_here
                && !gives_check;

            let mut score = if reduce {
                -search_node(
                    &pos,
                    depth - 2,
                    ply + 1,
                    !is_white_turn,
                    -(alpha + 1),
                    -alpha,
                    history,
                )
                .0
            } else {
                i32::MIN + 1 // Sentinel: forces the full search below.
            };

            if !reduce || score > alpha {
                score = -search_node(
                    &pos,
                    depth - 1,
                    ply + 1,
                    !is_white_turn,
                    safe_beta,
                    safe_alpha,
                    history,
                )
                .0;
            }
            score
        };
        history.pop();
        moves_searched += 1;

        // Stopped while this child was being searched: its score is the
        // placeholder from the stop check, not a result. Leave before it can
        // reach the best score, alpha, the killers or the history table. Every
        // child searched before the flag went up is still genuine, so what this
        // node has at this point is the best of the moves it actually finished.
        if STOP.load(Ordering::Relaxed) {
            break 'stages;
        }
        if ply == 0 {
            history.root_completed.push(zobrist::hash(&pos));
        }

        // If this move is better, update the best score and best position.
        if score > best_score {
            best_score = score;
            best_pos = pos;
            best_move = move_key(parent_occupancy, &pos, is_white_turn);
        }

        // Update alpha and do a beta cutoff if possible.
        alpha = alpha.max(score);
        if alpha >= beta {
            // This move refuted the line. Remember it as a killer so sibling
            // nodes at the same ply try it early. Captures are excluded: they
            // are already ordered by MVV-LVA, and a killer slot spent on one is
            // a slot not spent on the quiet move that needed the help.
            if next_move.score < KILLER_BONUS {
                let key = move_key(parent_occupancy, &pos, is_white_turn);
                history.store_killer(ply, key);
                history.credit_history(key, depth);
            }
            cutoff = true;
            break 'stages; // Beta cutoff.
        }
    }

    if quiets_generated {
        break 'stages;
    }
    // The noisy moves are spent and none of them cut. Build the quiet ones and
    // carry on through the same list: `next_move` selects over whatever has not
    // been searched yet, so appending to the tail is all it takes.
    let first_quiet = legal_moves.moves.len();
    if is_white_turn {
        crate::move_gen::white_quiets_with(cb, &mut legal_moves.moves, &legality);
    } else {
        crate::move_gen::black_quiets_with(cb, &mut legal_moves.moves, &legality);
    }
    order_stage(
        &mut legal_moves.moves[first_quiet..],
        parent_occupancy,
        is_white_turn,
        tt_move,
        ply,
        history,
    );
    quiets_generated = true;
    }

    // A stopped node concluded nothing that may outlive this search. Its best
    // score covers only the moves it finished, so as a table entry it would
    // claim a bound over moves it never looked at -- and the table lasts the
    // whole game, so a later search would read that claim back as a result.
    //
    // Before this, every node on the path being searched when the flag went up
    // stored `max(real children, 0)` at its full depth, 0 being the placeholder
    // every later child returned. See `abort_tests`.
    if STOP.load(Ordering::Relaxed) {
        history.path_dependent |= outer_path_dependent;
        pool::give(legal_moves.into_inner());
        // No move finished: report the placeholder rather than i32::MIN, which
        // the caller negates.
        let score = if best_score == i32::MIN { 0 } else { best_score };
        return (score, best_pos);
    }

    // Record what this node concluded. `bound` says how much to trust it: a
    // cutoff only proves the score is at least this, and a node where nothing
    // beat alpha only proves it is at most this.
    //
    // A score that came from a repetition or the fifty-move rule is a property
    // of this path, not of the position, so it is not cached -- another route
    // to the same position may not be a draw at all.
    if !is_mate_score(best_score) {
        let bound = if cutoff {
            Bound::Lower
        } else if best_score <= original_alpha {
            Bound::Upper
        } else {
            Bound::Exact
        };
        // A score that came from a repetition or the fifty-move rule belongs to
        // this path, not this position, so it must not be reused. Storing it at
        // depth 0 keeps the move -- which is still the best one found here, and
        // is worth having for ordering and for the principal variation -- while
        // ensuring no search deeper than 0 will ever trust the score.
        let storable_depth = if history.path_dependent { 0 } else { depth };
        // No move from a node where nothing beat alpha. Every move there was
        // refuted, so the "best" of them is only the least-refuted -- and the
        // table would hand it on as the move to try first, and as the reply the
        // principal variation expects. Replayed over eight of the bot's games
        // with real clocks and pondering, every ponder token that flattered us
        // by two pawns or more against the reply the engine names when asked
        // directly (22 of 532 turns) had been written by exactly such a node.
        // Stockfish stores no move here either and keeps whatever was there.
        let stored_move = if bound == Bound::Upper { 0 } else { best_move };
        #[cfg(test)]
        if STOP.load(Ordering::Relaxed) {
            test_hooks::STORES_WHILE_STOPPED.fetch_add(1, Ordering::Relaxed);
        }
        history.table.store(key, storable_depth, best_score, bound, stored_move);
    }

    // Propagate upwards: our caller's score depends on ours.
    history.path_dependent |= outer_path_dependent;
    pool::give(legal_moves.into_inner());

    (best_score, best_pos)
}

pub fn nega_max_alpha_beta_best_line(
    cb: &Chessboard,
    depth: u32,
    ply: u32,
    is_white_turn: bool,
    mut alpha: i32,
    beta: i32,
) -> Vec<(i32, Vec<Chessboard>)> {
    // Base case: Evaluate and return a single-line variation.
    if depth == 0 {
        return quiescence_search_best_line(cb, is_white_turn, alpha, beta);
    }

    // Generate legal moves for the current side.
    let mut legal_moves = if is_white_turn {
        MoveList::new(white_legal_moves(cb))
    } else {
        MoveList::new(black_legal_moves(cb))
    };

    // We'll store the best candidate variation found so far.
    let mut best_line: Option<(i32, Vec<Chessboard>)> = None;

    'legal_moves: while let Some(next_move) = legal_moves.next_move() {
        let pos = next_move.chessboard;
        // Recursively get the best line from the child node.
        let safe_beta = safe_neg(beta);
        let safe_alpha = safe_neg(alpha);
        let child_results = nega_max_alpha_beta_best_line(
            &pos,
            depth - 1,
            ply + 1,
            !is_white_turn,
            safe_beta,
            safe_alpha,
        );
        if child_results.is_empty() {
            let score = terminal_score(cb, is_white_turn, ply);
            return vec![(score, vec![cb.clone(), pos])];
        }

        // Assume that the best candidate from the child branch is the first one.
        let (child_score, child_line) = child_results[0].clone();
        let score = if child_score == i32::MIN {
            i32::MAX // Representing the worst possible score for the opponent
        } else {
            -child_score
        }; // Negamax: invert the child's score.
        let mut line = vec![cb.clone()];
        line.extend(child_line);

        // Update best_line if this candidate is better.
        if best_line.is_none() || score > best_line.as_ref().unwrap().0 {
            best_line = Some((score, line));
        }

        // Update alpha for pruning based on the best candidate from this branch.
        alpha = alpha.max(score);
        if alpha >= beta {
            break 'legal_moves; // Beta cutoff.
        }
    }

    // Return the best line if one was found; otherwise, return an empty vector.
    best_line.map(|v| vec![v]).unwrap_or_else(Vec::new)
}

pub fn quiescence_search_best_move(
    cb: &Chessboard,
    is_white_turn: bool,
    ply: usize,
    mut alpha: i32,
    beta: i32,
) -> (i32, Chessboard) {
    if count_node_and_should_stop() {
        return (0, *cb);
    }
    note_ply(ply);

    // Do a static evaluation of the current (quiet) position.
    // For black, invert the evaluation to maintain the negamax framework.
    let stand_pat = if is_white_turn { evaluate(cb) } else { -evaluate(cb) };
    let mut best_score = stand_pat;
    // Default board is the current board (used if no move improves the evaluation)
    let mut best_board = cb.clone();

    // Fail-soft: return what was actually found, not the window edge. A clamped
    // `beta` here is not a real score, and a node above that stores a value
    // derived from one would cache something only valid for this window --
    // which is exactly what breaks the transposition table.
    if best_score >= beta {
        return (best_score, best_board);
    }
    if alpha < best_score {
        alpha = best_score;
    }

    let mut all_moves = pool::take();
    // One set of masks for the whole node: they say whether this position is in
    // check, they are what the generator needs, and `has_any_legal_move` wants
    // the same `danger` map again below. Asking three times over meant three
    // attack tests on one board.
    let legality = if is_white_turn {
        crate::move_gen::legality::white_legality(cb)
    } else {
        crate::move_gen::legality::black_legality(cb)
    };
    let checked = legality.in_check();
    match (checked, is_white_turn) {
        (true, true) => crate::move_gen::white_legal_moves_with(cb, &mut all_moves, &legality),
        (true, false) => crate::move_gen::black_legal_moves_with(cb, &mut all_moves, &legality),
        (false, true) => crate::move_gen::white_captures_with(cb, &mut all_moves, &legality),
        (false, false) => crate::move_gen::black_captures_with(cb, &mut all_moves, &legality),
    }

    if all_moves.is_empty() {
        pool::give(all_moves);
        // In check everything was generated, so an empty list is mate.
        // Otherwise it only means there was nothing to capture -- which is what
        // a quiet position looks like, and also what stalemate looks like. The
        // two score differently (stalemate is a draw however the evaluation
        // reads), so they have to be told apart.
        if checked || !crate::move_gen::has_any_legal_move_with(cb, is_white_turn, &legality) {
            return (terminal_score(cb, is_white_turn, ply as u32), best_board);
        }
        return (best_score, best_board);
    }

    // Generate only "noisy" moves (e.g., captures).
    // Keep the captures, then drop the ones that plainly lose material.
    // Quiescence exists to resolve exchanges, not to explore a queen taking a
    // defended pawn -- static exchange evaluation settles those without a
    // search. Captures that come out level or better are kept; a losing one can
    // only be right as a sacrifice, which is beyond what quiescence looks for.
    let parent_occupancy = if is_white_turn {
        cb.get_white_occupancy()
    } else {
        cb.get_black_occupancy()
    };
    // Delta pruning: if even winning a queen from here would leave the score
    // well short of alpha, this position is lost regardless of what is captured
    // and searching the captures cannot change that. Switched off in check,
    // where the replies may be forced, and near mate scores, where material is
    // not what decides the position.
    if toggles::on(&toggles::DELTA)
        && !checked
        && alpha < MATE_SCORE_THRESHOLD
        && stand_pat + QUEEN_VALUE + DELTA_MARGIN < alpha
    {
        pool::give(all_moves);
        return (best_score, best_board);
    }

    all_moves.retain(|m| {
        m.score >= 6 && !loses_material(cb, parent_occupancy, &m.chessboard, is_white_turn)
    });

    if all_moves.is_empty() {
        pool::give(all_moves);
        return (best_score, best_board);
    }

    let mut not_quiet_moves = MoveList::new(all_moves);

    // Loop through each capture move.
    while let Some(next_move) = not_quiet_moves.next_move() {
        let pos = next_move.chessboard.clone();
        // Swap bounds for the negamax recursion.
        let safe_beta = safe_neg(beta);
        let safe_alpha = safe_neg(alpha);
        // Recurse: the returned board here is from the child's perspective.
        // Since we want the immediate move (next_move.chessboard) at this level,
        // we ignore the child's board state.
        // Quiescence is entered at depth 0 and is bounded by captures running
        // out, not by this counter, so it must saturate rather than wrap.
        let child_result =
            quiescence_search_best_move(&pos, !is_white_turn, ply + 1, safe_beta, safe_alpha);

        // Invert the child's score (negamax style).
        let score = -child_result.0;


        // Beta cutoff: return immediately with the move that produced this
        // cutoff. Fail-soft, for the reason given above.
        if score >= beta {
            pool::give(not_quiet_moves.into_inner());
            return (score, next_move.chessboard);
        }

        // If we find a move that improves alpha, update.
        if score > alpha {
            alpha = score;
            best_score = score;
            best_board = next_move.chessboard;
        }
    }

    pool::give(not_quiet_moves.into_inner());
    (best_score, best_board)
}



pub fn quiescence_search_best_line(
    cb: &Chessboard,
    is_white_turn: bool,
    mut alpha: i32,
    beta: i32,
) -> Vec<(i32, Vec<Chessboard>)> {
    // First, do a static evaluation of the current (quiet) position.
    let stand_pat = if is_white_turn { evaluate(cb) } else { -evaluate(cb) };


    // In a negamax framework the evaluation is from the perspective of the side to move.
    // (Assuming evaluate() returns a score from white's perspective, then if it is black's turn,
    // you might need to invert the value. Adjust if necessary.)
    let mut best_score = stand_pat;
    let mut best_line = vec![cb.clone()];

    // Check cutoff: if the stand_pat is already good enough, return immediately.
    if best_score >= beta {
        return vec![(beta, best_line)]; // Fail-hard beta cutoff.
    }
    if alpha < best_score {
        alpha = best_score;
    }

    // Generate only "noisy" moves (e.g. captures).
    let capture_moves: Vec<_> = if is_white_turn {
        // Filter white moves for captures.
        white_legal_moves(cb)
            .into_iter()
            .filter(|m| m.score >= 6)
            .collect()
    } else {
        black_legal_moves(cb)
            .into_iter()
            .filter(|m| m.score >= 6)
            .collect()
    };

    let mut not_quiet_moves = MoveList::new(capture_moves);

    // Loop through each capture move.
    while let Some(next_move) = not_quiet_moves.next_move() {
        let pos = next_move.chessboard;

        // Negamax: call quiescence search recursively with swapped bounds.
        let safe_beta = safe_neg(beta);
        let safe_alpha = safe_neg(alpha);
        let child_results = quiescence_search_best_line(&pos, !is_white_turn, safe_beta, safe_alpha);

        if child_results.is_empty() {
            // If no moves are returned, treat it as a terminal position.
            continue;
        }

        let (child_score, child_line) = child_results[0].clone();
        let score = if child_score == i32::MIN {
            i32::MAX // handle the overflow case
        } else {
            -child_score
        };

        // If the move improves our alpha, update.
        if score >= beta {
            let mut line = vec![cb.clone()];
            line.extend(child_line);
            return vec![(beta, best_line)]; // Beta cutoff.
        }

        if score > alpha {
            alpha = score;
            best_score = score;
            let mut line = vec![cb.clone()];
            line.extend(child_line);
            best_line = line;
        }
    }

    vec![(best_score, best_line)]
}


/// Score for a node where the side to move has no legal reply.
///
/// Checkmate is a loss for the side to move; stalemate is a draw and must score
/// 0, not a loss, or the engine happily stalemates a won position. The score is
/// relative to the side to move, so it does not depend on colour. Mates found
/// nearer the root (smaller `ply`) score worse, so the search prefers the
/// slowest loss and the fastest win.
pub fn terminal_score(cb: &Chessboard, is_white_turn: bool, ply: u32) -> i32 {
    let in_check = if is_white_turn {
        cb.is_white_king_under_attack()
    } else {
        cb.is_black_king_under_attack()
    };

    if !in_check {
        return DRAW; // Stalemate.
    }

    MATED_AT_ROOT + ply as i32
}

/// Identifies a move by the squares the moving side vacated and filled.
///
/// Move generation hands back positions rather than moves, so this is the
/// cheapest available handle on "which move was that": six ORs for the child's
/// occupancy against the parent's, which the caller computes once per node.
#[inline]
/// Apply the ordering bonuses to one stage of the move list.
///
/// Called once per stage rather than once per node, so it takes a slice: the
/// quiet stage is appended to the tail of a list whose head has already been
/// searched, and only the tail wants scoring.
///
/// Returns whether the table's move was in this stage, which is how the caller
/// learns that a stored move is quiet and has to be built now rather than
/// after the captures.
fn order_stage(
    moves: &mut [Move],
    parent_occupancy: u64,
    is_white_turn: bool,
    tt_move: MoveKey,
    ply: usize,
    history: &History,
) -> bool {
    let mut found_tt_move = false;
    for m in moves {
        let key = move_key(parent_occupancy, &m.chessboard, is_white_turn);
        if tt_move != 0 && key == tt_move {
            // Whatever was best here last time goes first, ahead of every
            // capture. This is what makes the shallower iterations pay off.
            m.score = TT_MOVE_BONUS;
            found_tt_move = true;
        } else if m.score < KILLER_BONUS && ply < MAX_PLY && history.is_killer(ply, key) {
            // Promote this ply's killers so the lazy selection in `next_move`
            // picks them ahead of the other quiet moves. Only quiet moves are
            // promoted: a capture already outranks the bonus.
            m.score = KILLER_BONUS;
        } else if m.score < KILLER_BONUS && toggles::on(&toggles::HISTORY) {
            // Remaining quiet moves are ordered by how often they have caused a
            // cutoff elsewhere in this search. Capped below KILLER_BONUS so the
            // ordering above it is never disturbed.
            let h = history.history_score(key);
            if h > 0 {
                m.score = 1 + (h.min(1 << 16) >> 13) as u32;
            }
        }
    }
    found_tt_move
}

fn move_key(parent_occupancy: u64, child: &Chessboard, is_white_turn: bool) -> u64 {
    let child_occupancy = if is_white_turn {
        child.get_white_occupancy()
    } else {
        child.get_black_occupancy()
    };
    parent_occupancy ^ child_occupancy
}

/// The two squares a move key encodes, or `None` if it is not a simple move.
///
/// A key is the mover's occupancy before XOR after, so a normal move sets
/// exactly two bits: the square left and the square arrived on. Castling sets
/// four and is not tracked.
fn squares_of(key: u64) -> Option<(usize, usize)> {
    if key.count_ones() != 2 {
        return None;
    }
    let first = key.trailing_zeros() as usize;
    let second = (key & (key - 1)).trailing_zeros() as usize;
    Some((first, second))
}

/// Ordering bonus for a killer move: above every quiet move, below every
/// capture, so MVV-LVA still leads.
const KILLER_BONUS: u32 = 9;

/// Ordering bonus for the table's move, which outranks everything: it is the
/// best move a previous, usually deeper, search found here.
const TT_MOVE_BONUS: u32 = 1000;

/// Ordering scores at or above this mark a capture -- MVV-LVA starts at 10.
/// Below it are quiet moves, which are what late move reductions apply to.
const FIRST_CAPTURE_SCORE: u32 = 10;

/// Moves this far down the list are searched shallower first. Ordering is good
/// enough (table move, captures, killers) that anything this late rarely wins.
const LMR_FIRST_REDUCED_MOVE: usize = 4;

/// Below this depth there is nothing worth saving by reducing.
const LMR_MIN_DEPTH: u32 = 3;

/// Switches for the individually-unproven search techniques, so each can be
/// disabled at runtime and measured on its own. They were added and measured as
/// one group (+35 +/- 40 Elo), which cannot tell whether any single one is
/// actually harmful.
///
/// All default to on; the UCI options exist for A/B testing, not for play.
pub mod toggles {
    use std::sync::atomic::{AtomicBool, Ordering};

    pub static CHECK_EXTENSIONS: AtomicBool = AtomicBool::new(true);
    pub static HISTORY: AtomicBool = AtomicBool::new(true);
    pub static FUTILITY: AtomicBool = AtomicBool::new(true);
    pub static DELTA: AtomicBool = AtomicBool::new(true);
    pub static LMR: AtomicBool = AtomicBool::new(true);
    pub static NULL_MOVE: AtomicBool = AtomicBool::new(true);

    /// Set one by name. Returns whether the name was recognised.
    pub fn set(name: &str, on: bool) -> bool {
        let target = match name.to_ascii_lowercase().as_str() {
            "checkextensions" => &CHECK_EXTENSIONS,
            "history" => &HISTORY,
            "futility" => &FUTILITY,
            "delta" => &DELTA,
            "lmr" => &LMR,
            "nullmove" => &NULL_MOVE,
            _ => return false,
        };
        target.store(on, Ordering::Relaxed);
        true
    }

    #[inline]
    pub fn on(flag: &AtomicBool) -> bool {
        flag.load(Ordering::Relaxed)
    }
}

/// How much a position that is in check is worth searching beyond the nominal
/// depth. Forcing sequences are cheap -- few legal replies -- and stopping in
/// the middle of one is how an engine walks into a mate it could have seen.
const CHECK_EXTENSION: u32 = 1;

/// Futility pruning: near the leaves, a quiet move in a position already this
/// far below alpha is unlikely to claw its way back, so it is skipped. The
/// margin grows with the depth still to search.
const FUTILITY_MARGIN_PER_PLY: i32 = 120;
const FUTILITY_MAX_DEPTH: u32 = 3;

/// Delta pruning: in quiescence, a capture that cannot bring the score near
/// alpha even after winning the piece is not worth searching.
const DELTA_MARGIN: i32 = 200;
/// Value of the most valuable piece that can be captured, for delta pruning.
const QUEEN_VALUE: i32 = crate::piece_square_tables::MG_VALUE[4];

/// How much shallower the null-move verification search runs. Two plies is the
/// usual choice: deep enough to be meaningful, shallow enough to be cheap.
const NULL_MOVE_REDUCTION: u32 = 2;

/// Is the side to move in check?
fn in_check(cb: &Chessboard, is_white_turn: bool) -> bool {
    if is_white_turn {
        cb.is_white_king_under_attack()
    } else {
        cb.is_black_king_under_attack()
    }
}

/// Does the side to move have anything but pawns and a king?
///
/// Null-move pruning assumes having the move is an advantage. In king-and-pawn
/// endgames that is false -- in zugzwang every move worsens the position -- so
/// the heuristic is switched off when only pawns remain.
fn has_pieces(cb: &Chessboard, is_white_turn: bool) -> bool {
    let pieces = if is_white_turn {
        cb.white_knights | cb.white_bishops | cb.white_rooks | cb.white_queens
    } else {
        cb.black_knights | cb.black_bishops | cb.black_rooks | cb.black_queens
    };
    pieces != 0
}

/// Score of a drawn position, from either side's point of view.
pub const DRAW: i32 = 0;

/// Mate scores are kept this far from the ends of the range so that negating
/// one, which negamax does at every node, cannot overflow.
const MATE_BOUND: i32 = 1000;

/// Plies to mate encoded in `score`, or `None` if it is an ordinary score.
///
/// This is the inverse of the mate score [`terminal_score`] produces, and lives
/// beside it so the two cannot drift apart. The distance is carried in the
/// score itself, so unlike the depth-relative encoding this replaced, no
/// context from the caller is needed to read it back.
///
/// **Signed like the score it decodes**: positive when the side to move gives
/// the mate, negative when it is the one being mated. The two are separate
/// branches of the encoding and a bare distance cannot tell them apart, which
/// is how `mate N` was once reported for a position the engine was losing.
pub fn mate_in_plies(score: i32) -> Option<i32> {
    if score > MATE_SCORE_THRESHOLD {
        Some(-score - MATED_AT_ROOT) // Delivering: the negation of a mated score.
    } else if score < -MATE_SCORE_THRESHOLD {
        Some(-(score - MATED_AT_ROOT)) // Being mated: negative, by the doc above.
    } else {
        None
    }
}

/// The line the search currently believes both sides will play.
///
/// Walked out of the transposition table: each position remembers the move that
/// was best there, so following those moves reconstructs the line. The table
/// identifies a move only by the squares it touched, so the matching legal move
/// has to be found at each step.
///
/// Stops at `max_len`, at the first position with nothing stored, or on a
/// repetition -- a line that returns to a position it already visited would
/// otherwise loop forever.
pub fn principal_variation(
    cb: &Chessboard,
    table: &TranspositionTable,
    max_len: usize,
) -> Vec<Chessboard> {
    let mut line = Vec::with_capacity(max_len);
    let mut position = *cb;
    let mut seen: Vec<u64> = Vec::with_capacity(max_len);

    for _ in 0..max_len {
        let key = zobrist::hash(&position);
        if seen.contains(&key) {
            break; // The line repeats; stop rather than cycle.
        }
        seen.push(key);

        let Some(wanted) = table.best_move(key) else { break };

        let is_white = position.side_to_move == Color::White;
        let parent_occupancy = if is_white {
            position.get_white_occupancy()
        } else {
            position.get_black_occupancy()
        };

        let next = legal_moves(&position)
            .into_iter()
            .map(|m| m.chessboard)
            .find(|child| move_key(parent_occupancy, child, is_white) == wanted);

        match next {
            Some(child) => {
                line.push(child);
                position = child;
            }
            // The entry belongs to another position that hashed to the same
            // slot, or the move is no longer legal. Either way the line ends.
            None => break,
        }
    }

    line
}

/// Does this capture lose material once the exchange plays out?
///
/// Move generation yields positions rather than moves, so the squares involved
/// are recovered by diffing the mover's occupancy: the square it left and the
/// one it arrived on.
fn loses_material(
    parent: &Chessboard,
    parent_occupancy: u64,
    child: &Chessboard,
    is_white_turn: bool,
) -> bool {
    let child_occupancy = if is_white_turn {
        child.get_white_occupancy()
    } else {
        child.get_black_occupancy()
    };
    let vacated = parent_occupancy & !child_occupancy;
    let filled = child_occupancy & !parent_occupancy;

    // Castling moves two pieces and en passant captures off the target square;
    // neither is an exchange worth statically resolving, so leave them alone.
    if vacated.count_ones() != 1 || filled.count_ones() != 1 {
        return false;
    }

    let from = vacated.trailing_zeros() as usize;
    let to = filled.trailing_zeros() as usize;
    if parent.piece_square[to] == crate::piece::ColoredPiece::Empty {
        return false; // En passant or a promotion push, not a capture on `to`.
    }

    crate::see::see(parent, from, to) < 0
}

fn safe_neg(value: i32) -> i32 {
    if value == i32::MIN {
        i32::MAX // Return max value instead of overflowing
    } else {
        -value
    }
}

#[cfg(test)]
mod abort_tests {
    use super::*;
    use crate::chessboard::Chessboard;

    /// A search stopped by the clock must not write to the transposition table.
    ///
    /// It did. The nodes on the path being searched when the flag went up
    /// finished their move loops: every child after that returned the
    /// placeholder 0 from the stop check, and each of those nodes then stored
    /// `max(real children, 0)` at its full depth. The table lasts the whole
    /// game, so a later search could read it back as a result. Measured over 8
    /// self-play games at 10+0.1: 493 stores after the stop, 48% of them
    /// exactly 0 against 1% of ordinary stores, and 24% contradicting their own
    /// bound by more than 100cp against a cold re-search, where ordinary stores
    /// do so 7.5% of the time -- a lost pawn ending at -714 stored as "at least
    /// 0".
    ///
    /// Several node budgets, so the stop lands at different depths of the
    /// stack: near the root, mid-tree and near the leaves.
    #[test]
    fn a_stopped_search_writes_nothing_to_the_table() {
        let fen = "r1bq1rk1/pp2bppp/2n1pn2/2pp4/3P1B2/2PBPN2/PP1N1PPP/R2Q1RK1 w - - 0 9";
        let board = Chessboard::from_fen(fen).unwrap();
        for limit in [3_000u64, 20_000, 77_777, 150_000, 400_000] {
            let mut history = History::new();
            history.ensure_table(16);
            reset_nodes();
            STOP.store(false, Ordering::Relaxed);
            test_hooks::STORES_WHILE_STOPPED.store(0, Ordering::Relaxed);
            test_hooks::NODE_LIMIT.store(limit, Ordering::Relaxed);

            let _ = nega_max_alpha_beta_best_move(
                &board, 12, true, i32::MIN + 1, i32::MAX - 1, &mut history,
            );

            let stopped = STOP.load(Ordering::Relaxed);
            let stores = test_hooks::STORES_WHILE_STOPPED.load(Ordering::Relaxed);
            test_hooks::NODE_LIMIT.store(0, Ordering::Relaxed);
            STOP.store(false, Ordering::Relaxed);

            assert!(stopped, "limit {limit}: the search finished before the stop, so this tested nothing");
            assert_eq!(stores, 0, "limit {limit}: {stores} table stores were made after the stop");
        }
    }
}
