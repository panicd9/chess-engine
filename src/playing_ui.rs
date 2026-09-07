use crate::{
    chessboard::{Chessboard, Color},
    display::display_board,
    iterative_deepening::{
        iterative_deepening_best_5_lines_with_interupt_support, start_key_listener,
    },
    perft::{compare_boards, format_move},
    piece::PromotionPiece,
    search::{nega_max_alpha_beta, nega_max_alpha_beta_best_line, nega_max_alpha_beta_best_move},
};
use crossterm::style::{Attribute, Color as TextColor, ResetColor, SetBackgroundColor, SetForegroundColor};
use std::{
    io::{self, Write},
    sync::{atomic::AtomicBool, Arc},
};

#[derive(Debug)]
pub enum ParseError {
    InvalidSquareFormat,
    InvalidFile,
    InvalidRank,
    InvalidPromotionPiece,
}

pub fn play() {
    const MAX_DEPTH: u32 = 7;

    let mut cb = Chessboard::from_fen("rnbqkb1r/p3pppp/2p2n2/8/PppP4/2N1PN2/1P3PPP/R1BQKB1R w KQkq - 0 7").unwrap();
    display_board(&cb);

    loop {
        println!(
            "\n\n################ Move #{} ################",
            cb.fullmove_counter
        );
        if cb.side_to_move == Color::White {
            println!("White to move!");
        } else {
            println!("Black to move!");
        }

        print!("Enter your move (e.g., e2e4): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.len() != 4 && input.len() != 5 {
            println!("Invalid move format. Please use the format 'e2e4'.");
            continue;
        }

        let from = parse_square(&input[0..2]);
        let to = parse_square(&input[2..4]);
        let promotion_piece = if input.len() == 5 {
            match input.chars().nth(4) {
                Some(ch) => match parse_promotion_piece(ch) {
                    Ok(piece) => Some(piece),
                    Err(err) => {
                        println!("Error parsing promotion piece: {:?}", err);
                        None
                    }
                },
                None => {
                    println!("Error: Promotion piece character not found.");
                    None
                }
            }
        } else {
            None
        };

        if from.is_err() || to.is_err() {
            println!("Invalid move. Please try again.");
            continue;
        }

        let from = from.unwrap();
        let to = to.unwrap();

        match cb.make_move(from, to, promotion_piece) {
            Ok(new_cb) => {
                println!("You played {}!", input);
                display_board(&new_cb);
                cb = new_cb;
            }
            Err(err) => {
                println!("Invalid move: {}", err);
                continue;
            }
        }
        println!("-----------------------------------------------------------------");

        let stop_flag = Arc::new(AtomicBool::new(false));
        start_key_listener(Arc::clone(&stop_flag));
        // Engine move
        let start_time = std::time::Instant::now();
        // let best_lines = engine_move_best_5_lines(cb, MAX_DEPTH);

        let best_move = engine_move(cb, MAX_DEPTH);
        let best_lines = vec![(best_move.0, vec![cb, best_move.1])];
        let elapsed = start_time.elapsed();

        // let compared = compare_boards(&cb, &new_cb);
        // if compared.is_some() {
        //     let (from, to) = compared.unwrap();
        //     let formatted_move = format_move(from, to);
        //     println!("\nEngine played: {}", formatted_move);
        // } else {
        //     println!("\nEngine played: {}", "Unknown");
        // }
        // println!("Time taken: {:?}", elapsed);
        // display_board(&new_cb);
        // println!("Engine eval: {}", score);

        let new_cb: Chessboard;
        // First, check if we have at least one line
        if let Some(best_line) = best_lines.first() {
            new_cb = best_line.1[1];
            let new_cb_score = best_line.0;
            println!("\nTime taken: {:?}", elapsed);
            println!(
                "\n================== BEST LINE ({}) ==================",
                new_cb_score
            );

            for (i, window) in best_line.1.windows(2).enumerate() {
                let (prev, next) = (&window[0], &window[1]);
                let compared = compare_boards(prev, next);

                if let Some((from, to)) = compared {
                    let formatted_move = format_move(from, to);

                    if i == 0 {
                        // Highlight the first move with bold, background color, and green text
                        print!(
                            "{}{}{}",
                            SetForegroundColor(TextColor::Black), // Black text
                            SetBackgroundColor(TextColor::Yellow), // Yellow background
                            Attribute::Bold                   // Bold text
                        );
                        print!(" {} ", formatted_move); // Arrows for extra emphasis
                        print!("{}", ResetColor); // Reset styling after the move
                    } else {
                        // Use light blue text for the remaining moves
                        print!(
                            " {}{}{}",
                            SetForegroundColor(TextColor::Cyan),
                            formatted_move,
                            ResetColor
                        );
                    }
                } else {
                    print!(" UNKNOWN");
                }
            }

            println!("\n===============================================");
        } else {
            println!("NO MOVES!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
            new_cb = cb;
        }

        // Now print the rest of the lines
        for (i, line) in best_lines.iter().skip(1).enumerate() {
            // Adjust numbering if you want best line to be "Line 1"
            println!("Line {} ({}): ", i + 2, line.0);
            for window in line.1.windows(2) {
                let (prev, next) = (&window[0], &window[1]); // Extract the consecutive elements
                let compared = compare_boards(prev, next);

                if let Some((from, to)) = compared {
                    let formatted_move = format_move(from, to);
                    print!(" {}", formatted_move);
                } else {
                    print!(" UNKNOWN");
                }
            }
            println!();
        }

        display_board(&new_cb);

        cb = new_cb;
    }
}

fn engine_move_best_5_lines_iterative_deepening(
    cb: Chessboard,
    depth: u32,
    stop_flag: Arc<AtomicBool>,
) -> Vec<(f64, Vec<Chessboard>)> {
    let is_white_turn = if let Color::White = cb.side_to_move {
        true
    } else {
        false
    };

    println!("\nEngine thinking...");
    let best_5_lines = iterative_deepening_best_5_lines_with_interupt_support(
        &cb,
        depth,
        is_white_turn,
        stop_flag,
    );

    // Classic DFS
    // let best_5_lines = nega_max_alpha_beta_best_5_lines(
    //     &cb,
    //     depth,
    //     is_white_turn,
    //     i64::MIN, i64::MAX);

    // Map the lines and scores into the desired format with floating-point scores
    let best_5_lines_with_scores: Vec<(f64, Vec<Chessboard>)> = best_5_lines
        .into_iter()
        .map(|(score, line)| {
            let float_score = if cb.side_to_move == Color::Black {
                -score as f64 / 100.0
            } else {
                score as f64 / 100.0
            };
            (float_score, line)
        })
        .collect();

    best_5_lines_with_scores
}

fn engine_move_best_5_lines(cb: Chessboard, depth: u32) -> Vec<(f32, Vec<Chessboard>)> {
    let is_white_turn = if let Color::White = cb.side_to_move {
        true
    } else {
        false
    };
    let best_5_lines =
        nega_max_alpha_beta_best_line(&cb, depth, is_white_turn, i32::MIN, i32::MAX);

    println!("\nEngine thinking...");
    // Map the lines and scores into the desired format with floating-point scores
    let best_5_lines_with_scores: Vec<(f32, Vec<Chessboard>)> = best_5_lines
        .into_iter()
        .map(|(score, line)| {
            let float_score = if cb.side_to_move == Color::Black {
                -score as f32 / 100.0
            } else {
                score as f32 / 100.0
            };
            (float_score, line)
        })
        .collect();

    best_5_lines_with_scores
}

//////////////////////////// PARSING ////////////////////////////
fn parse_square(square: &str) -> Result<usize, ParseError> {
    if square.len() != 2 {
        return Err(ParseError::InvalidSquareFormat);
    }

    let file = square.chars().nth(0).unwrap().to_ascii_lowercase();
    let rank = square.chars().nth(1).unwrap();

    let file_index = match file {
        'a'..='h' => file as usize - 'a' as usize,
        _ => return Err(ParseError::InvalidFile),
    };

    let rank_index = match rank {
        '1'..='8' => rank as usize - '1' as usize,
        _ => return Err(ParseError::InvalidRank),
    };

    Ok(rank_index * 8 + file_index)
}

fn parse_promotion_piece(piece: char) -> Result<PromotionPiece, ParseError> {
    match piece {
        'q' | 'Q' => Ok(PromotionPiece::Queen),
        'r' | 'R' => Ok(PromotionPiece::Rook),
        'b' | 'B' => Ok(PromotionPiece::Bishop),
        'n' | 'N' => Ok(PromotionPiece::Knight),
        _ => Err(ParseError::InvalidPromotionPiece),
    }
}

fn engine_move(cb: Chessboard, depth: u32) -> (f32, Chessboard) {
    let is_white_turn = if let Color::White = cb.side_to_move {
        true
    } else {
        false
    };
    let best_move = nega_max_alpha_beta_best_move(&cb, depth, is_white_turn, i32::MIN, i32::MAX);
    let best_score = best_move.0;
    let float_score = if cb.side_to_move == Color::Black {
        -best_score as f32 / 100.0
    } else {
        best_score as f32 / 100.0
    };
    (float_score, best_move.1)
}

// fn engine_move_best_line(cb: Chessboard, depth: u32) -> (f64, Vec<Chessboard>) {
//     let is_white_turn = if let SideToMove::White = cb.side_to_move {
//         true
//     } else {
//         false
//     };
//     let (score, best_line) =
//         nega_max_alpha_beta_best_line(&cb, depth, is_white_turn, i64::MIN, i64::MAX);
//     let float_score = if cb.side_to_move == SideToMove::Black {
//         -score as f64 / 100.0
//     } else {
//         score as f64 / 100.0
//     };
//     (float_score, best_line)
// }
