use chess_engine::chessboard::{Chessboard, Color};
use chess_engine::display::to_fen;
use chess_engine::move_gen::legal_moves;
use chess_engine::search::{History, nega_max_alpha_beta_best_move};

const A: i32 = i32::MIN + 1;
const B: i32 = i32::MAX - 1;

#[test]
fn stalemate_is_a_draw() {
    let cb = Chessboard::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
    assert!(legal_moves(&cb).is_empty());
    let (score, _) = nega_max_alpha_beta_best_move(&cb, 1, false, A, B, &mut History::new());
    println!("stalemate scores {score}");
    assert_eq!(score, 0);
}

#[test]
fn prefers_mate_over_stalemate() {
    let cb = Chessboard::from_fen("7k/8/6K1/8/8/8/5Q2/8 w - - 0 1").unwrap();
    for depth in 2..=4u32 {
        let (score, next) = nega_max_alpha_beta_best_move(&cb, depth, true, A, B, &mut History::new());
        let no_moves = legal_moves(&next).is_empty();
        let stalemate = no_moves && !next.is_black_king_under_attack();
        let mate = no_moves && next.is_black_king_under_attack();
        println!("depth {depth}: {} -> {}", to_fen(&next),
            if mate { "MATE" } else if stalemate { "STALEMATE" } else { "neither" });
        assert!(!stalemate, "chose stalemate over mate at depth {depth}");
        assert!(score > 0, "should be winning, got {score}");
    }
}

#[test]
fn still_finds_checkmate() {
    let cb = Chessboard::from_fen("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1").unwrap();
    let (score, next) = nega_max_alpha_beta_best_move(&cb, 3, true, A, B, &mut History::new());
    println!("mate search: {} score {score}", to_fen(&next));
    assert!(legal_moves(&next).is_empty() && next.is_black_king_under_attack(), "should be mate");
}

/// Quiescence is entered at depth 0 and recursed with `depth - 1` on a u32,
/// which panicked in any build with overflow checks on (i.e. every debug build).
#[test]
fn quiescence_depth_does_not_underflow() {
    let cb = Chessboard::from_fen(
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10").unwrap();
    let (score, _) = nega_max_alpha_beta_best_move(&cb, 3, true, A, B, &mut History::new());
    println!("search completed, score {score}");
}

/// Positions that are not legal chess used to be accepted, and then crashed the
/// move generator: the waiting side's king can be captured, and generating moves
/// for the resulting king-less board indexes KING_ATTACKS[64].
#[test]
fn illegal_fens_are_rejected() {
    assert!(Chessboard::from_fen("7k/6Q1/6K1/8/8/8/8/8 w - - 0 1").is_err(),
        "waiting side in check");
    assert!(Chessboard::from_fen("7k/8/8/8/8/8/8/8 w - - 0 1").is_err(), "no white king");
    assert!(Chessboard::from_fen("4k3/8/8/8/8/8/8/8 w - - 0 1").is_err(), "no white king");
    // Legal positions still parse.
    for fen in [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    ] {
        assert!(Chessboard::from_fen(fen).is_ok(), "{fen} should parse");
    }
}

/// `Chessboard` is Copy and is cloned at every search node, so its size is on
/// the hot path. repr(usize) on ColoredPiece made piece_square 512 bytes.
#[test]
fn board_stays_small() {
    let size = std::mem::size_of::<Chessboard>();
    println!("size_of::<Chessboard>() = {size}");
    assert!(size <= 256, "board grew to {size} bytes");
    assert_eq!(std::mem::size_of::<chess_engine::piece::ColoredPiece>(), 1);
}

// --- board-state invariants -------------------------------------------------
//
// `piece_square` is a redundant view of the twelve piece bitboards, and the
// capture helpers trust it to decide which piece to remove. When the two drift,
// the evaluation silently reads a piece that is not there and captures can
// remove the wrong one. Two bugs of exactly that shape have been fixed here
// (castling not clearing the rook's origin, and a capture-promotion writing the
// destination before reading it), so the invariant is worth asserting directly.

use chess_engine::piece::ColoredPiece;

fn expected_piece_square(cb: &Chessboard) -> [ColoredPiece; 64] {
    let mut squares = [ColoredPiece::Empty; 64];
    for (bitboard, piece) in [
        (cb.white_pawns, ColoredPiece::WhitePawn),
        (cb.white_knights, ColoredPiece::WhiteKnight),
        (cb.white_bishops, ColoredPiece::WhiteBishop),
        (cb.white_rooks, ColoredPiece::WhiteRook),
        (cb.white_queens, ColoredPiece::WhiteQueen),
        (cb.white_king, ColoredPiece::WhiteKing),
        (cb.black_pawns, ColoredPiece::BlackPawn),
        (cb.black_knights, ColoredPiece::BlackKnight),
        (cb.black_bishops, ColoredPiece::BlackBishop),
        (cb.black_rooks, ColoredPiece::BlackRook),
        (cb.black_queens, ColoredPiece::BlackQueen),
        (cb.black_king, ColoredPiece::BlackKing),
    ] {
        let mut remaining = bitboard;
        while remaining != 0 {
            squares[remaining.trailing_zeros() as usize] = piece;
            remaining &= remaining - 1;
        }
    }
    squares
}

fn assert_consistent(cb: &Chessboard, context: &str) {
    for (square, &want) in expected_piece_square(cb).iter().enumerate() {
        assert_eq!(
            cb.piece_square[square], want,
            "piece_square[{square}] disagrees with the bitboards after {context}"
        );
    }
    assert_eq!(
        cb.get_white_occupancy() & cb.get_black_occupancy(),
        0,
        "a square is occupied by both colours after {context}"
    );
}

fn walk(cb: &Chessboard, depth: u32, context: &str) {
    assert_consistent(cb, context);
    if depth == 0 {
        return;
    }
    for m in legal_moves(cb) {
        walk(&m.chessboard, depth - 1, context);
    }
}

#[test]
fn piece_square_stays_in_sync_with_bitboards() {
    for (fen, depth) in [
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 4),
        ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 3),
        ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 4),
        ("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 3),
        ("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 3),
        ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", 3),
    ] {
        walk(&Chessboard::from_fen(fen).unwrap(), depth, fen);
    }
}

/// Castling moves two pieces; the rook's home square must not be left occupied.
#[test]
fn castling_clears_the_rook_home_square() {
    for (fen, king_to, rook_home) in [
        ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", 6u32, 7usize),   // O-O,   h1
        ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", 2, 0),           // O-O-O, a1
        ("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", 62, 63),         // ..O-O, h8
        ("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", 58, 56),         // ..O-O-O, a8
    ] {
        let cb = Chessboard::from_fen(fen).unwrap();
        let castled = legal_moves(&cb)
            .into_iter()
            .map(|m| m.chessboard)
            .find(|c| (c.white_king | c.black_king) & (1u64 << king_to) != 0)
            .expect("castling should be legal here");
        assert_eq!(
            castled.piece_square[rook_home],
            ColoredPiece::Empty,
            "square {rook_home} still occupied after castling in {fen}"
        );
        assert_consistent(&castled, "castling");
    }
}

/// All four promotion pieces must be reachable on a capture; promoting to a
/// bishop while capturing used to corrupt the board.
#[test]
fn capture_promotions_produce_all_four_pieces() {
    let cb = Chessboard::from_fen("r1r5/1P6/8/8/8/8/8/4K2k w - - 0 1").unwrap();
    let mut promoted = vec![];
    for m in legal_moves(&cb) {
        let c = m.chessboard;
        assert_consistent(&c, "capture promotion");
        // The pawn left b7; whatever it became is on a8 or c8.
        for sq in [56usize, 58] {
            if c.white_pawns & (1u64 << 49) == 0 && c.piece_square[sq] != cb.piece_square[sq] {
                promoted.push(c.piece_square[sq]);
            }
        }
    }
    for wanted in [
        ColoredPiece::WhiteQueen,
        ColoredPiece::WhiteRook,
        ColoredPiece::WhiteBishop,
        ColoredPiece::WhiteKnight,
    ] {
        assert!(promoted.contains(&wanted), "no capture-promotion to {wanted:?}");
    }
}

// --- notation round trip ----------------------------------------------------

/// Every legal move must survive being written as UCI and read back: describe
/// it, parse it, apply it, and land on exactly the same position. This is what
/// the UCI layer relies on, and it is where castling (two pieces move), en
/// passant (a piece vanishes off the destination square) and promotion (the
/// arriving piece is not the one that left) all get exercised at once.
#[test]
fn describe_and_parse_round_trip() {
    use chess_engine::display::to_fen;
    use chess_engine::notation::{describe_move, parse_move};

    let positions = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        // Castling both ways, both colours.
        "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
        "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1",
        // Promotions, including capture-promotions.
        "r1r5/1P6/8/8/8/8/8/4K2k w - - 0 1",
        "4k3/8/8/8/8/8/1p6/R1R1K3 b - - 0 1",
        // En passant available.
        "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
        // Dense middlegame.
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    ];

    let mut checked = 0;
    for fen in positions {
        let cb = Chessboard::from_fen(fen).unwrap();
        for m in legal_moves(&cb) {
            let text = describe_move(&cb, &m.chessboard)
                .unwrap_or_else(|| panic!("could not describe a move in {fen}"));
            let parsed = parse_move(&text)
                .unwrap_or_else(|e| panic!("`{text}` from {fen} did not parse back: {e}"));
            let replayed = cb
                .make_move(parsed.from, parsed.to, parsed.promotion)
                .unwrap_or_else(|e| panic!("`{text}` from {fen} was rejected: {e}"));
            assert_eq!(
                to_fen(&replayed),
                to_fen(&m.chessboard),
                "`{text}` from {fen} replayed to a different position"
            );
            checked += 1;
        }
    }
    println!("round-tripped {checked} moves");
    assert!(checked > 150, "expected a broad sample of moves, only saw {checked}");
}

/// `make_move` must play the move it was asked for, or reject it. Each branch
/// of the pawn validator has to confirm the destination matches: without that,
/// whichever branch comes last answers every request that reaches it.
#[test]
fn make_move_rejects_moves_it_cannot_play() {
    // Blocked push while en passant happens to be available. e5e6 is illegal
    // (a knight is on e6); it must not silently become exd6 e.p.
    let cb = Chessboard::from_fen(
        "rnbqkbnr/ppp1pppp/4n3/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3").unwrap();
    assert!(cb.make_move(36, 44, None).is_err(), "blocked e5e6 must be rejected");
    // The real en passant capture on the same position is still legal.
    let ep = cb.make_move(36, 43, None).expect("e5d6 e.p. should be legal");
    assert_eq!(ep.piece_square[35], ColoredPiece::Empty, "captured pawn should be gone");

    // Same shape for black: d4d3 is blocked, en passant is available on e3.
    let cb = Chessboard::from_fen(
        "rnbqkbnr/pppp1ppp/8/8/3pP3/3N4/PPPP1PPP/R1BQKBNR b KQkq e3 0 3").unwrap();
    assert!(cb.make_move(27, 19, None).is_err(), "blocked d4d3 must be rejected");
    assert!(cb.make_move(27, 20, None).is_ok(), "d4e3 e.p. should be legal");
}

// --- draw detection ---------------------------------------------------------

/// A repetition must score as a draw, so a side that is winning does not walk
/// into one and a side that is losing can steer towards it.
#[test]
fn repetition_scores_as_a_draw() {
    use chess_engine::notation::parse_move;

    // Black is a queen up and completely winning. If White shuffles its rook,
    // Black repeating would throw the win away.
    let start = Chessboard::from_fen("6k1/5ppp/8/8/8/8/q4PPP/4R1K1 w - - 10 40").unwrap();

    // Build the history of a shuffle that has already happened once:
    // Re1-e2 Qa2-a1 Re2-e1 Qa1-a2 returns to `start` with White to move.
    let mut history = History::new();
    let mut board = start;
    for uci in ["e1e2", "a2a1", "e2e1", "a1a2"] {
        let m = parse_move(uci).unwrap();
        history.push(&board);
        board = board.make_move(m.from, m.to, m.promotion).unwrap();
    }

    // `board` is now the same position as `start`, seen a second time.
    assert_eq!(
        chess_engine::zobrist::hash(&board),
        chess_engine::zobrist::hash(&start),
        "the shuffle should return to the same position"
    );
    assert!(history.repeats(&board), "the repetition should be detected");
}

/// The fifty-move rule has to be a draw, but delivering mate on the hundredth
/// half-move still wins.
#[test]
fn fifty_move_rule_is_a_draw_but_mate_still_wins() {
    // Black is a queen up, but the halfmove clock has run out.
    let drawn = Chessboard::from_fen("6k1/5ppp/8/8/8/8/q4PPP/4R1K1 w - - 100 80").unwrap();
    let (score, _) = nega_max_alpha_beta_best_move(&drawn, 2, true, A, B, &mut History::new());
    assert_eq!(score, 0, "fifty-move rule should be a draw, not a loss");

    // Same clock, but White is being mated: the mate takes priority.
    let mated = Chessboard::from_fen(
        "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 100 80").unwrap();
    assert!(legal_moves(&mated).is_empty(), "should be checkmate");
    let (score, _) = nega_max_alpha_beta_best_move(&mated, 2, true, A, B, &mut History::new());
    assert!(score < -(1 << 19), "checkmate must outrank the fifty-move draw, got {score}");
}

/// A position that has not occurred before is not a repetition, and a short
/// halfmove clock rules one out entirely.
#[test]
fn fresh_positions_are_not_repetitions() {
    let cb = Chessboard::new_initial_board();
    let mut history = History::new();
    history.push(&cb);
    assert!(!history.repeats(&cb), "halfmove clock 0 cannot be a repetition");
}

/// Killer moves are an ordering heuristic, so they must not change what the
/// search concludes -- only how fast it gets there. A wrong killer bonus that
/// leaked into scoring would show up here.
#[test]
fn move_ordering_does_not_change_the_result() {
    // Each of these has an unambiguous best move at this depth.
    for (fen, expected_move) in [
        ("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1", "a1a8"),          // mate in 1
        ("r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 4 4", "f3f7"),
    ] {
        let cb = Chessboard::from_fen(fen).unwrap();
        // The same position searched twice must agree with itself, including
        // once the killer table has been warmed by a previous search.
        let mut state = History::new();
        let (first, first_pos) = nega_max_alpha_beta_best_move(&cb, 4, true, A, B, &mut state);
        let (second, second_pos) = nega_max_alpha_beta_best_move(&cb, 4, true, A, B, &mut state);
        assert_eq!(first, second, "score changed on a re-search of {fen}");
        assert_eq!(
            chess_engine::notation::describe_move(&cb, &first_pos),
            chess_engine::notation::describe_move(&cb, &second_pos),
            "best move changed on a re-search of {fen}"
        );
        assert_eq!(
            chess_engine::notation::describe_move(&cb, &first_pos).as_deref(),
            Some(expected_move),
            "wrong best move in {fen}"
        );
    }
}

/// The table must not break tactics: where there is one clearly best move, it
/// still has to be found, and a mate still has to be seen.
///
/// Note what is deliberately NOT asserted: that scores are bit-identical with
/// and without the table. They are not always, and that is a property of the
/// search rather than a bug in the table -- quiescence returns bounds rather
/// than exact values, so a node above it can cache a score that was only exact
/// for the window it was searched with. Every engine with a table and a
/// quiescence search has this. It shows up as an occasional different-but-
/// equal-valued move, not as a wrong move.
#[test]
fn transposition_table_preserves_tactics() {
    // Some positions have several equally best moves -- the two rooks below are
    // symmetric -- so each case lists every acceptable answer.
    for (fen, acceptable) in [
        ("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1", &["a1a8"][..]), // mate in 1
        ("r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 4 4", &["f3f7"][..]),
        // Capture-promotion: either rook, both winning a rook and queening.
        ("r1r5/1P6/8/8/8/8/8/4K2k w - - 0 1", &["b7a8q", "b7c8q"][..]),
    ] {
        let cb = Chessboard::from_fen(fen).unwrap();
        let mut state = History::new();
        state.ensure_table(16);
        let (_, pos) = nega_max_alpha_beta_best_move(&cb, 5, true, A, B, &mut state);
        let played = chess_engine::notation::describe_move(&cb, &pos).unwrap();
        assert!(
            acceptable.contains(&played.as_str()),
            "table lost the best move in {fen}: played {played}, expected one of {acceptable:?}"
        );
    }
}

/// Entries have to survive between the iterations of a deepening search -- that
/// is the whole point -- so a warm table must still give the same answer.
#[test]
fn warm_transposition_table_is_still_correct() {
    let cb = Chessboard::from_fen(
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10").unwrap();
    let mut state = History::new();
    state.ensure_table(16);

    // Deepen the way the UCI driver does, then confirm the deepest answer
    // matches a cold search to the same depth.
    let mut warm = 0;
    for depth in 1..=5 {
        warm = nega_max_alpha_beta_best_move(&cb, depth, true, A, B, &mut state).0;
    }
    let cold = nega_max_alpha_beta_best_move(&cb, 5, true, A, B, &mut History::new()).0;
    assert_eq!(warm, cold, "a warmed table changed the depth-5 score");
}

/// Exhaustive check of the `make_move` path: for every from/to/promotion
/// combination, it must play exactly the move asked for, or refuse.
///
/// `check_and_make_move.rs` is a second, hand-maintained copy of the move
/// generator with "is this the requested destination?" filters woven in. Two
/// shipped bugs were a branch that forgot its filter, so the branch answered
/// every request that reached it: a capture-promotion returned a quiet
/// promotion on the wrong square, and en passant hijacked a blocked push.
/// Rather than test those two branches, this walks every branch at once.
#[test]
fn make_move_plays_exactly_what_was_asked() {
    use chess_engine::notation::describe_move;
    use chess_engine::piece::PromotionPiece;
    use std::collections::HashSet;

    let positions = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
        "rnbqkbnr/ppp1pppp/4n3/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3",
        "r1r5/1P6/8/8/8/8/8/4K2k w - - 0 1",
        "4k3/8/8/8/8/8/1p6/R1R1K3 b - - 0 1",
        "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
    ];
    let promotions = [
        None,
        Some(PromotionPiece::Queen),
        Some(PromotionPiece::Rook),
        Some(PromotionPiece::Bishop),
        Some(PromotionPiece::Knight),
    ];

    for fen in positions {
        let cb = Chessboard::from_fen(fen).unwrap();
        let legal: HashSet<String> = legal_moves(&cb)
            .iter()
            .filter_map(|m| describe_move(&cb, &m.chessboard))
            .collect();

        for from in 0..64usize {
            for to in 0..64usize {
                for promotion in promotions {
                    let mut asked = format!(
                        "{}{}{}{}",
                        (b'a' + (from % 8) as u8) as char,
                        from / 8 + 1,
                        (b'a' + (to % 8) as u8) as char,
                        to / 8 + 1
                    );
                    if let Some(p) = promotion {
                        asked.push(match p {
                            PromotionPiece::Queen => 'q',
                            PromotionPiece::Rook => 'r',
                            PromotionPiece::Bishop => 'b',
                            PromotionPiece::Knight => 'n',
                        });
                    }

                    match cb.make_move(from, to, promotion) {
                        Ok(after) => {
                            let played = describe_move(&cb, &after).unwrap_or_default();
                            // A promotion suffix on a move that does not promote
                            // is meaningless, so only the squares are compared there.
                            let ok = if played.len() == 5 || asked.len() == 4 {
                                played == asked
                            } else {
                                played == asked[..4]
                            };
                            assert!(ok, "asked {asked} in {fen}, played {played}");
                        }
                        Err(_) => assert!(
                            !legal.contains(&asked),
                            "{asked} is legal in {fen} but make_move refused it"
                        ),
                    }
                }
            }
        }
    }
}

/// Null-move pruning assumes having the move is an advantage. In a king-and-pawn
/// endgame that is false -- in zugzwang every move worsens the position -- so
/// the heuristic has to be switched off when only pawns remain, or the search
/// will prune away the very lines that decide the game.
#[test]
fn null_move_is_disabled_without_pieces() {
    // Classic opposition: white to move draws, black to move loses. If null
    // move were applied here the search would conclude the position is winning
    // for whoever is not to move, which is exactly backwards.
    let cb = Chessboard::from_fen("8/8/8/4k3/8/4K3/4P3/8 w - - 0 1").unwrap();
    let mut state = History::new();
    state.ensure_table(16);
    let (score, _) = nega_max_alpha_beta_best_move(&cb, 6, true, A, B, &mut state);
    // White is a pawn up but the black king holds the opposition; the score
    // should be modest, not a runaway win from a bad null-move cutoff.
    assert!(
        score.abs() < 500,
        "king-and-pawn endgame scored {score}; null move likely fired in zugzwang"
    );

    // And a mate must still be found in a pawnless position, where null move is
    // also disabled.
    let mate = Chessboard::from_fen("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1").unwrap();
    let (score, pos) = nega_max_alpha_beta_best_move(&mate, 4, true, A, B, &mut History::new());
    assert!(score > (1 << 19), "should still see the mate, scored {score}");
    assert_eq!(
        chess_engine::notation::describe_move(&mate, &pos).as_deref(),
        Some("a1a8")
    );
}

/// Evaluation must be symmetric: mirror the position top-to-bottom, swap the
/// colours, and the score must negate exactly. Any term that treats white and
/// black differently -- a wrong rank index, a mask built for one direction --
/// shows up here as an asymmetry, and would make the engine play one colour
/// worse than the other.
#[test]
fn evaluation_is_colour_symmetric() {
    use chess_engine::eval::evaluate;

    fn mirror(fen: &str) -> String {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        let ranks: Vec<String> = parts[0]
            .split('/')
            .rev()                       // flip the board vertically
            .map(|r| r.chars().map(|c| {
                if c.is_ascii_uppercase() { c.to_ascii_lowercase() }
                else if c.is_ascii_lowercase() { c.to_ascii_uppercase() }
                else { c }
            }).collect())
            .collect();
        let side = if parts[1] == "w" { "b" } else { "w" };
        let castling: String = if parts[2] == "-" { "-".into() } else {
            parts[2].chars().map(|c| {
                if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() }
            }).collect()
        };
        format!("{} {} {} - 0 1", ranks.join("/"), side, castling)
    }

    for fen in [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        // Passed pawns on both wings.
        "8/1P6/8/8/8/8/6p1/4K2k w - - 0 1",
        // Broken king shield.
        "r1bq1rk1/pp3ppp/2n5/8/8/2N5/PP3PPP/R1BQ1RK1 w - - 0 1",
        // Knights on the second and seventh ranks, where the knight table once
        // counted each knight's own square for white alone.
        "r1bqkb1r/pppnnppp/8/8/8/8/PPPNNPPP/R1BQKB1R w KQkq - 0 1",
        // Advanced passed pawns with kings and pieces around them (gCd8UcfI,
        // 2qYroOWA), which is what `passed_pawn_pieces` reads.
        "8/1P1Pk3/2n4p/2p5/2P2p2/K2N4/8/8 w - - 1 56",
        "1Q6/3Pqpk1/6p1/8/7p/2P4P/3b1PP1/5K2 w - - 1 58",
        // From gCd8UcfI: mirrored, black's knight lands on d2.
        "5R2/3N4/1P1P2pp/2p3k1/K1Pn1p2/1r6/8/8 w - - 2 53",
    ] {
        let a = Chessboard::from_fen(fen).unwrap();
        let m = mirror(fen);
        let b = Chessboard::from_fen(&m)
            .unwrap_or_else(|e| panic!("mirrored fen {m} rejected: {e}"));
        assert_eq!(
            evaluate(&a), -evaluate(&b),
            "asymmetric evaluation:\n  {fen} -> {}\n  {m} -> {}",
            evaluate(&a), evaluate(&b)
        );
    }
}

/// A passed pawn whose queening square the enemy controls is worth less than
/// the same pawn with a free path. The piece-square tables cannot tell them
/// apart, and that threw away gCd8UcfI: the engine sacrificed a rook into an
/// endgame it scored +394 for two pawns on the seventh that a knight had
/// stopped. Stockfish scores it 0.00.
///
/// Measured as a difference of differences so that nothing but the passed-pawn
/// term can move it: the knight moves between a5 and c6 (covering d8 or not)
/// with the pawn on d7, and again with the pawn on d3, where the term does not
/// apply. The tables, mobility and the pawn hash cancel.
#[test]
fn a_stopped_passed_pawn_is_worth_less_than_a_free_one() {
    use chess_engine::eval::evaluate;
    let e = |fen: &str| evaluate(&Chessboard::from_fen(fen).unwrap());
    let advanced = e("7k/3P4/2n5/8/8/8/8/K7 w - - 0 1") - e("7k/3P4/8/n7/8/8/8/K7 w - - 0 1");
    let behind = e("7k/8/2n5/8/8/3P4/8/K7 w - - 0 1") - e("7k/8/8/n7/8/3P4/8/K7 w - - 0 1");
    assert!(
        advanced - behind <= -50,
        "covering the queening square of a pawn on the seventh moved it by only {}cp",
        advanced - behind
    );
}

/// The principal variation must be a real, playable line -- every move legal in
/// the position before it. It is walked out of the transposition table by
/// matching stored move keys against generated moves, so a mismatch would
/// produce a line that cannot actually be played.
#[test]
fn principal_variation_is_playable() {
    use chess_engine::search::principal_variation;

    for fen in [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    ] {
        let cb = Chessboard::from_fen(fen).unwrap();
        let mut state = History::new();
        state.ensure_table(16);
        nega_max_alpha_beta_best_move(&cb, 6, cb.side_to_move == Color::White, A, B, &mut state);

        let line = principal_variation(&cb, &state.table, 12);
        assert!(!line.is_empty(), "no principal variation for {fen}");

        // Every step must be reachable by a legal move from the one before it.
        let mut from = cb;
        for (i, position) in line.iter().enumerate() {
            let reachable = legal_moves(&from)
                .into_iter()
                .any(|m| to_fen(&m.chessboard) == to_fen(position));
            assert!(reachable, "pv move {} is not legal in {fen}", i + 1);
            from = *position;
        }
        // Alternating sides, as a real game must.
        assert_ne!(line[0].side_to_move, cb.side_to_move, "side did not alternate");
    }
}

/// Static exchange evaluation must correctly value a capture sequence, so the
/// search can tell a winning capture from one that hangs a piece.
#[test]
fn static_exchange_evaluation_is_correct() {
    use chess_engine::see::see;

    // (fen, from, to, expected, description)
    let cases: &[(&str, usize, usize, i32, &str)] = &[
        // Rook takes an undefended pawn on e5: wins a clean pawn.
        ("4k3/8/8/4p3/8/8/8/4R1K1 w - - 0 1", 4, 36, 100, "Rxe5, pawn is free"),
        // Same, but the pawn is defended by a pawn on f6: rook for pawn, losing.
        ("4k3/8/5p2/4p3/8/8/8/4R1K1 w - - 0 1", 4, 36, 100 - 500, "Rxe5 loses the rook"),
        // Queen takes a pawn defended by a pawn: queen for pawn.
        ("4k3/8/5p2/4p3/8/8/8/3QK3 w - - 0 1", 3, 36, 100 - 900, "Qxe5 loses the queen"),
        // Equal trade: rook takes rook, recaptured by a rook.
        ("4k3/4r3/8/4r3/8/8/4R3/4K3 w - - 0 1", 12, 36, 0, "Rxe5 Rxe5, even"),
        // Undefended piece is simply won.
        ("4k3/8/8/4n3/8/8/8/4R1K1 w - - 0 1", 4, 36, 320, "Rxe5 wins a knight"),
        // The same, on e2. The knight table's c2..h2 entries once included the
        // knight's own square, so a knight there counted as defending itself and
        // this capture read as bishop-for-knight.
        ("4k3/8/8/7b/8/8/4N3/K7 b - - 0 1", 39, 12, 320, "Bxe2 wins a knight"),
    ];

    for (fen, from, to, expected, what) in cases {
        let cb = Chessboard::from_fen(fen).unwrap();
        let got = see(&cb, *from, *to);
        assert_eq!(got, *expected, "{what} in {fen}: see said {got}, expected {expected}");
    }
}

/// Every knight attack set must be exactly the up-to-eight squares a knight
/// reaches, and never its own square. Move generation masks own-occupied
/// squares, so perft passed for years with c2..h2 each including itself; the
/// extra square leaked into mobility (a white knight on the second rank earned
/// one square more than a black knight on the seventh) and into SEE, where the
/// knight defended itself.
#[test]
fn knight_attack_table_is_exact() {
    use chess_engine::move_gen::move_gen_knight::knight_attacks_from_single_knight_bitboard;
    for square in 0..64i32 {
        let (file, rank) = (square % 8, square / 8);
        let mut expected = 0u64;
        for (df, dr) in [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)] {
            let (f, r) = (file + df, rank + dr);
            if (0..8).contains(&f) && (0..8).contains(&r) {
                expected |= 1u64 << (r * 8 + f);
            }
        }
        assert_eq!(
            knight_attacks_from_single_knight_bitboard(1u64 << square),
            expected,
            "knight attacks from square {square}"
        );
    }
}

/// The capture generator must produce everything the full generator produces
/// that quiescence would keep.
///
/// `*_captures_into` duplicates `*_legal_moves_into` with the target mask
/// narrowed, and the pawn capture generator duplicates the capture, promotion
/// and en passant branches outright. When the two drift, quiescence searches
/// the wrong set of moves and perft cannot see it, because perft never calls
/// the capture path. A first cut of that split dropped the quiet promotions --
/// all four score 6-9, which is at or above the threshold `quiescence_search`
/// retains -- and lost a mate.
///
/// The legality masks are the same hazard again: they are applied in both
/// generators, and masking one and not the other would show up here and
/// nowhere else.
#[test]
fn capture_generator_keeps_everything_quiescence_wants() {
    use chess_engine::move_gen::{
        black_captures_into, black_legal_moves_into, white_captures_into, white_legal_moves_into,
    };
    use chess_engine::zobrist;

    /// The score at or above which `quiescence_search_best_move` retains a move.
    const QUIESCENCE_THRESHOLD: u32 = 6;

    const POSITIONS: &[(&str, &str)] = &[
        ("initial", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("kiwipete", "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"),
        ("kiwipete black", "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1"),
        ("endgame", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
        ("promotions", "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"),
        ("position 6 black", "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 b - - 0 10"),
        ("position 5", "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"),
        ("position 6", "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10"),
        // In check: the masks narrow to the checker and the squares before it.
        ("in check", "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3"),
        // Pinned pieces, including a pinned pawn that may still capture.
        ("pinned", "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1"),
        // A free promotion push for each side. All four pieces score 6-9, so
        // quiescence keeps them and the capture generator has to produce them;
        // the first cut of that generator did not, and lost a mate.
        ("white promotion push", "7k/P7/8/8/8/8/8/K7 w - - 0 1"),
        ("black promotion push", "k7/8/8/8/8/8/p7/7K b - - 0 1"),
        // A pinned pawn with a capture that the pin forbids. Masking this in
        // one generator and not the other puts an illegal move in quiescence.
        ("pinned pawn capture", "4r2k/8/8/8/8/3n4/4P3/4K3 w - - 0 1"),
        ("pinned pawn capture black", "4k3/4p3/3N4/8/8/8/8/4R2K b - - 0 1"),
        // En passant, which is the one move still decided by make-and-test.
        ("en passant", "8/8/3p4/KPp4r/1R3p1k/8/4P1P1/8 w - c6 0 3"),
        ("en passant pin", "8/8/8/2KPp2r/8/8/8/4k3 w - e6 0 1"),
    ];

    for (name, fen) in POSITIONS {
        let cb = Chessboard::from_fen(fen).unwrap_or_else(|e| panic!("{name}: {e}"));
        let white = cb.side_to_move == Color::White;

        let mut full = Vec::new();
        let mut captures = Vec::new();
        if white {
            white_legal_moves_into(&cb, &mut full);
            white_captures_into(&cb, &mut captures);
        } else {
            black_legal_moves_into(&cb, &mut full);
            black_captures_into(&cb, &mut captures);
        }

        let key = |m: &chess_engine::move_list::Move| (zobrist::hash(&m.chessboard), m.score);
        let in_captures: Vec<_> = captures.iter().map(key).collect();
        let in_full: Vec<_> = full.iter().map(key).collect();

        for m in full.iter().filter(|m| m.score >= QUIESCENCE_THRESHOLD) {
            assert!(
                in_captures.contains(&key(m)),
                "{name}: the capture generator dropped a move scoring {} that quiescence keeps",
                m.score
            );
        }
        for m in &captures {
            assert!(
                in_full.contains(&key(m)),
                "{name}: the capture generator invented a move the full generator does not have"
            );
        }
    }
}

/// The two stages must partition the full generator exactly: every legal move
/// in one of them, no move in both.
///
/// Staged generation searches the noisy moves first and only builds the quiet
/// ones if nothing cut. That is only sound if the two sets add up. A move in
/// neither is a move the search cannot find -- a missed mate, or a stalemate
/// scored as a position. A move in both is searched twice, which is slower and
/// double-counts its history credit.
///
/// Perft cannot see any of this: it calls neither stage. This is the same
/// hazard as `capture_generator_keeps_everything_quiescence_wants`, one level
/// further on.
#[test]
fn capture_and_quiet_stages_partition_the_full_generator() {
    use chess_engine::move_gen::legality::{black_legality, white_legality};
    use chess_engine::move_gen::{
        black_captures_with, black_legal_moves_with, black_quiets_with, white_captures_with,
        white_legal_moves_with, white_quiets_with,
    };
    use chess_engine::zobrist;
    use std::collections::HashSet;

    const POSITIONS: &[(&str, &str)] = &[
        ("initial", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("kiwipete", "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"),
        ("kiwipete black", "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1"),
        ("endgame", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
        ("promotions", "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"),
        ("position 5", "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"),
        ("position 6", "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10"),
        ("position 6 black", "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 b - - 0 10"),
        ("in check", "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3"),
        ("pinned", "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1"),
        ("white promotion push", "7k/P7/8/8/8/8/8/K7 w - - 0 1"),
        ("black promotion push", "k7/8/8/8/8/8/p7/7K b - - 0 1"),
        ("castling both sides", "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1"),
        ("castling black", "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1"),
        ("en passant", "8/8/3p4/KPp4r/1R3p1k/8/4P1P1/8 w - c6 0 3"),
        ("double check", "3rkr2/8/8/8/8/8/4N3/4K3 b - - 0 1"),
    ];

    for (name, fen) in POSITIONS {
        let cb = Chessboard::from_fen(fen).unwrap_or_else(|e| panic!("{name}: {e}"));
        let white = cb.side_to_move == Color::White;

        let (mut full, mut captures, mut quiets) = (Vec::new(), Vec::new(), Vec::new());
        if white {
            let legality = white_legality(&cb);
            white_legal_moves_with(&cb, &mut full, &legality);
            white_captures_with(&cb, &mut captures, &legality);
            white_quiets_with(&cb, &mut quiets, &legality);
        } else {
            let legality = black_legality(&cb);
            black_legal_moves_with(&cb, &mut full, &legality);
            black_captures_with(&cb, &mut captures, &legality);
            black_quiets_with(&cb, &mut quiets, &legality);
        }

        let keys = |ms: &[chess_engine::move_list::Move]| -> Vec<u64> {
            ms.iter().map(|m| zobrist::hash(&m.chessboard)).collect()
        };
        let (full_keys, capture_keys, quiet_keys) = (keys(&full), keys(&captures), keys(&quiets));

        // Neither stage may produce the same move twice.
        assert_eq!(
            capture_keys.len(),
            capture_keys.iter().collect::<HashSet<_>>().len(),
            "{name}: the noisy stage produced a duplicate"
        );
        assert_eq!(
            quiet_keys.len(),
            quiet_keys.iter().collect::<HashSet<_>>().len(),
            "{name}: the quiet stage produced a duplicate"
        );

        // Disjoint.
        let noisy: HashSet<_> = capture_keys.iter().copied().collect();
        for (key, m) in quiet_keys.iter().zip(&quiets) {
            assert!(
                !noisy.contains(key),
                "{name}: a move scoring {} is in both stages and would be searched twice",
                m.score
            );
        }

        // And together exactly the full generator.
        let staged: HashSet<_> = capture_keys.iter().chain(&quiet_keys).copied().collect();
        let whole: HashSet<_> = full_keys.iter().copied().collect();
        assert_eq!(whole.len(), full_keys.len(), "{name}: the full generator produced a duplicate");
        for (key, m) in full_keys.iter().zip(&full) {
            assert!(
                staged.contains(key),
                "{name}: neither stage produces a legal move scoring {} -- the search cannot find it",
                m.score
            );
        }
        assert_eq!(
            staged.len(),
            whole.len(),
            "{name}: the stages produce {} moves against the full generator's {}",
            staged.len(),
            whole.len()
        );
    }
}
