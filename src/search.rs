use crate::{chessboard::Chessboard, eval::evaluate, move_gen::{black_legal_moves, white_legal_moves}};

pub fn nega_max_alpha_beta(cb: &Chessboard, depth: u32, is_white_turn: bool, alpha: i64, beta: i64) -> (i64, Chessboard) {
    if depth == 0 {
        let score = evaluate(cb); // No need to consider side to move here
        return (if is_white_turn { score } else { -score }, cb.clone()); // Negate if it's black's turn
    }

    let legal_positions = if is_white_turn {
        white_legal_moves(cb)
    } else {
        black_legal_moves(cb)
    };

    let mut max: i64 = i64::MIN;
    let mut best_move = *cb;
    let mut alpha = alpha;
    let mut beta = beta;

    for pos in legal_positions {
        let (child_score, _) = nega_max_alpha_beta(&pos, depth - 1, !is_white_turn, alpha, beta);

        // Check if negating would overflow (i64::MIN can't be negated)
        let score = if child_score == i64::MIN {
            i64::MAX // Return the maximum possible value to handle the overflow
        } else {
            -child_score // Otherwise, negate the score normally
        };

        if score > max {
            max = score;
            best_move = pos; // Update the best move if this score is higher
        }

        // Alpha-beta pruning
        if max >= beta {
            break; // Beta cut-off
        }

        alpha = alpha.max(max);
    }

    return (max, best_move);
}

pub fn nega_max(cb: &Chessboard, depth: u32, is_white_turn: bool) -> i64 {
    if depth == 0 {
        return evaluate(cb);
    }

    let legal_positions = if is_white_turn {
        white_legal_moves(cb)
    } else {
        black_legal_moves(cb)
    };
    let mut max: i64 = i64::MIN;

    for pos in legal_positions {
        let child_score = nega_max(&pos, depth - 1, !is_white_turn);

        // Check if negating would overflow (i64::MIN can't be negated)
        let score = if child_score == i64::MIN {
            i64::MAX // Return the maximum possible value to handle the overflow
        } else {
            -child_score // Otherwise, negate the score normally
        };

        if score > max {
            max = score;
        }
    }

    return max;
}

mod test {
    use std::time::Instant;

    use crate::chessboard::Chessboard;

    use super::nega_max;

    #[test]
    fn test_negamax() {
        let cb = Chessboard::new_initial_board();

        let start = Instant::now();
        let result = nega_max(&cb, 5, true);
        let duration = start.elapsed();

        println!("Result: {}", result);
        println!("Time taken: {:?}", duration);
    }
}
