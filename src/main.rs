

use std::io::{self, Write};

use  chess_engine::{chessboard::{Chessboard, Color}, display::display_board, move_gen::{black_legal_moves, white_legal_moves}, perft::{compare_boards, format_move}, piece::PromotionPiece, search::{nega_max, nega_max_alpha_beta}};

fn main() {
    let mut cb = Chessboard::new_initial_board();
    display_board(&cb);

    loop {
        if cb.side_to_move == Color::White {
            println!("\nWhite to move:");
        } else {
            println!("\nBlack to move:");
        }

        print!("Enter your move (e.g., e2e4): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.len() != 4  && input.len() != 5 {
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
            Ok(new_cb) => cb = new_cb,
            Err(err) => {
                println!("Invalid move: {}", err);
                continue;
            }
        }

        // Engine move
        let start_time = std::time::Instant::now();
        let (score, new_cb) = engine_move(cb, 6);
        let elapsed = start_time.elapsed();
        println!("Time taken: {:?}", elapsed);

        let compared = compare_boards(&cb, &new_cb);
        if compared.is_some() {
            let (from, to) = compared.unwrap();
            let formatted_move = format_move(from, to);
            println!("Engine played: {}", formatted_move);
        } else {
            println!("Engine played: {}", "Unknown");
        }
        display_board(&new_cb);
        println!("Engine eval: {}", score);
        cb = new_cb;
    }

    #[derive(Debug)]
    pub enum ParseError {
        InvalidSquareFormat,
        InvalidFile,
        InvalidRank,
        InvalidPromotionPiece,
    }

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
    
    fn engine_move(cb: Chessboard, depth: u32) -> (f64, Chessboard) {
        let is_white_turn = if let Color::White = cb.side_to_move { true } else { false };
        let (score, best_cb) = nega_max_alpha_beta(&cb, depth, is_white_turn, i64::MIN, i64::MAX);
        let float_score = if cb.side_to_move == Color::Black {
            -score as f64 / 100.0
        } else {
            score as f64 / 100.0
        };
        (float_score, best_cb)
    }
}