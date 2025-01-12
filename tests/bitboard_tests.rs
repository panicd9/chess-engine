use crate::board::Bitboards;
use crate::move_gen::generate_rook_moves;

#[test]
fn test_rook_moves() {
    let board = Bitboards::new();
    let rook_position = board.white_rooks;
    let moves = generate_rook_moves(rook_position, 0);  // Example test case
    // Add assertions based on expected moves
}
