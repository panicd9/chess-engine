use crate::{
    chessboard::Chessboard,
    display::display_board_string,
    eval::evaluate,
    move_gen::{black_legal_moves, legal_moves, white_legal_moves},
    move_list::MoveList,
};

pub fn nega_max_alpha_beta_best_move(
    cb: &Chessboard,
    depth: u32,
    is_white_turn: bool,
    mut alpha: i32,
    beta: i32,
) -> (i32, Chessboard) {
    // Base case: evaluate the board when depth is 0.
    if depth == 0 {
        // Directly return the evaluation and a clone of the board.
        return quiescence_search_best_move(cb, is_white_turn, depth, alpha, beta);
        // let eval = if is_white_turn { evaluate(cb) } else { -evaluate(cb) };
        // return (eval, cb.clone());
    }

    // We'll track the best score and best resulting position.
    let mut best_score = i32::MIN;
    let mut best_pos: Chessboard = *cb;

    // Generate the legal moves for the current side.
    // (Assuming that white_legal_moves/black_legal_moves returns an iterator or slice.)
    let mut legal_moves = if is_white_turn {
        MoveList::new(white_legal_moves(cb))
    } else {
        MoveList::new(black_legal_moves(cb))
    };

    if legal_moves.moves.is_empty() {
        let depth_adj = depth as i32;
        const OVERFLOW_PROTECTOR: i32 = 1000;
        let checkmate_score = if is_white_turn {
            i32::MIN + (OVERFLOW_PROTECTOR - depth_adj)
        } else {
            // i32::MAX - (OVERFLOW_PROTECTOR - depth_adj)
            i32::MIN + (OVERFLOW_PROTECTOR - depth_adj)
        };
        // let checkmate_score = if is_white_turn {
        //     i32::MAX - 100
        // } else {
        //     i32::MIN + 100
        // };
        return (checkmate_score, *cb);
    }

    // Iterate over moves.
    'legal_moves: while let Some(next_move) = legal_moves.next_move() {
        // Extract the new position from the move.
        let pos = next_move.chessboard;
        // Negamax: invert alpha and beta for the recursive call.
        let safe_beta = safe_neg(beta);
        let safe_alpha = safe_neg(alpha);

        // Recursively search the subtree.
        let child_result = nega_max_alpha_beta_best_move(&pos, depth - 1, !is_white_turn, safe_beta, safe_alpha);

        // Determine the score for this move.
        let score = -child_result.0; // Negate the child's score.

        // If this move is better, update the best score and best position.
        if score > best_score {
            best_score = score;
            best_pos = pos;
        }

        // Update alpha and do a beta cutoff if possible.
        alpha = alpha.max(score);
        if alpha >= beta {
            break 'legal_moves; // Beta cutoff.
        }
    }

    // Return the best move (if any).
    (best_score, best_pos)
}

pub fn nega_max_alpha_beta_best_line(
    cb: &Chessboard,
    depth: u32,
    is_white_turn: bool,
    mut alpha: i32,
    beta: i32,
) -> Vec<(i32, Vec<Chessboard>)> {
    // Base case: Evaluate and return a single-line variation.
    if depth == 0 {
        let score: i32 = evaluate(cb);
        // let final_score = if is_white_turn { score } else { -score };
        // return vec![(final_score, vec![cb.clone()])]; // No negation here
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
            !is_white_turn,
            safe_beta,
            safe_alpha,
        );
        if child_results.is_empty() {
            let depth_adj = depth as i32;
            const OVERFLOW_PROTECTOR: i32 = 1000;
            let checkmate_score = if is_white_turn {
                i32::MIN + (OVERFLOW_PROTECTOR - depth_adj)
            } else {
                i32::MAX - (OVERFLOW_PROTECTOR - depth_adj)
            };
            // let checkmate_score = if is_white_turn { i64::MAX  } else { i64::MIN  };
            return vec![(checkmate_score, vec![cb.clone(), pos])]; // Return an empty vector if no moves are found
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
    depth: u32,
    mut alpha: i32,
    beta: i32,
) -> (i32, Chessboard) {
    // Do a static evaluation of the current (quiet) position.
    // For black, invert the evaluation to maintain the negamax framework.
    let stand_pat = if is_white_turn { evaluate(cb) } else { -evaluate(cb) };
    let mut best_score = stand_pat;
    // Default board is the current board (used if no move improves the evaluation)
    let mut best_board = cb.clone();

    // Fail-hard beta cutoff.
    if best_score >= beta {
        return (beta, best_board);
    }
    if alpha < best_score {
        alpha = best_score;
    }

    let all_moves = if is_white_turn {
        white_legal_moves(cb)
    } else {
        black_legal_moves(cb)
    };

    if all_moves.is_empty() {
        let depth_adj = depth as i32;
        const OVERFLOW_PROTECTOR: i32 = 1000;
        let checkmate_score = if is_white_turn {
            i32::MIN + (OVERFLOW_PROTECTOR - depth_adj)
        } else {
            i32::MIN + (OVERFLOW_PROTECTOR - depth_adj)
            // i32::MAX - (OVERFLOW_PROTECTOR - depth_adj)
        };
        return (checkmate_score, best_board); // Return an empty vector if no moves are found
    }

    // Generate only "noisy" moves (e.g., captures).
    let capture_moves: Vec<_> = all_moves.into_iter().filter(|m| m.score >= 6).collect();

    if capture_moves.is_empty() {
        return (best_score, best_board);
    }

    let mut not_quiet_moves = MoveList::new(capture_moves);

    // Loop through each capture move.
    while let Some(next_move) = not_quiet_moves.next_move() {
        let pos = next_move.chessboard.clone();
        // Swap bounds for the negamax recursion.
        let safe_beta = safe_neg(beta);
        let safe_alpha = safe_neg(alpha);
        // Recurse: the returned board here is from the child's perspective.
        // Since we want the immediate move (next_move.chessboard) at this level,
        // we ignore the child's board state.
        let child_result =
            quiescence_search_best_move(&pos, !is_white_turn, depth - 1, safe_beta, safe_alpha);

        // Invert the child's score (negamax style).
        let score = -child_result.0;


        // Beta cutoff: return immediately with the move that produced this cutoff.
        if score >= beta {
            return (beta, next_move.chessboard);
        }

        // If we find a move that improves alpha, update.
        if score > alpha {
            alpha = score;
            best_score = score;
            best_board = next_move.chessboard;
        }
    }

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


fn safe_neg(value: i32) -> i32 {
    if value == i32::MIN {
        i32::MAX // Return max value instead of overflowing
    } else {
        -value
    }
}

pub fn nega_max_alpha_beta(
    cb: &Chessboard,
    depth: u32,
    alpha: i32,
    beta: i32,
) -> (i32, Chessboard) {
    if depth == 0 {
        // Evaluate from the perspective of the current player
        return (evaluate(cb), cb.clone());
    }

    let legal_positions = legal_moves(cb); // Assume this returns moves for the current player

    let mut max = i32::MIN;
    let mut best_move = cb.clone();
    let mut alpha = alpha;

    for pos in &legal_positions {
        // Note: We pass -beta and -alpha and then negate the returned score.
        let (child_score, _) = nega_max_alpha_beta(&pos.chessboard, depth - 1, -beta, -alpha);
        let score = if child_score == i32::MIN {
            i32::MAX // handle the overflow case
        } else {
            -child_score
        };

        if score > max {
            max = score;
            best_move = pos.chessboard;
        }

        alpha = alpha.max(max);
        if alpha >= beta {
            break; // Beta cut-off
        }
    }

    // if legal_positions.is_empty() {
    //     return (i64::MIN + 1, cb.clone());
    // }
    (max, best_move)
}

// pub fn nega_max_alpha_beta_best_line(
//     cb: &Chessboard,
//     depth: u32,
//     is_white_turn: bool,
//     alpha: i64,
//     beta: i64,
// ) -> (i64, Vec<Chessboard>) {
//     if depth == 0 {
//         let score = evaluate(cb); // Evaluate the position
//         return (
//             if is_white_turn { score } else { -score },
//             vec![cb.clone()], // Return the current position as the "line"
//         );
//     }

//     let legal_positions = if is_white_turn {
//         white_legal_moves(cb)
//     } else {
//         black_legal_moves(cb)
//     };

//     let mut max: i64 = i64::MIN;
//     let mut best_line: Vec<Chessboard> = Vec::new(); // This will hold the best sequence of moves
//     let mut alpha = alpha;
//     let mut beta = beta;

//     for pos in legal_positions {
//         let (child_score, child_line) = nega_max_alpha_beta_best_line(&pos, depth - 1, !is_white_turn, alpha, beta);

//         // Handle potential negation overflow
//         let score = if child_score == i64::MIN {
//             i64::MAX // Avoid overflow
//         } else {
//             -child_score // Negate normally
//         };

//         if score > max {
//             max = score;
//             best_line = vec![cb.clone()]; // Start the best line with the current position
//             best_line.extend(child_line); // Append the child's best line
//         }

//         // Alpha-beta pruning
//         if max >= beta {
//             break; // Beta cut-off
//         }

//         alpha = alpha.max(max);
//     }

//     return (max, best_line);
// }

// pub fn nega_max(cb: &Chessboard, depth: u32, is_white_turn: bool) -> i64 {
//     if depth == 0 {
//         return evaluate(cb);
//     }

//     let legal_positions = if is_white_turn {
//         white_legal_moves(cb)
//     } else {
//         black_legal_moves(cb)
//     };
//     let mut max: i64 = i64::MIN;

//     for pos in legal_positions {
//         let child_score = nega_max(&pos, depth - 1, !is_white_turn);

//         // Check if negating would overflow (i64::MIN can't be negated)
//         let score = if child_score == i64::MIN {
//             i64::MAX // Return the maximum possible value to handle the overflow
//         } else {
//             -child_score // Otherwise, negate the score normally
//         };

//         if score > max {
//             max = score;
//         }
//     }

//     return max;
// }

mod test {
    use std::time::Instant;

    use crate::chessboard::Chessboard;

    // #[test]
    // fn test_negamax() {
    //     let cb = Chessboard::new_initial_board();

    //     let start = Instant::now();
    //     let result = nega_max(&cb, 5, true);
    //     let duration = start.elapsed();

    //     println!("Result: {}", result);
    //     println!("Time taken: {:?}", duration);
    // }
}
