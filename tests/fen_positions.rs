use chess_engine::{chessboard::{Chessboard, Color}, display::{self, display_board}, perft::{perft, perft_divide}, squares::SquareBitboard};

#[test]
pub fn initial_position(){
    let cb = Chessboard::new_initial_board();
    // display_board(&cb);

    let nodes = perft(&cb, 7);
    println!("Nodes searched: {}", nodes);
    assert_eq!(nodes, 3_195_901_860	);
}

#[test]
pub fn fen_test_2() {
    let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    let cb = Chessboard::from_fen(fen).unwrap();
    // display_board(&cb);

    let nodes = perft(&cb, 6);
    println!("Nodes searched: {}", nodes);
    assert_eq!(nodes, 8_031_647_685);
}

#[test]
pub fn fen_test_3(){
    let fen = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
    let cb = Chessboard::from_fen(fen).unwrap();
    // display_board(&cb);

    let nodes = perft(&cb, 8);
    println!("Nodes searched: {}", nodes);
    assert_eq!(nodes, 3_009_794_393);
}

#[test]
pub fn fen_test_4(){
    let fen = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
    let cb = Chessboard::from_fen(fen).unwrap();
    // display_board(&cb);

    let nodes = perft(&cb, 6);
    println!("Nodes searched: {}", nodes);
    assert_eq!(nodes, 706_045_033);
}

#[test]
pub fn fen_test_5(){
    let fen = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";
    let cb = Chessboard::from_fen(fen).unwrap();
    // display_board(&cb);

    let nodes = perft(&cb, 5);
    println!("Nodes searched: {}", nodes);
    assert_eq!(nodes, 89_941_194);
}
