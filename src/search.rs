use crate::{chessboard::Chessboard, eval::evaluate, move_gen::{black_legal_moves, white_legal_moves}};

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
