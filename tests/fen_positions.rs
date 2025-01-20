use chess_engine::{chessboard::{Chessboard, Color}, display::{self, display_board}, perft::{perft, perft_divide}, squares::SquareBitboard};

#[test]
pub fn fen_test_1() {
    let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    let mut cb = Chessboard::from_fen(fen).unwrap();
    // cb.white_pawns &= !(SquareBitboard::D5 as u64);
    // cb.white_pawns |= SquareBitboard::D6 as u64;
    // cb.black_pawns &= !(SquareBitboard::B4 as u64);
    // cb.black_pawns |= SquareBitboard::B3 as u64;
    // cb.white_pawns &= !(SquareBitboard::D6 as u64);
    // cb.white_pawns |= SquareBitboard::C7 as u64;
    // cb.black_pawns &= !(SquareBitboard::C7 as u64);

    // cb.black_pawns &= !(SquareBitboard::G6 as u64);
    // cb.black_pawns |= SquareBitboard::G5 as u64;
    // cb.white_pawns &= !(SquareBitboard::A2 as u64);
    // cb.white_pawns |= SquareBitboard::A4 as u64;
    // cb.en_passant = SquareBitboard::A3 as u64;
    // cb.black_pawns &= !(SquareBitboard::B4 as u64);
    // cb.black_pawns |= SquareBitboard::A3 as u64;
    // cb.white_pawns &= !(SquareBitboard::A4 as u64);

    cb.side_to_move = Color::White;
    display_board(&cb);

    let nodes = perft(&cb, 6);
    println!("Nodes: {}", nodes);
    assert_eq!(nodes, 8031647685);
}