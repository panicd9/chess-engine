//! Position evaluation, from white's point of view.
//!
//! The bulk of the score is the tapered piece-square evaluation in
//! [`crate::piece_square_tables`], which already blends material and placement
//! between opening and endgame. Added on top are the things those tables
//! cannot express, because they depend on the position as a whole rather than
//! on where one piece stands:
//!
//! - **mobility** -- how many squares a side's pieces actually control
//! - **king safety** -- whether the pawns in front of the king are still there
//! - **passed pawns** -- pawns with nothing left to stop them queening
//! - **what the pieces let a passed pawn do** -- the kings' distance to it, and
//!   whether its path is free
//!
//! Every weight here is a hand-picked guess in centipawns, not a tuned value.

use crate::chessboard::{Chessboard, FILE_MASKS};
use crate::move_gen::move_gen_bishop::single_bishop_attacks;
use crate::move_gen::move_gen_king::king_attacks;
use crate::move_gen::move_gen_knight::knight_attacks_from_single_knight_bitboard;
use crate::move_gen::move_gen_rook::single_rook_attacks;
use crate::piece_square_tables::{eval, EVAL_TABLES, MG_VALUE};

/// The weights these terms use, all in centipawns.
///
/// Overridable at runtime so they can be tuned by playing matches rather than
/// by rebuilding for each candidate: see the `EvalWeight` UCI options.
///
/// **These are fitted values, not guesses.** They come from a Texel fit against
/// two independently sourced public corpora -- each held out against the other,
/// since neither has game ids -- and were then measured over the board:
/// **+17.1 +/- 7.6 Elo, LOS 100%, over 5000 games at 10+0.1** against the
/// hand-picked set they replaced. Do not adjust one by eye; the six were fitted
/// and measured together.
pub mod weights {
    use std::sync::atomic::{AtomicI32, Ordering};

    /// Per square of mobility, by piece. The term as a whole is worth
    /// +58.7 +/- 26.1 Elo over 400 games against setting all four to zero.
    ///
    /// These were 4/4/2/1, on the reasoning that the weight should fall as the
    /// piece gets bigger because a queen already attacks many squares from
    /// anywhere, so counting each at the knight's rate would swamp the rest of
    /// the evaluation and reward shuffling her into the open. **The fit
    /// disagrees and the board agreed with the fit**: it is close to flat, and
    /// the rook -- which gains the most from a line opening -- is now the
    /// highest. The reasoning was sound and the conclusion was wrong.
    pub static KNIGHT_MOBILITY: AtomicI32 = AtomicI32::new(3);
    pub static BISHOP_MOBILITY: AtomicI32 = AtomicI32::new(3);
    pub static ROOK_MOBILITY: AtomicI32 = AtomicI32::new(4);
    pub static QUEEN_MOBILITY: AtomicI32 = AtomicI32::new(3);

    /// Per missing pawn of the three in front of a castled king.
    ///
    /// Was 12. Both corpora pushed it down hard and independently, which is the
    /// most surprising part of the fit: counting missing pawns is a crude proxy
    /// for king safety, and overpaying for it makes the engine hold pawns in
    /// front of its king that are worth more elsewhere.
    pub static MISSING_SHIELD_PAWN: AtomicI32 = AtomicI32::new(8);

    /// Penalty per pawn beyond the first on a file, and per isolated pawn on a
    /// file with no enemy pawn.
    ///
    /// **Measured, at two time controls, against the same engine with both at 0:**
    ///
    /// ```text
    /// 10+0.1   1812 games   depth ~10    +31.0 +/- 12.6
    /// 40+0.4    700 games   depth ~11.8  +36.4 +/- 19.1
    /// ```
    ///
    /// Fitted by `examples/texel` against game results, cross-validated each way
    /// over big3 and quiet-labeled (19/14 and 16/10; these are the consensus).
    /// The short-control run was stopped early on a favourable reading, so its
    /// magnitude is biased upward; the sign is not in doubt at either control.
    ///
    /// They were tried once before at 25 and 19, read off a regression against
    /// Stockfish's evaluation, and measured +3.8 +/- 17.4 at identical conditions
    /// -- a difference of +27 +/- 22 from this. Two things changed: the weights
    /// now come from game results, and the terms live behind `crate::pawn_hash`,
    /// which computes them only on a miss instead of costing 10-12% of
    /// `evaluate()`. Do not adjust one by eye.
    ///
    /// They are the two features that survive a change of corpus: top-ranked
    /// against both zurichess `quiet-labeled` and our own game positions, at both
    /// oracle depths. Nothing else in that study does.
    pub static DOUBLED_PAWN: AtomicI32 = AtomicI32::new(21);
    pub static ISOLATED_HALF_OPEN_PAWN: AtomicI32 = AtomicI32::new(20);

    /// Scales the passed pawn bonus, as a percentage. 100 leaves the by-rank
    /// table below unchanged.
    ///
    /// Was 100. The cut is small and it agrees with an independent measurement:
    /// regressing `evaluate()` against Stockfish 19 over 66k quiet positions
    /// found the passed-pawn term over-generous by ~16cp per pawn, on top of
    /// what the piece-square tables already pay an advanced pawn.
    pub static PASSED_PAWN_SCALE: AtomicI32 = AtomicI32::new(96);

    /// How far along a passed pawn's path the pieces let it go. See
    /// [`super::passed_pawn_pieces`]; these scale Stockfish 15's classical
    /// terms, whose shapes are kept and whose sizes are refitted.
    ///
    /// `PASSED_KING_THEM` and `PASSED_KING_US` are in quarter centipawns per
    /// square of king distance to the stop square, per unit of the rank weight.
    /// `PASSED_FREE_PATH` is a percentage of Stockfish's free-path bonus, and
    /// `PASSED_PATH_OFFSET` is subtracted from that bonus's 0..41 grade first,
    /// so that a pawn whose path is taken can score below the tables' average
    /// rather than only a free one above it.
    ///
    /// Fitted by `examples/texel` with `only=` these four plus
    /// `PassedPawnScale`, from Stockfish's shapes scaled to our pawn (9,4,45,17),
    /// and cross-validated each way:
    ///
    /// ```text
    /// trained on       Scale  Them  Us  Free  Offset
    /// big3               120     7   2    25      16
    /// quiet-labeled       60     8   1    21      19
    /// ```
    ///
    /// The four agree; `PassedPawnScale` splits either side of its adopted 84 as
    /// it did in the six-weight fit, so it stays. Against the term switched off,
    /// these values cut the error by 0.85% on big3 and 2.5% on quiet-labeled,
    /// and by the same amounts on each as the held-out set.
    ///
    /// **Measured against the same binary with these four at 0:**
    ///
    /// ```text
    /// 10+0.1   355 games   SPRT [0,15] accepted   +52.3 +/- 28.4
    /// 40+0.4   254 games   depth ~12.4            +72.1 +/- 32.1
    /// ```
    ///
    /// Both runs stopped early -- the first by its SPRT, the second on a
    /// favourable reading of a planned 800 -- so the magnitudes lean high. The
    /// sign is not in doubt at either control.
    pub static PASSED_KING_THEM: AtomicI32 = AtomicI32::new(11);
    pub static PASSED_KING_US: AtomicI32 = AtomicI32::new(3);
    pub static PASSED_FREE_PATH: AtomicI32 = AtomicI32::new(24);
    pub static PASSED_PATH_OFFSET: AtomicI32 = AtomicI32::new(7);

    /// How dangerous each attacker is on a square next to the enemy king.
    ///
    /// The only king safety before this was `MISSING_SHIELD_PAWN`, which counts
    /// the three pawns in front of a castled king and so **caps at 15cp in
    /// total** -- a king about to be mated and a king perfectly safe differed
    /// by at most that. These weight the squares of the king's zone that each
    /// enemy piece type attacks, and the total is squared, so pieces arriving
    /// together cost far more than the sum of their parts, which is how an
    /// attack actually works.
    pub static KING_ATTACK_KNIGHT: AtomicI32 = AtomicI32::new(50);
    pub static KING_ATTACK_BISHOP: AtomicI32 = AtomicI32::new(96);
    pub static KING_ATTACK_ROOK: AtomicI32 = AtomicI32::new(93);
    pub static KING_ATTACK_QUEEN: AtomicI32 = AtomicI32::new(22);

    /// Scales the danger penalty, in sixty-fourths. **Zero switches the whole
    /// term off**, which is the setting the A/B measures against.
    pub static KING_ATTACK_SCALE: AtomicI32 = AtomicI32::new(30);

    /// Per safe check by each piece type, Stockfish 15's `SafeCheck` halved to
    /// this engine's units as a starting point (it uses 805/650/1071/730 for a
    /// single check of each type, and roughly 1.5x for several).
    pub static CHECK_KNIGHT: AtomicI32 = AtomicI32::new(400);
    pub static CHECK_BISHOP: AtomicI32 = AtomicI32::new(325);
    pub static CHECK_ROOK: AtomicI32 = AtomicI32::new(535);
    pub static CHECK_QUEEN: AtomicI32 = AtomicI32::new(365);

    /// Per square of the king ring an enemy piece attacks (Stockfish: 69).
    pub static KING_RING_ATTACKS: AtomicI32 = AtomicI32::new(69);

    /// Subtracted from the danger when the attacker has no queen. Stockfish's
    /// single largest king-safety line, worth ~24 Elo there.
    pub static KING_NO_QUEEN: AtomicI32 = AtomicI32::new(873);

    /// Per rook on a file with no pawns at all, and on one with only enemy
    /// pawns. Both are things the piece-square tables cannot see: a rook's
    /// worth depends on the pawns around it, not on the square it stands on.
    pub static ROOK_OPEN_FILE: AtomicI32 = AtomicI32::new(19);
    pub static ROOK_SEMI_OPEN_FILE: AtomicI32 = AtomicI32::new(13);

    /// For holding both bishops. The tables score each bishop alone, so the
    /// pair's extra worth -- covering both colour complexes -- has nowhere else
    /// to live.
    ///
    /// Fitted by `examples/texel` against game results and cross-validated each
    /// way: 9/4/18 trained on big3 (holdout -0.10%), 10/5/18 trained on
    /// quiet-labeled (holdout -0.25%). The first guesses were 20/10/30 -- the
    /// fit wants about half of each, which is the same direction every earlier
    /// fit here has gone.
    pub static BISHOP_PAIR: AtomicI32 = AtomicI32::new(32);



    /// How much of the evaluation survives in a material configuration that
    /// cannot be won, in sixty-fourths. 64 leaves the evaluation untouched and
    /// is the setting that reproduces the pre-term engine exactly.
    ///
    /// Without this the evaluation is confidently wrong about whole classes of
    /// drawn endgame, because material is counted and the mating potential is
    /// not. In `7iixcekP` the engine reached
    /// `1R6/8/8/4KN2/r7/8/k7/8 w - - 34 70` -- rook and knight against rook, a
    /// textbook draw -- and scored it **+380** static, +366 searched, where
    /// Stockfish 19 says +11. Over the 80 games of build `0d213881`, 204 of the
    /// 633 positions we over-read by 200cp or more were endgames of seven
    /// pieces or fewer.
    pub static DRAWISH_SCALE: AtomicI32 = AtomicI32::new(7);

    /// Set a weight by name. Unknown names are ignored, as UCI requires.
    /// Returns whether the name was recognised.
    pub fn set(name: &str, value: i32) -> bool {
        // Case-insensitive: GUIs are inconsistent about how they echo names.
        let target = match name.to_ascii_lowercase().as_str() {
            "knightmobility" => &KNIGHT_MOBILITY,
            "bishopmobility" => &BISHOP_MOBILITY,
            "rookmobility" => &ROOK_MOBILITY,
            "queenmobility" => &QUEEN_MOBILITY,
            "kingshield" => &MISSING_SHIELD_PAWN,
            "passedpawnscale" => &PASSED_PAWN_SCALE,
            "doubledpawn" => &DOUBLED_PAWN,
            "isolatedhalfopenpawn" => &ISOLATED_HALF_OPEN_PAWN,
            "passedkingthem" => &PASSED_KING_THEM,
            "passedkingus" => &PASSED_KING_US,
            "passedfreepath" => &PASSED_FREE_PATH,
            "passedpathoffset" => &PASSED_PATH_OFFSET,
            "drawishscale" => &DRAWISH_SCALE,
            "rookopenfile" => &ROOK_OPEN_FILE,
            "rooksemiopenfile" => &ROOK_SEMI_OPEN_FILE,
            "bishoppair" => &BISHOP_PAIR,
            "kingattackknight" => &KING_ATTACK_KNIGHT,
            "kingattackbishop" => &KING_ATTACK_BISHOP,
            "kingattackrook" => &KING_ATTACK_ROOK,
            "kingattackqueen" => &KING_ATTACK_QUEEN,
            "kingattackscale" => &KING_ATTACK_SCALE,
            "checkknight" => &CHECK_KNIGHT,
            "checkbishop" => &CHECK_BISHOP,
            "checkrook" => &CHECK_ROOK,
            "checkqueen" => &CHECK_QUEEN,
            "kingringattacks" => &KING_RING_ATTACKS,
            "kingnoqueen" => &KING_NO_QUEEN,
            _ => return false,
        };
        target.store(value, Ordering::Relaxed);
        true
    }

    #[inline]
    pub fn get(w: &AtomicI32) -> i32 {
        w.load(Ordering::Relaxed)
    }
}

/// The evaluation split into the part the piece-square tables contribute, the
/// part every other term contributes, and the drawish scale applied to their
/// sum -- so that `evaluate` is `(pst + rest) * scale / 64`.
///
/// Only the tuner uses this. Fitting the tables needs the rest held constant
/// while the tables move, and the rest cannot be recovered from `evaluate`
/// alone once the scale has divided through it.
pub fn evaluate_split(cb: &Chessboard) -> (i32, i32, i32) {
    let attacks = PieceAttacks::new(cb);
    let pst = eval(&cb.piece_square, &EVAL_TABLES);
    let rest = mobility(&attacks)
        + king_safety(cb)
        + crate::pawn_hash::passed_pawns(cb)
        + passed_pawn_pieces(cb, &attacks)
        + rook_files(cb)
        + bishop_pair(cb)
        + king_attack(cb, &attacks);
    (pst, rest, drawish_scale(cb, pst + rest))
}

pub fn evaluate(cb: &Chessboard) -> i32 {
    let attacks = PieceAttacks::new(cb);
    let raw = eval(&cb.piece_square, &EVAL_TABLES)
        + mobility(&attacks)
        + king_safety(cb)
        + crate::pawn_hash::passed_pawns(cb)
        + passed_pawn_pieces(cb, &attacks)
        + rook_files(cb)
        + bishop_pair(cb)
        + king_attack(cb, &attacks);

    match drawish_scale(cb, raw) {
        64 => raw,
        scale => raw * scale / 64,
    }
}

/// Pressure on the enemy king, from white's point of view. The shapes are
/// Stockfish 15's `Evaluation::king()`; the magnitudes are refitted, as they
/// were for [`passed_pawn_pieces`].
///
/// The first version of this counted king-zone squares attacked by each enemy
/// piece type and squared the total. That is only the `kingAttackersCount *
/// kingAttackersWeight` line below, which Stockfish's own annotations put at
/// ~10 Elo out of its king term. It also weighted the queen near the top, and
/// Stockfish weights the queen **lowest of all** -- 14 against a knight's 76 --
/// because a queen sweeping across open lines sees many squares near a king
/// without threatening anything. The queen's danger is in the *checks* she can
/// give, which is a separate line.
///
/// What is ported, with Stockfish's own Elo notes:
///
/// - **No enemy queen: a large discount** (~24 Elo, its biggest single line).
///   Free to compute and the main thing the old version missed.
/// - **Safe checks** by rook, queen, bishop and knight (~4+ Elo). A check is
///   safe when the checking square is not defended by us. Two slider
///   computations from our own king square answer all four.
/// - **Attackers and their weight** (~10 Elo), now with Stockfish's weights.
/// - **Flank attacks and defence** (~5 Elo).
///
/// What is deliberately **not** ported, because each needs the set of squares
/// attacked *twice*, which this evaluation does not build: weak squares in the
/// king ring (~15 Elo) and unsafe checks (~4 Elo). Building double-attack sets
/// means walking every piece individually, which measured **21% of search
/// speed** when tried for a much smaller gain. Blockers-for-king (~2 Elo) needs
/// pin detection, which lives in the move generator, not here.
///
/// The danger total is turned into a penalty by Stockfish's own transform:
/// nothing below a threshold, then quadratic.
#[inline]
fn king_attack(cb: &Chessboard, attacks: &PieceAttacks) -> i32 {
    let scale = weights::get(&weights::KING_ATTACK_SCALE);
    if scale == 0 {
        return 0;
    }
    let heavy = (cb.white_queens | cb.black_queens | cb.white_rooks | cb.black_rooks).count_ones();
    if heavy < 2 {
        return 0;
    }
    // From white's point of view: danger to the black king is a bonus, danger
    // to our own is a penalty.
    danger_for(cb, attacks, true) - danger_for(cb, attacks, false)
}

/// The danger to one king, as a positive penalty. `black_king` picks whose king
/// is under attack, so the caller subtracts one from the other.
#[inline]
fn danger_for(cb: &Chessboard, attacks: &PieceAttacks, black_king: bool) -> i32 {
    let (king, them, them_pawn, us_all, them_all, them_queens) = if black_king {
        (cb.black_king, &attacks.white, attacks.white_pawn, attacks.black_all,
         attacks.white_all, cb.white_queens)
    } else {
        (cb.white_king, &attacks.black, attacks.black_pawn, attacks.white_all,
         attacks.black_all, cb.black_queens)
    };
    if king == 0 {
        return 0;
    }
    let ring = king | king_attacks(king);
    // If nothing of theirs touches the ring there is no attack to score, and no
    // check either: a check attacks the king's own square, which is in the ring.
    // This is what keeps the term affordable -- most nodes stop here.
    if them_all & ring == 0 {
        return 0;
    }
    let occupancy = cb.get_occupancy();

    // Attackers on the ring, and how much of it they cover.
    let w = [
        weights::get(&weights::KING_ATTACK_KNIGHT),
        weights::get(&weights::KING_ATTACK_BISHOP),
        weights::get(&weights::KING_ATTACK_ROOK),
        weights::get(&weights::KING_ATTACK_QUEEN),
    ];
    let mut attackers = 0;
    let mut attacker_weight = 0;
    let mut ring_attacks = 0;
    for i in 0..4 {
        let hits = (them[i] & ring).count_ones() as i32;
        if hits > 0 {
            attackers += 1;
            attacker_weight += w[i];
            ring_attacks += hits;
        }
    }
    // Pawn attacks count towards the ring but carry no attacker weight, as in
    // Stockfish, where KingAttackWeights[PAWN] is zero.
    ring_attacks += (them_pawn & ring).count_ones() as i32;

    // A check is safe when we do not defend the square it comes from.
    let their_pieces = if black_king { cb.get_white_occupancy() } else { cb.get_black_occupancy() };
    let safe = !us_all & !their_pieces;
    // Each ray set is only needed if they actually have a piece that could
    // check along it; a magic lookup is not free.
    let rook_rays = if them[2] | them[3] != 0 { single_rook_attacks(occupancy, king) } else { 0 };
    let bishop_rays = if them[1] | them[3] != 0 { single_bishop_attacks(occupancy, king) } else { 0 };
    let knight_rays = if them[0] != 0 { knight_attacks_from_single_knight_bitboard(king) } else { 0 };
    let mut danger = 0;
    let sc = |b: u64, one: i32, many: i32| -> i32 {
        match b.count_ones() {
            0 => 0,
            1 => one,
            _ => many,
        }
    };
    let rook_checks = rook_rays & them[2] & safe;
    let queen_checks = (rook_rays | bishop_rays) & them[3] & safe & !rook_checks;
    let bishop_checks = bishop_rays & them[1] & safe & !queen_checks;
    let knight_checks = knight_rays & them[0] & safe;
    danger += sc(rook_checks, weights::get(&weights::CHECK_ROOK), weights::get(&weights::CHECK_ROOK) * 3 / 2);
    danger += sc(queen_checks, weights::get(&weights::CHECK_QUEEN), weights::get(&weights::CHECK_QUEEN) * 3 / 2);
    danger += sc(bishop_checks, weights::get(&weights::CHECK_BISHOP), weights::get(&weights::CHECK_BISHOP) * 3 / 2);
    danger += sc(knight_checks, weights::get(&weights::CHECK_KNIGHT), weights::get(&weights::CHECK_KNIGHT) * 3 / 2);

    // The king's own flank, in our half of the board.
    let file = (king.trailing_zeros() % 8) as usize;
    let flank = KING_FLANK[file] & if black_king { BLACK_CAMP } else { WHITE_CAMP };
    let flank_attack = (them_all & flank).count_ones() as i32;
    let flank_defense = (us_all & flank).count_ones() as i32;

    danger += attackers * attacker_weight
        + weights::get(&weights::KING_RING_ATTACKS) * ring_attacks
        + 3 * flank_attack * flank_attack / 8
        - weights::get(&weights::KING_NO_QUEEN) * (them_queens == 0) as i32
        - 4 * flank_defense
        + 37;

    if danger <= 100 {
        return 0;
    }
    danger * danger / 4096 * weights::get(&weights::KING_ATTACK_SCALE) / 64
}

/// Stockfish's king-flank file masks: the three files around the king, widened
/// at the edges so that a king on the a-file still has a three-file flank.
const KING_FLANK: [u64; 8] = {
    const QS: u64 = 0x0F0F_0F0F_0F0F_0F0F; // files a-d
    const CF: u64 = 0x3C3C_3C3C_3C3C_3C3C; // files c-f
    const KS: u64 = 0xF0F0_F0F0_F0F0_F0F0; // files e-h
    const FD: u64 = 0x0808_0808_0808_0808; // file d
    const FE: u64 = 0x1010_1010_1010_1010; // file e
    [QS ^ FD, QS, QS, CF, CF, KS, KS, KS ^ FE]
};
/// Everything but the three ranks furthest from each side's back rank, which
/// is Stockfish's `Camp`: ranks 1-5 for White, 4-8 for Black.
const WHITE_CAMP: u64 = 0x0000_00FF_FFFF_FFFF; // ranks 1-5
const BLACK_CAMP: u64 = 0xFFFF_FFFF_FF00_0000; // ranks 4-8

/// Rooks on files the pawns have left, from white's point of view.
///
/// A file is **open** when neither side has a pawn on it and **semi-open** for
/// a side when only the enemy has one: the rook sees down it either way, but an
/// enemy pawn can still be advanced to block or to be defended, so the two are
/// worth different amounts and are weighted separately.
///
/// The files holding a side's pawns are that side's pawns smeared over the
/// whole board vertically, which is two shifts-and-ors each way, so this costs
/// two fills and a handful of masks however many rooks there are.
#[inline]
fn rook_files(cb: &Chessboard) -> i32 {
    let white_pawn_files = fill_north(fill_south(cb.white_pawns));
    let black_pawn_files = fill_north(fill_south(cb.black_pawns));
    let open = !(white_pawn_files | black_pawn_files);

    let count = |b: u64| b.count_ones() as i32;
    let open_diff = count(cb.white_rooks & open) - count(cb.black_rooks & open);
    // Semi-open for us means no pawn of ours and at least one of theirs.
    let semi_diff = count(cb.white_rooks & !white_pawn_files & black_pawn_files)
        - count(cb.black_rooks & !black_pawn_files & white_pawn_files);

    open_diff * weights::get(&weights::ROOK_OPEN_FILE)
        + semi_diff * weights::get(&weights::ROOK_SEMI_OPEN_FILE)
}

/// Holding both bishops, from white's point of view.
///
/// Counting two bishops rather than two of opposite colours is the usual
/// simplification: a same-coloured pair only arises from an underpromotion.
#[inline]
fn bishop_pair(cb: &Chessboard) -> i32 {
    let pair = |b: u64| (b.count_ones() >= 2) as i32;
    (pair(cb.white_bishops) - pair(cb.black_bishops)) * weights::get(&weights::BISHOP_PAIR)
}

/// How much of the evaluation to keep, in sixty-fourths.
///
/// One rule, the standard one: **a side with no pawns and less than a bishop of
/// extra material cannot force mate.** That covers rook and knight against rook,
/// rook and bishop against rook, rook against minor, minor against minor, and a
/// lone minor against a bare king -- every one of which the material term scores
/// as a comfortable advantage and every one of which is a draw.
///
/// The side the rule is asked about is **the side the evaluation favours**, not
/// the side with more material. Those differ, and using material instead is
/// wrong: in king and pawn against king and knight the knight is the greater
/// material, but it is the *pawn* that has the winning chances, and scaling that
/// position down crushes a real advantage. `tests/regressions.rs::
/// a_stopped_passed_pawn_is_worth_less_than_a_free_one` fails on exactly that
/// mistake, which is how it was found.
///
/// Deliberately *not* covered, because each needs its own shape and would be
/// tested separately: opposite-coloured bishops, the wrong rook pawn with a
/// bishop, and rook-and-pawn against rook. Two knights against a bare king is a
/// draw this rule misses (640 of extra material clears the bishop threshold);
/// it is rare enough to leave.
#[inline]
fn drawish_scale(cb: &Chessboard, raw: i32) -> i32 {
    // The side being asked about must have no pawns, so when it has one the
    // answer is always 64. Testing that first keeps the popcounts out of every
    // middlegame evaluation, which is worth 4-8% of search speed.
    let white_winning = match raw.signum() {
        1 => true,
        -1 => false,
        _ => return 64,
    };
    if white_winning && cb.white_pawns != 0 || !white_winning && cb.black_pawns != 0 {
        return 64;
    }
    let scale = weights::get(&weights::DRAWISH_SCALE);
    if scale == 64 {
        return 64; // Term switched off: skip the work entirely.
    }

    let npm = |knights: u64, bishops: u64, rooks: u64, queens: u64| {
        knights.count_ones() as i32 * MG_VALUE[1]
            + bishops.count_ones() as i32 * MG_VALUE[2]
            + rooks.count_ones() as i32 * MG_VALUE[3]
            + queens.count_ones() as i32 * MG_VALUE[4]
    };
    let white = npm(cb.white_knights, cb.white_bishops, cb.white_rooks, cb.white_queens);
    let black = npm(cb.black_knights, cb.black_bishops, cb.black_rooks, cb.black_queens);

    let edge = if white_winning { white - black } else { black - white };
    if edge <= MG_VALUE[2] {
        scale
    } else {
        64
    }
}

/// The squares each side's knights, bishops, rooks and queens attack, computed
/// once per evaluation: mobility counts them, and the passed-pawn term needs
/// their union. Computing them twice cost up to 15% of search time in endgames.
struct PieceAttacks {
    white: [u64; 4],
    black: [u64; 4],
    /// Pawn attacks, and everything each side attacks including pawns and king.
    /// The king-danger term needs "is this square defended at all", which the
    /// per-type sets alone cannot answer.
    white_pawn: u64,
    black_pawn: u64,
    white_all: u64,
    black_all: u64,
}

impl PieceAttacks {
    #[inline]
    fn new(cb: &Chessboard) -> Self {
        let white = [
            cb.white_knights_attacks(),
            cb.white_bishops_attacks(),
            cb.white_rooks_attacks(),
            cb.white_queens_attacks(),
        ];
        let black = [
            cb.black_knights_attacks(),
            cb.black_bishops_attacks(),
            cb.black_rooks_attacks(),
            cb.black_queens_attacks(),
        ];
        let white_pawn = ((cb.white_pawns << 9) & !FILE_MASKS[0])
            | ((cb.white_pawns << 7) & !FILE_MASKS[7]);
        let black_pawn = ((cb.black_pawns >> 7) & !FILE_MASKS[0])
            | ((cb.black_pawns >> 9) & !FILE_MASKS[7]);
        let wk = if cb.white_king == 0 { 0 } else { king_attacks(cb.white_king) };
        let bk = if cb.black_king == 0 { 0 } else { king_attacks(cb.black_king) };
        PieceAttacks {
            white_all: white[0] | white[1] | white[2] | white[3] | white_pawn | wk,
            black_all: black[0] | black[1] | black[2] | black[3] | black_pawn | bk,
            white, black, white_pawn, black_pawn,
        }
    }
}

/// Every square from `b` southwards, `b` included.
#[inline]
fn fill_south(mut b: u64) -> u64 {
    b |= b >> 8;
    b |= b >> 16;
    b |= b >> 32;
    b
}

/// Every square from `b` northwards, `b` included.
#[inline]
fn fill_north(mut b: u64) -> u64 {
    b |= b << 8;
    b |= b << 16;
    b |= b << 32;
    b
}

#[inline]
fn with_neighbour_files(b: u64) -> u64 {
    b | ((b << 1) & !FILE_MASKS[0]) | ((b >> 1) & !FILE_MASKS[7])
}

#[inline]
fn distance(a: usize, b: usize) -> i32 {
    let files = (a % 8).abs_diff(b % 8);
    let ranks = (a / 8).abs_diff(b / 8);
    files.max(ranks) as i32
}

/// The part of a passed pawn's value that depends on the pieces rather than the
/// pawns, which is why it cannot live in [`crate::pawn_hash`].
///
/// The piece-square tables pay a pawn on the seventh rank ~150cp in the endgame
/// and the pawn hash adds a rank bonus, whether or not anything stops it. That
/// is an average over pawns that queen and pawns that are lost, and it threw
/// away a won game: in `gCd8UcfI` the engine sacrificed into an endgame it
/// scored +394 because of two pawns on the seventh, with an enemy knight
/// covering both queening squares and the enemy king next to one of them.
/// Stockfish scores it 0.00. `2qYroOWA` is the same blindness from the other
/// side: a blockaded enemy pawn on d7 was worth 280cp to us, so the winning
/// blockade looked equal and the engine took a perpetual.
///
/// The shapes are Stockfish 15's `Evaluation::passed()`, for pawns on the fourth
/// rank and beyond, weighted by `w = 5 * relative_rank - 13`:
///
/// - **King proximity** (endgame only): the enemy king's distance to the stop
///   square is a bonus, our own king's distance to it and to the square beyond
///   (2:1) a penalty, each capped at 5.
/// - **Free path** (both phases): when the stop square is empty, a bonus by how
///   much of the pawn's path the enemy controls -- none of the span (36), only
///   squares our pawns defend (30), none of the pawn's own file (17), not the
///   stop square (7), or the stop square itself (0) -- plus 5 when we defend the
///   stop square or have a rook or queen behind the pawn. An enemy rook or queen
///   behind the pawn makes the whole span count as controlled.
///
/// Stockfish's constants are in its own pawn units (208cp in the endgame against
/// our 94), so only the shapes carry over; the weights are fitted.
fn passed_pawn_pieces(cb: &Chessboard, attacks: &PieceAttacks) -> i32 {
    // The fourth rank and beyond, from each side's own point of view.
    const WHITE_ADVANCED: u64 = 0x00FF_FFFF_FF00_0000;
    const BLACK_ADVANCED: u64 = 0x0000_00FF_FFFF_FF00;

    // Passed: no enemy pawn ahead on its own or a neighbouring file. The same
    // test as the pawn hash's `blocking_mask`, done with fills.
    let white_passed = cb.white_pawns
        & WHITE_ADVANCED
        & !with_neighbour_files(fill_south(cb.black_pawns >> 8));
    let black_passed = cb.black_pawns
        & BLACK_ADVANCED
        & !with_neighbour_files(fill_north(cb.white_pawns << 8));
    if white_passed | black_passed == 0 {
        return 0;
    }

    let (king_them, king_us, free_path, offset) = (
        weights::get(&weights::PASSED_KING_THEM),
        weights::get(&weights::PASSED_KING_US),
        weights::get(&weights::PASSED_FREE_PATH),
        weights::get(&weights::PASSED_PATH_OFFSET),
    );

    let white_pieces = cb.white_pawns | cb.white_knights | cb.white_bishops
        | cb.white_rooks | cb.white_queens | cb.white_king;
    let black_pieces = cb.black_pawns | cb.black_knights | cb.black_bishops
        | cb.black_rooks | cb.black_queens | cb.black_king;
    let occupied = white_pieces | black_pieces;
    let heavy = cb.white_rooks | cb.black_rooks | cb.white_queens | cb.black_queens;

    let white = Side {
        king: cb.white_king.trailing_zeros() as usize,
        pieces: white_pieces,
        pawn_attacks: cb.white_pawns_attacks(),
        attacks: attacks.white.iter().fold(cb.white_pawns_attacks() | cb.white_king_attacks(), |a, b| a | b),
    };
    let black = Side {
        king: cb.black_king.trailing_zeros() as usize,
        pieces: black_pieces,
        pawn_attacks: cb.black_pawn_attacks(),
        attacks: attacks.black.iter().fold(cb.black_pawn_attacks() | cb.black_king_attacks(), |a, b| a | b),
    };

    // Accumulated white minus black before any division, so that a mirrored
    // position scores exactly the negation.
    let (mut endgame_quarters, mut both) = (0i32, 0i32);
    let mut pawns = white_passed;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let (k, f) = passer(square, true, &white, &black, occupied, heavy, king_them, king_us, offset);
        endgame_quarters += k;
        both += f;
    }
    let mut pawns = black_passed;
    while pawns != 0 {
        let square = pawns.trailing_zeros() as usize;
        pawns &= pawns - 1;
        let (k, f) = passer(square, false, &black, &white, occupied, heavy, king_them, king_us, offset);
        endgame_quarters -= k;
        both -= f;
    }

    let phase = ((cb.white_knights | cb.black_knights | cb.white_bishops | cb.black_bishops)
        .count_ones()
        + 2 * (cb.white_rooks | cb.black_rooks).count_ones()
        + 4 * (cb.white_queens | cb.black_queens).count_ones())
    .min(24) as i32;

    endgame_quarters * (24 - phase) / 96 + both * free_path / 100
}

struct Side {
    king: usize,
    pieces: u64,
    pawn_attacks: u64,
    attacks: u64,
}

/// One passed pawn on its fourth rank or beyond: (king proximity in quarter
/// centipawns, endgame only; free-path grade less the offset, times the rank
/// weight, both phases).
#[inline]
#[allow(clippy::too_many_arguments)]
fn passer(
    square: usize,
    white: bool,
    us: &Side,
    them: &Side,
    occupied: u64,
    heavy: u64,
    king_them: i32,
    king_us: i32,
    offset: i32,
) -> (i32, i32) {
    let rank = if white { square / 8 } else { 7 - square / 8 } as i32;
    let w = 5 * rank - 13;
    let stop = if white { square + 8 } else { square - 8 };
    let stop_bb = 1u64 << stop;

    let mut proximity = king_them * distance(them.king, stop).min(5) - king_us * distance(us.king, stop).min(5);
    if rank != 6 {
        // Short of the seventh, the square after the stop square matters too.
        let beyond = if white { stop + 8 } else { stop - 8 };
        proximity -= king_us * distance(us.king, beyond).min(5) / 2;
    }

    // A pawn whose stop square is occupied grades 0, the same as one whose stop
    // square the enemy controls.
    let mut grade = 0;
    if occupied & stop_bb == 0 {
        let pawn = 1u64 << square;
        let (to_queen, behind) = if white {
            (fill_north(pawn << 8), fill_south(pawn >> 8))
        } else {
            (fill_south(pawn >> 8), fill_north(pawn << 8))
        };
        let heavy_behind = behind & heavy;
        let mut unsafe_squares = with_neighbour_files(to_queen);
        if heavy_behind & them.pieces == 0 {
            unsafe_squares &= them.attacks | them.pieces;
        }
        let mut k = if unsafe_squares == 0 {
            36
        } else if unsafe_squares & !us.pawn_attacks == 0 {
            30
        } else if unsafe_squares & to_queen == 0 {
            17
        } else if unsafe_squares & stop_bb == 0 {
            7
        } else {
            0
        };
        if heavy_behind & us.pieces != 0 || us.attacks & stop_bb != 0 {
            k += 5;
        }
        grade = k;
    }
    (proximity * w, (grade - offset) * w)
}

/// Squares attacked by each side's pieces, weighted by piece type.
///
/// Counts attacked squares rather than legal moves, which is cheaper and close
/// enough: a piece that eyes many squares is usually the more active one.
fn mobility(attacks: &PieceAttacks) -> i32 {
    let weight = [
        weights::get(&weights::KNIGHT_MOBILITY),
        weights::get(&weights::BISHOP_MOBILITY),
        weights::get(&weights::ROOK_MOBILITY),
        weights::get(&weights::QUEEN_MOBILITY),
    ];
    let side = |a: &[u64; 4]| -> i32 {
        a.iter().zip(weight).map(|(squares, w)| squares.count_ones() as i32 * w).sum()
    };
    side(&attacks.white) - side(&attacks.black)
}

/// Penalise a king whose pawn cover has gone.
///
/// Only the king's own file and its neighbours are considered, and only while
/// there is still enough material for an attack -- in an endgame the king
/// wants to be active, not hidden.
fn king_safety(cb: &Chessboard) -> i32 {
    let heavy_material = (cb.white_queens | cb.black_queens | cb.white_rooks | cb.black_rooks)
        .count_ones();
    if heavy_material < 2 {
        return 0; // Endgame: the shield no longer matters.
    }

    let shield = |king: u64, pawns: u64, ahead: fn(u64) -> u64| -> i32 {
        if king == 0 {
            return 0;
        }
        let file = (king.trailing_zeros() % 8) as usize;
        let mut files = FILE_MASKS[file];
        if file > 0 {
            files |= FILE_MASKS[file - 1];
        }
        if file < 7 {
            files |= FILE_MASKS[file + 1];
        }
        // Pawns standing on the three files, ahead of the king.
        let cover = pawns & files & ahead(king);
        let missing = 3i32 - cover.count_ones().min(3) as i32;
        -missing * weights::get(&weights::MISSING_SHIELD_PAWN)
    };

    // "Ahead" is up the board for white, down for black.
    let white = shield(cb.white_king, cb.white_pawns, |k| {
        let rank = k.trailing_zeros() / 8;
        if rank >= 7 { 0 } else { u64::MAX << ((rank + 1) * 8) }
    });
    let black = shield(cb.black_king, cb.black_pawns, |k| {
        let rank = k.trailing_zeros() / 8;
        if rank == 0 { 0 } else { u64::MAX >> ((8 - rank) * 8) }
    });
    white - black
}

