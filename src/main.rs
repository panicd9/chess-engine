use chess_engine::{chessboard::{Chessboard, Color}, display::display_board, eval::{self, evaluate}, playing_ui::play, search::quiescence_search};


fn main() {
    let cb = Chessboard::from_fen("rnbqkbnr/pppp1ppp/4p3/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2").unwrap();
    display_board(&cb);
    println!("Board debug: {:?}", cb);
    let is_white_turn = if cb.side_to_move == Color::White {
        true
    } else {
        false
    };

    // let qsearch =  quiescence_search(&cb, is_white_turn, i32::MIN, i32::MAX);
    // println!("eval: {}", qsearch[0].0 as f32 / 100.0);
    // println!("{:?}", qsearch[0].1);

    let negamax = evaluate(&cb);
    println!("eval: {}", negamax as f32 / 100.0);
    // display_board(qsearch[1].1.first().unwrap());
    play();
}