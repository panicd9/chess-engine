use chess_engine::chessboard::Chessboard;
use chess_engine::display::to_fen;
use chess_engine::move_gen::legal_moves;
use chess_engine::search::nega_max_alpha_beta_best_move;

const A: i32 = i32::MIN + 1;
const B: i32 = i32::MAX - 1;

#[test]
fn stalemate_is_a_draw() {
    let cb = Chessboard::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
    assert!(legal_moves(&cb).is_empty());
    let (score, _) = nega_max_alpha_beta_best_move(&cb, 1, false, A, B);
    println!("stalemate scores {score}");
    assert_eq!(score, 0);
}

#[test]
fn prefers_mate_over_stalemate() {
    let cb = Chessboard::from_fen("7k/8/6K1/8/8/8/5Q2/8 w - - 0 1").unwrap();
    for depth in 2..=4u32 {
        let (score, next) = nega_max_alpha_beta_best_move(&cb, depth, true, A, B);
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
    let (score, next) = nega_max_alpha_beta_best_move(&cb, 3, true, A, B);
    println!("mate search: {} score {score}", to_fen(&next));
    assert!(legal_moves(&next).is_empty() && next.is_black_king_under_attack(), "should be mate");
}

/// Quiescence is entered at depth 0 and recursed with `depth - 1` on a u32,
/// which panicked in any build with overflow checks on (i.e. every debug build).
#[test]
fn quiescence_depth_does_not_underflow() {
    let cb = Chessboard::from_fen(
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10").unwrap();
    let (score, _) = nega_max_alpha_beta_best_move(&cb, 3, true, A, B);
    println!("search completed, score {score}");
}
