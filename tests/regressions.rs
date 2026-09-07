use chess_engine::chessboard::Chessboard;
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
