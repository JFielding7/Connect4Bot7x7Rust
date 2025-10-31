use crate::caches::{StateCaches, CACHE_SIZE};
use crate::error::{Connect4Error, Result};
use crate::state::*;
use crate::threats::*;
use crate::worker_threads::*;
use crate::*;
use std::cmp::{max, min};
use std::sync::atomic::{AtomicBool, Ordering};


const CONNECTION_DIRECTIONS: &[i32; 4] = &[1, 7, 8, 9];
const MAX_TOTAL_MOVES: i8 = 49;
pub const MAX_PLAYER_MOVES: i8 = 25;
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

fn next_legal_moves(move_order: u32, height_map: u64) -> impl Iterator<Item = (u32, u64)> {
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

    alpha = max(alpha, caches.get_lower_bound(state, moves_made, cache_index));
    if alpha >= beta {
        return Some(alpha);
    }

    beta = min(beta, caches.get_upper_bound(state, moves_made, cache_index));
    if alpha >= beta {
        return Some(alpha);
    }

    let mut threats = 0;
    let mut forced_move_count = 0;
    let mut forced_move = 0;

    for (col, next_move) in next_legal_moves(DEFAULT_MOVE_ORDER, height_map) {
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

        alpha = max(alpha, -caches.get_upper_bound(
            next_state,
            moves_made + 1,
            cache_index!(next_state),
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

    for (_col, next_move) in next_legal_moves(heuristic_move_order, height_map) {
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

            caches.put_lower_bound(alpha, state, moves_made, cache_index);
            return Some(alpha);
        }

        moves_searched += 1;
    }

    caches.put_upper_bound(alpha, state, moves_made, cache_index);
    Some(alpha)
}

pub fn evaluate_position_with_workers(game_state: State, pos: &mut usize) -> Result<i8> {
    let mut caches = StateCaches::new();

    let worker_thread_handlers = spawn_evaluate_position_worker_threads(
        DEFAULT_NUM_WORKER_THREADS, &game_state, &caches);

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
    ).ok_or_else(|| Connect4Error::EvaluatePositionError)?;

    for handler in &worker_thread_handlers {
        handler.terminate();
    }

    for handler in worker_thread_handlers {
        handler.join().map_err(|_| Connect4Error::WorkerThreadJoinError)?;
    }

    Ok(eval)
}

pub fn optimal_moves(
    state: &State,
    caches: &mut StateCaches,
    pos: &mut usize,
) -> Result<(i8, Vec<u32>)> {

    let mut best_moves = Vec::new();
    let mut threats = 0;

    for (col, next_move) in next_legal_moves(DEFAULT_MOVE_ORDER, state.height_map) {
        let updated_pieces = update_pieces!(state.curr_pieces, next_move);

        if is_win(updated_pieces) {
            best_moves.push(col);
        }

        let updated_height_map = update_height_map!(state.height_map, next_move);
        threats |= count_threats(updated_pieces, updated_height_map) << index!(col);
    }

    if best_moves.len() > 0 {
        return Ok((max_eval!(state.moves_made), best_moves))
    }

    let heuristic_move_order = sort_by_threats(threats);
    let mut state_max_eval = MIN_EVAL;
    let unused = AtomicBool::new(false);

    for (col, next_move) in next_legal_moves(heuristic_move_order, state.height_map) {
        let mut eval = -evaluate_position_rec(
            state.opp_pieces,
            update_pieces!(state.curr_pieces, next_move),
            update_height_map!(state.height_map, next_move),
            state.moves_made + 1,
            -state_max_eval - 1,
            -state_max_eval + 1,
            caches,
            &unused,
            pos
        ).ok_or_else(|| Connect4Error::EvaluatePositionError)?;

        println!("Initial Eval: {eval} {col}");

        if eval > state_max_eval {
            eval = -evaluate_position_rec(
                state.opp_pieces,
                update_pieces!(state.curr_pieces, next_move),
                update_height_map!(state.height_map, next_move),
                state.moves_made + 1,
                MIN_EVAL,
                -eval,
                caches,
                &unused,
                pos
            ).ok_or_else(|| Connect4Error::EvaluatePositionError)?;

            println!("Updated Eval: {eval} {col}");

            best_moves = vec![col];
            state_max_eval = eval;
        } else if eval == state_max_eval {
            best_moves.push(col);
        }
    }

    Ok((state_max_eval, best_moves))
}

pub fn optimal_moves_with_workers(
    state: &State,
    caches: &mut StateCaches,
    pos: &mut usize
) -> Result<(i8, Vec<u32>)> {

    let worker_thread_handlers = spawn_evaluate_position_worker_threads(
        DEFAULT_NUM_WORKER_THREADS, state, caches);

    let best_moves = optimal_moves(state, caches, pos)?;

    for handler in &worker_thread_handlers {
        handler.terminate();
    }

    for handler in worker_thread_handlers {
        *pos += handler.join().map_err(|_| Connect4Error::WorkerThreadJoinError)?;
    }

    Ok(best_moves)
}
