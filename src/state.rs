use crate::engine::{is_win, reflect_bitboard, COLS, COLUMN_MASK, DEFAULT_MOVE_ORDER, IS_LEGAL, ROWS};
use std::collections::HashSet;
use std::fmt;
use crate::threats::FOUR_BIT_MASK;
use crate::{col_shift, index, update_height_map, update_pieces};


#[derive(Debug)]
#[derive(Eq, Hash, PartialEq, Clone)]
pub struct State {
    pub curr_pieces: u64,
    pub opp_pieces: u64,
    pub height_map: u64,
    pub moves_made: i8
}

impl State {
    const CURR_PIECE: char = 'X';
    const OPP_PIECE: char = 'O';

    fn new() -> Self {
        Self {
            curr_pieces: 0,
            opp_pieces: 0,
            height_map: 0,
            moves_made: 0,
        }
    }

    pub fn from_bitboard(bitboard: u64) -> State {
        let mut state = Self::new();

        for i in 0..COLS {
            let col = (bitboard >> (i << 3)) & COLUMN_MASK;
            let height = col.ilog2();
            let col_mask = (1 << height) - 1;

            state.curr_pieces |= (col & col_mask) << (i << 3);
            state.opp_pieces |= ((col_mask << 1) + 1 - col) << (i << 3);
            state.height_map |= 1 << (height + (i << 3));
            state.moves_made += height as i8;
        }

        state
    }

    pub fn to_bitboard(&self) -> u64 {
        self.curr_pieces | self.height_map
    }

    pub fn encode(board: Vec<&str>) -> Self {
        let board_str = board.join("\n");

        let mut game_state = Self {
            curr_pieces: 0,
            opp_pieces: 0,
            height_map: 0,
            moves_made: 0,
        };

        for c in 0..COLS {
            let mut cell = 1 << (c * (ROWS + 1));

            for r in 0..ROWS {
                let piece = board_str.as_bytes()[((ROWS - 1 - r) * (COLS + 1) + c) as usize] as char;

                if piece == Self::CURR_PIECE {
                    game_state.curr_pieces |= cell;
                    game_state.moves_made += 1;
                } else if piece == Self::OPP_PIECE {
                    game_state.opp_pieces |= cell;
                    game_state.moves_made += 1;
                } else {
                    break;
                }

                cell <<= 1;
            }

            game_state.height_map |= cell;
        }

        if (game_state.moves_made & 1) == 1 {
            let temp = game_state.curr_pieces;
            game_state.curr_pieces = game_state.opp_pieces;
            game_state.opp_pieces = temp;
        }

        game_state
    }

    pub fn decode(&self) -> String {
        let mut board_str = String::new();

        for r in (0..ROWS).rev() {
            let mut cell = 1 << r;

            for _ in 0..COLS {
                if (self.curr_pieces & cell) != 0 {
                    board_str.push(Self::CURR_PIECE);
                } else if (self.opp_pieces & cell) != 0 {
                    board_str.push(Self::OPP_PIECE);
                } else {
                    board_str.push(' ');
                }

                cell <<= ROWS + 1;
            }

            board_str.push('\n');
        }

        board_str
    }

    pub fn start_state() -> Self {
        Self::encode(vec![&" ".repeat(COLS as usize); ROWS as usize])
    }

    pub fn next_states(self) -> Vec<Self> {
        let mut next_states = vec![];

        for i in 0..COLS {
            let col = (DEFAULT_MOVE_ORDER >> index!(i)) & FOUR_BIT_MASK;
            let next_move = self.height_map & (COLUMN_MASK << col_shift!(col));

            if (next_move & IS_LEGAL) != 0 {
                next_states.push(State {
                    curr_pieces: self.opp_pieces,
                    opp_pieces: update_pieces!(self.curr_pieces, next_move),
                    height_map: update_height_map!(self.height_map, next_move),
                    moves_made: self.moves_made + 1,
                });
            }
        }

        next_states
    }

    fn generate_states_rec(self, depth: usize, states: &mut HashSet<u64>) {
        let state_bitboard = self.to_bitboard();

        if !states.contains(&reflect_bitboard(state_bitboard)) {
            states.insert(state_bitboard);

            if depth == 0 || is_win(self.opp_pieces) {
                return;
            }

            for next_state in self.next_states() {
                next_state.generate_states_rec(depth - 1, states);
            }
        }
    }

    pub fn generate_states(self, depth: usize) -> HashSet<u64> {
        let mut states = HashSet::new();

        self.generate_states_rec(depth, &mut states);

        states
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.decode())
    }
}
