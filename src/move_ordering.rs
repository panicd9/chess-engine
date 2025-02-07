use crate::piece::Piece;

pub const EN_PASSANT_MOVE_ORDER_SCORE: u32 = 15; 
// TODO: Benchmark with ColoredPiece precomputed table
/// MVV-LVA precomputed table:
pub const MVV_LVA: [[u32; 7];7] = [
    [15, 25, 35, 45, 55, 0, 0], // Attacker P, Victim P, N, B, R, Q, K, None
    [14, 24, 34, 44, 54, 0, 2], // Attacker N, Victim P, N, B, R, Q, K, None
    [13, 23, 33, 43, 53, 0, 2], // Attacker B, Victim P, N, B, R, Q, K, None
    [12, 22, 32, 42, 52, 0, 3], // Attacker R, Victim P, N, B, R, Q, K, None
    [11, 21, 31, 41, 51, 0, 4], // Attacker Q, Victim P, N, B, R, Q, K, None
    [10 ,20, 30, 40, 50, 0, 0], // Attacker K, Victim P, N, B, R, Q, K, None
    [0, 0, 0, 0, 0, 0, 0]       // Attacker None
];

pub fn mvv_lva_score(attacker: Piece, victim: Piece) -> u32 {
    return MVV_LVA[attacker as usize][victim as usize];
}

pub fn move_order_score(attacker: Piece, victim: Piece) -> u32 {
    mvv_lva_score(attacker, victim)
}