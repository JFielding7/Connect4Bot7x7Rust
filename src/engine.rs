use crate::{
    index, cache_index,
    get_cache_entry_eval,
    create_cache_entry,
    cache_get, cache_get_lower_bound, cache_get_upper_bound,
    cache_put, cache_put_lower_bound, cache_put_upper_bound,
};
use crate::threats::{count_threats, sort_by_threats, FOUR_BIT_MASK};
use std::cmp::{max, min};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::caches::{StateCaches, CACHE_SIZE, BEGINNING_GAME_CACHE_DEPTH, CACHE_VALUE_SHIFT};
use crate::state::State;
use crate::worker_threads::spawn_worker_threads;
use std::thread::Result;

pub const ROWS: u32 = 7;
pub const COLS: u32 = 7;
const BOARD_BITS: usize = 56;
const BOARD_MASK: u64 = (1 << BOARD_BITS) - 1;
pub const COL_BITS: usize = 8;
pub const COLUMN_MASK: u64 = (1 << COL_BITS) - 1;
const CONNECTION_DIRECTIONS: &[i32; 4] = &[1, 7, 8, 9];
const MAX_TOTAL_MOVES: i8 = 49;
const MAX_PLAYER_MOVES: i8 = 25;
pub const MAX_EVAL: i8 = 22;
pub const MIN_EVAL: i8 = -MAX_EVAL;
const DRAW: i8 = 0;
pub const DEFAULT_MOVE_ORDER: u32 = (3 << 0) | (2 << 4) | (4 << 8) | (5 << 12) | (1 << 16) | (6 << 20) | (0 << 24);
pub const IS_LEGAL: u64 = 0b01111111011111110111111101111111011111110111111101111111;


macro_rules! min_eval {
    ($moves_made:expr) => {
        -(MAX_PLAYER_MOVES - (($moves_made + 1) >> 1))
    };
}

macro_rules! max_eval {
    ($moves_made:expr) => {
        MAX_PLAYER_MOVES - ($moves_made >> 1)
    };
}

#[macro_export]
macro_rules! update_pieces {
    ($curr_pieces:expr, $next_move:expr) => {
        $curr_pieces | $next_move
    };
}

#[macro_export]
macro_rules! update_height_map {
    ($height_map:expr, $next_move:expr) => {
        $height_map + $next_move
    };
}

macro_rules! get_col {
    ($cols:expr, $i:expr) => {
        ($cols >> index!($i)) & FOUR_BIT_MASK
    };
}

#[macro_export]
macro_rules! col_shift {
    ($col:expr) => {
        $col << 3
    };
}

macro_rules! open_row {
    ($height_map:expr, $col:expr) => {
        $height_map & (COLUMN_MASK << col_shift!($col))
    };
}

pub fn reflect_bitboard(state: u64) -> u64 {
    let mut reflected = 0;

    for i in (0..BOARD_BITS).step_by(COL_BITS) {
        reflected |= ((state >> i) & COLUMN_MASK) << ((BOARD_BITS - COL_BITS) - i);
    }

    reflected
}

pub fn state_bitboard(curr_pieces: u64, height_map: u64) -> u64 {
    let bitboard = curr_pieces | height_map;
    let reflected_bitboard = reflect_bitboard(bitboard);

    min(bitboard, reflected_bitboard)
}

pub fn is_win(pieces: u64) -> bool {
    for i in CONNECTION_DIRECTIONS {
        let mut connections = pieces;

        for _ in 0..3 {
            connections &= connections >> i;
        }

        if connections != 0 {
            return true
        }
    }

    false
}

fn next_moves(move_order: u32, height_map: u64) -> impl Iterator<Item = (u32, u64)> {
    (0..COLS).filter_map(move |i| {
        let col = get_col!(move_order, i);
        let next_move = open_row!(height_map, col);

        if (next_move & IS_LEGAL) != 0 {
            Some((col, next_move))
        } else {
            None
        }
    })
}

// unpack state struct for better performance
pub fn evaluate_position_rec(
    curr_pieces: u64,
    opp_pieces: u64,
    height_map: u64,
    moves_made: i8,
    mut alpha: i8,
    mut beta: i8,
    caches: &mut StateCaches,
    terminate: &AtomicBool,
    pos: &mut usize,
) -> Option<i8> {

    if terminate.load(Ordering::Relaxed) {
        return None
    }

    *pos += 1;

    if moves_made == MAX_TOTAL_MOVES {
        return Some(DRAW);
    }

    alpha = max(alpha, min_eval!(moves_made));
    beta = min(beta, max_eval!(moves_made));

    let state = state_bitboard(curr_pieces, height_map);
    let cache_index = cache_index!(state);

    alpha = max(alpha, cache_get_lower_bound!(state, moves_made, cache_index, caches));
    if alpha >= beta {
        return Some(alpha);
    }

    beta = min(beta, cache_get_upper_bound!(state, moves_made, cache_index, caches));
    if alpha >= beta {
        return Some(alpha);
    }

    let mut threats = 0;
    let mut forced_move_count = 0;
    let mut forced_move = 0;

    for (col, next_move) in next_moves(DEFAULT_MOVE_ORDER, height_map) {
        let updated_pieces = update_pieces!(curr_pieces, next_move);

        if is_win(updated_pieces) {
            return Some(max_eval!(moves_made));
        }

        if is_win(update_pieces!(opp_pieces, next_move)) {
            forced_move_count += 1;
            forced_move = next_move;
        }

        let updated_height_map = update_height_map!(height_map, next_move);

        let next_state = state_bitboard(opp_pieces, updated_height_map);

        alpha = max(alpha, -cache_get_upper_bound!(
            next_state,
            moves_made + 1,
            cache_index!(next_state),
            caches
        ));

        if alpha >= beta {
            return Some(alpha);
        }

        threats |= count_threats(updated_pieces, updated_height_map) << index!(col);
    }

    if forced_move_count > 1 {
        return Some(min_eval!(moves_made));
    }

    if forced_move_count == 1 {
        return Some(-evaluate_position_rec(
            opp_pieces,
            update_pieces!(curr_pieces, forced_move),
            update_height_map!(height_map, forced_move),
            moves_made + 1,
            -beta,
            -alpha,
            caches,
            terminate,
            pos
        )?);
    }

    let heuristic_move_order = sort_by_threats(threats);
    let mut moves_searched = 0;

    for (_col, next_move) in next_moves(heuristic_move_order, height_map) {
        let updated_pieces = update_pieces!(curr_pieces, next_move);
        let updated_height_map = update_height_map!(height_map, next_move);

        let eval = if moves_searched == 0 {
            -evaluate_position_rec(
                opp_pieces,
                updated_pieces,
                updated_height_map,
                moves_made + 1,
                -beta,
                -alpha,
                caches,
                terminate,
                pos
            )?
        } else {
            let null_window_eval = -evaluate_position_rec(
                opp_pieces,
                updated_pieces,
                updated_height_map,
                moves_made + 1,
                -alpha - 1,
                -alpha,
                caches,
                terminate,
                pos
            )?;

            if null_window_eval > alpha && null_window_eval < beta {
                -evaluate_position_rec(
                    opp_pieces,
                    updated_pieces,
                    updated_height_map,
                    moves_made + 1,
                    -beta,
                    -alpha,
                    caches,
                    terminate,
                    pos
                )?
            } else {
                null_window_eval
            }
        };

        alpha = max(alpha, eval);

        if alpha >= beta {

            cache_put_lower_bound!(alpha, state, moves_made, cache_index, caches);
            return Some(alpha);
        }

        moves_searched += 1;
    }

    cache_put_upper_bound!(alpha, state, moves_made, cache_index, caches);
    Some(alpha)
}

pub fn evaluate_position(game_state: State, pos: &mut usize) -> Result<i8> {
    let mut caches = StateCaches::new();

    let worker_thread_handlers = spawn_worker_threads(game_state.clone(), &caches);

    let eval = evaluate_position_rec(
        game_state.curr_pieces,
        game_state.opp_pieces,
        game_state.height_map,
        game_state.moves_made,
        MIN_EVAL,
        MAX_EVAL,
        &mut caches,
        &AtomicBool::new(false),
        pos,
    ).unwrap();

    for handler in &worker_thread_handlers {
        handler.terminate();
    }

    for handler in worker_thread_handlers {
        handler.join()?;
    }

    Ok(eval)
}
