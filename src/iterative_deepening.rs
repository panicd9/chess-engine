use crate::{chessboard::Chessboard, search::nega_max_alpha_beta_best_line};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode};

pub fn iterative_deepening_best_5_lines(
    cb: &Chessboard,
    max_depth: u32,
    is_white_turn: bool,
) -> Vec<(i32, Vec<Chessboard>)> {
    let mut best_lines: Vec<(i32, Vec<Chessboard>)> = Vec::new();
    let mut alpha = i32::MIN + 1;
    let mut beta = i32::MAX - 1;

    for depth in 1..=max_depth {
        let current_lines = nega_max_alpha_beta_best_line(cb, depth, is_white_turn, alpha, beta);

        if !current_lines.is_empty() {
            best_lines = current_lines;
            // Optional: Use the top line's score to refine alpha for faster pruning
            // alpha = best_lines[0].0.max(alpha);
        }

        // Optional: Add time constraint check here if needed
    }

    best_lines
}

// Iterative Deepening with interrupt support
pub fn iterative_deepening_best_5_lines_with_interupt_support(
    cb: &Chessboard,
    max_depth: u32,
    is_white_turn: bool,
    stop_flag: Arc<AtomicBool>,
) -> Vec<(i32, Vec<Chessboard>)> {
    let mut best_lines = Vec::new();
    let mut alpha = i32::MIN;
    let mut beta = i32::MAX;

    for depth in 1..=max_depth {
        if stop_flag.load(Ordering::Relaxed) {
            println!("Search stopped at depth {}", depth - 1);
            break;
        }

        let current_lines = nega_max_alpha_beta_best_line(cb, depth, is_white_turn, alpha, beta);
        if !current_lines.is_empty() {
            best_lines = current_lines;
            alpha = best_lines[0].0.max(alpha);
        }
    }

    best_lines
}

// Key listener that sets the stop flag when 'q' is pressed
pub fn start_key_listener(stop_flag: Arc<AtomicBool>) {
    thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap() {
                if let Event::Key(key_event) = event::read().unwrap() {
                    if key_event.code == KeyCode::Char('q') {
                        stop_flag.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        }
    });
}