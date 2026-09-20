//! Syzygy endgame tablebases: perfect play once few enough pieces are left.
//!
//! The tables answer win/draw/loss exactly for any position within their piece
//! count, which is something no evaluation term can approximate. The drawish
//! material scaling added earlier is a heuristic for the same ground -- rook
//! and knight against rook scored +380 before it and +47 after, where the truth
//! is a draw -- and a five-piece table simply knows.
//!
//! Probing is done through `shakmaty_syzygy`, which wants its own board type,
//! so a position crosses over as a FEN. That costs a parse per probe and is
//! only affordable because probes are rare: the search asks only when the piece
//! count is already inside the tables.
//!
//! Loading is **file by file rather than by directory**. `add_directory` gives
//! up on the first unusable file, which turns one truncated download into "no
//! tablebases at all" with an error naming neither the file nor the cause --
//! exactly what a half-finished 150 GB download produces.

use crate::chessboard::Chessboard;
use crate::display::to_fen;
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess};
use shakmaty_syzygy::{AmbiguousWdl, Tablebase};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// What the tables say, from the point of view of the side to move.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    Win,
    Draw,
    Loss,
}

static TABLES: OnceLock<Option<Tablebase<Chess>>> = OnceLock::new();
/// Largest position the loaded tables can answer. Zero means none are loaded,
/// which is the check the search makes before doing anything else.
static MAX_PIECES: AtomicUsize = AtomicUsize::new(0);
static HITS: AtomicUsize = AtomicUsize::new(0);

/// Load every table under the given colon-separated directories. Returns
/// (tables loaded, files rejected). Safe to call once; later calls are ignored.
pub fn load(path: &str) -> (usize, Vec<String>) {
    let mut tb = Tablebase::new();
    let mut loaded = 0usize;
    let mut bad = Vec::new();
    for dir in path.split(':').filter(|d| !d.is_empty()) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                bad.push(format!("{dir}: {e}"));
                continue;
            }
        };
        for entry in entries.flatten() {
            let p = entry.path();
            match p.extension().and_then(|e| e.to_str()) {
                Some("rtbw") | Some("rtbz") => {}
                _ => continue,
            }
            match tb.add_file(&p) {
                Ok(_) => loaded += 1,
                Err(e) => bad.push(format!("{}: {e}", p.file_name().unwrap().to_string_lossy())),
            }
        }
    }
    let max = tb.max_pieces();
    if loaded > 0 {
        MAX_PIECES.store(max, Ordering::Relaxed);
        let _ = TABLES.set(Some(tb));
    }
    (loaded, bad)
}

/// The largest piece count the loaded tables cover, or zero if none.
#[inline]
pub fn max_pieces() -> usize {
    MAX_PIECES.load(Ordering::Relaxed)
}

/// How many probes have hit, for reporting.
pub fn hits() -> usize {
    HITS.load(Ordering::Relaxed)
}

pub fn reset_hits() {
    HITS.store(0, Ordering::Relaxed);
}

/// Ask the tables about this position, from the side to move's point of view.
///
/// Returns `None` when there are no tables, when the position has too many
/// pieces, when castling rights are still present (Syzygy has no notion of
/// them), or when the particular table is missing from a partial download.
///
/// `CursedWin` and `BlessedLoss` are reported as draws: they are wins and
/// losses that the fifty-move rule takes away, and this engine enforces that
/// rule, so a draw is what it will actually get.
///
/// `MaybeWin` and `MaybeLoss` mean "win, or loss, unless the fifty-move counter
/// gets there first". They are only believed when the counter has just been
/// reset, where there is no ambiguity to resolve; otherwise they are reported
/// as draws. Claiming a win the rule then takes away is the worse error: the
/// search would steer into it and find nothing there.
pub fn probe(cb: &Chessboard) -> Option<Verdict> {
    let max = max_pieces();
    if max == 0 {
        return None;
    }
    if cb.white_can_castle_king_side
        || cb.white_can_castle_queen_side
        || cb.black_can_castle_king_side
        || cb.black_can_castle_queen_side
    {
        return None;
    }
    if cb.get_occupancy().count_ones() as usize > max {
        return None;
    }
    let tables = TABLES.get()?.as_ref()?;
    let pos: Chess = to_fen(cb)
        .parse::<Fen>()
        .ok()?
        .into_position(CastlingMode::Standard)
        .ok()?;
    let wdl = tables.probe_wdl(&pos).ok()?;
    HITS.fetch_add(1, Ordering::Relaxed);
    let fresh = cb.halfmove_clock == 0;
    Some(match wdl {
        AmbiguousWdl::Win => Verdict::Win,
        AmbiguousWdl::Loss => Verdict::Loss,
        AmbiguousWdl::MaybeWin if fresh => Verdict::Win,
        AmbiguousWdl::MaybeLoss if fresh => Verdict::Loss,
        _ => Verdict::Draw,
    })
}
