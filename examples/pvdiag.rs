// Why does the principal variation stop early? Replays the table walk that
// `search::principal_variation` does, reporting the reason at each step.
use chess_engine::chessboard::{Chessboard, Color};
use chess_engine::move_gen::legal_moves;
use chess_engine::notation::describe_move;
use chess_engine::search::{self, History};
use chess_engine::zobrist;

fn occ(cb: &Chessboard, white: bool) -> u64 {
    if white { cb.get_white_occupancy() } else { cb.get_black_occupancy() }
}

fn main() {
    let mut cb = Chessboard::new_initial_board();
    for mv in ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"] {
        let p = chess_engine::notation::parse_move(mv).unwrap();
        cb = cb.make_move(p.from, p.to, p.promotion).unwrap();
    }
    let depth: u32 = 11;
    let mut h = History::new();
    h.ensure_table(256);
    let is_white = cb.side_to_move == Color::White;
    for d in 1..=depth {
        search::nega_max_alpha_beta_best_move(&cb, d, is_white, i32::MIN + 1, i32::MAX - 1, &mut h);
    }

    let mut pos = cb;
    let mut seen: Vec<u64> = Vec::new();
    for ply in 0..depth as usize {
        let key = zobrist::hash(&pos);
        if seen.contains(&key) { println!("ply {ply}: STOP — line repeats"); break; }
        seen.push(key);
        let white = pos.side_to_move == Color::White;
        let parent = occ(&pos, white);
        match h.table.best_move(key) {
            None => { println!("ply {ply}: STOP — table has no move for this position"); break; }
            Some(wanted) => {
                let moves = legal_moves(&pos);
                let hit = moves.iter().map(|m| m.chessboard)
                    .find(|c| parent ^ occ(c, white) == wanted);
                match hit {
                    None => {
                        println!("ply {ply}: STOP — stored move {wanted:#x} matches none of {} legal moves \
                                  (collision, or castling/promotion ambiguity)", moves.len());
                        break;
                    }
                    Some(child) => {
                        println!("ply {ply}: {} (key {wanted:#x})",
                                 describe_move(&pos, &child).unwrap_or("????".into()));
                        pos = child;
                    }
                }
            }
        }
    }
}
