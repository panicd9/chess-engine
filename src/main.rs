mod chessboard;
mod r#move;
mod move_gen;
mod piece;
mod utils;
mod display;

use display::display_board;

use crate::chessboard::Chessboard;

fn main() {
    let cb = Chessboard::new_initial_board();

    display_board(&cb, None);
}
