use crate::chessboard::Chessboard;

#[derive(Clone)]
pub struct Move {
    pub chessboard: Chessboard,
    pub score: u32
}

impl Move {
    pub fn new(chessboard: Chessboard, score: u32) -> Self {
        Move { chessboard, score}
    }
}

/// A structure that wraps a vector of Chessboard objects (moves)
/// and maintains an index into the "unsorted" portion of the list.
pub struct MoveList {
    pub moves: Vec<Move>,
    next_index: usize, // All moves before next_index are ordered.
}

impl MoveList {
    /// Creates a new MoveList from a vector of Chessboard moves.
    pub fn new(boards: Vec<Move>) -> Self {
        MoveList {
        moves: boards,
            next_index: 0,
        }
    }

    /// Returns the next best move (Chessboard) from the unsorted tail of the list.
    /// This method performs a lazy selection: it searches for the best move among
    /// boards[next_index..], swaps it to the front of that segment, increments next_index,
    /// and returns the best move.
    pub fn next_move(&mut self) -> Option<Move> {
        let n = self.moves.len();
        if self.next_index >= n {
            return None; // No more moves.
        }

        // Find the index of the best move in boards[next_index..n].
        let mut best_index = self.next_index;
        for j in (self.next_index + 1)..n {
            if self.moves[j].score > self.moves[best_index].score {
                best_index = j;
            }
        }

        // Swap the best move into the position at next_index.
        self.moves.swap(self.next_index, best_index);

        // Return that move and increment next_index.
        let best_move = self.moves[self.next_index].clone();
        self.next_index += 1;
        Some(best_move)
    }
}