use std::cmp::{max, min};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::sync::Arc;
use crate::index;
use crate::state::State;
use crate::threat_sort::{count_threats, sort_by_threats, FOUR_BIT_MASK};
use dashmap::DashMap;

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
const CACHE_VALUE_SHIFT: u8 = 56;
pub const CACHE_SIZE: usize = (1 << 19) + 1;
const BEGINNING_GAME_CACHE_DEPTH: i8 = 32;


pub struct StateCaches {
    beg_game_lower_bound_cache: Arc<DashMap<u64, i8>>,
    beg_game_upper_bound_cache: Arc<DashMap<u64, i8>>,
    end_game_lower_bound_cache: Vec<u64>,
    end_game_upper_bound_cache: Vec<u64>,
}

impl StateCaches {
    pub fn from_beg_caches(
        beg_game_lower_bound_cache: Arc<DashMap<u64, i8>>,
        beg_game_upper_bound_cache: Arc<DashMap<u64, i8>>
    ) -> Self {
        Self {
            beg_game_lower_bound_cache,
            beg_game_upper_bound_cache,
            end_game_lower_bound_cache: vec![0; CACHE_SIZE],
            end_game_upper_bound_cache: vec![0; CACHE_SIZE],
        }
    }

    pub fn new() -> Self {
        Self::from_beg_caches(Arc::new(DashMap::new()), Arc::new(DashMap::new()))
    }

    pub fn new_same_beg_cache(&self) -> Self {
        Self::from_beg_caches(
            self.beg_game_lower_bound_cache.clone(),
            self.beg_game_upper_bound_cache.clone()
        )
    }
}


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

macro_rules! cache_index {
    ($state:expr) => {
        $state as usize % CACHE_SIZE
    };
}

macro_rules! get_cache_entry_eval {
    ($cache_entry:expr) => {
        ($cache_entry >> CACHE_VALUE_SHIFT) as i8 - MAX_PLAYER_MOVES
    }
}

macro_rules! create_cache_entry {
    ($state:expr, $bound:expr) => {
        $state | ((($bound + MAX_PLAYER_MOVES) as u64) << CACHE_VALUE_SHIFT)
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

fn cache_lookup(
    state: u64,
    moves_made: i8,
    cache_index: usize,
    beg_game_cache: Arc<DashMap<u64, i8>>,
    end_game_cache: &Vec<u64>,
    default_bound: i8
) -> i8 {
    if moves_made <= BEGINNING_GAME_CACHE_DEPTH {
        if let Some(cache_bound) = beg_game_cache.get(&state) {
            return *cache_bound
        }
    } else {
        let cache_entry = end_game_cache[cache_index];

        if (cache_entry & BOARD_MASK) == state {
            return get_cache_entry_eval!(cache_entry)
        }
    }

    default_bound
}

fn lower_bound_cache_lookup(
    state: u64,
    moves_made: i8,
    cache_index: usize,
    caches: &StateCaches,
) -> i8 {
    cache_lookup(
        state,
        moves_made,
        cache_index,
        caches.beg_game_lower_bound_cache.clone(),
        &caches.end_game_lower_bound_cache,
        MIN_EVAL,
    )
}

fn upper_bound_cache_lookup(
    state: u64,
    moves_made: i8,
    cache_index: usize,
    caches: &StateCaches,
) -> i8 {
    cache_lookup(
        state,
        moves_made,
        cache_index,
        caches.beg_game_upper_bound_cache.clone(),
        &caches.end_game_upper_bound_cache,
        MAX_EVAL,
    )
}

fn cache_put(
    bound: i8,
    state: u64,
    moves_made: i8,
    cache_index: usize,
    beg_game_cache: Arc<DashMap<u64, i8>>,
    end_game_cache: &mut Vec<u64>,
    cmp: fn(i8, i8) -> i8
) {
    if moves_made > BEGINNING_GAME_CACHE_DEPTH {
        end_game_cache[cache_index] = create_cache_entry!(state, bound);
    } else {
        beg_game_cache.entry(state)
            .and_modify(|entry| *entry = cmp(*entry, bound))
            .or_insert(bound);
    }
}

fn cache_put_lower_bound(
    bound: i8,
    state: u64,
    moves_made: i8,
    cache_index: usize,
    caches: &mut StateCaches,
) {
    cache_put(
        bound,
        state,
        moves_made,
        cache_index,
        caches.beg_game_lower_bound_cache.clone(),
        &mut caches.end_game_lower_bound_cache,
        max,
    )
}

fn cache_put_upper_bound(
    bound: i8,
    state: u64,
    moves_made: i8,
    cache_index: usize,
    caches: &mut StateCaches,
) {
    cache_put(
        bound,
        state,
        moves_made,
        cache_index,
        caches.beg_game_upper_bound_cache.clone(),
        &mut caches.end_game_upper_bound_cache,
        min,
    )
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
pub fn evaluate_position(
    curr_pieces: u64,
    opp_pieces: u64,
    height_map: u64,
    moves_made: i8,
    mut alpha: i8,
    mut beta: i8,
    caches: &mut StateCaches,
    pos: &mut usize,
) -> i8 {
    if *pos == 0 {
        println!("{curr_pieces}");
        println!("{opp_pieces}");
        println!("{height_map}");
        println!("{moves_made}");
    }

    *pos += 1;

    if moves_made == MAX_TOTAL_MOVES {
        return DRAW;
    }

    alpha = max(alpha, min_eval!(moves_made));
    beta = min(beta, max_eval!(moves_made));

    let state = state_bitboard(curr_pieces, height_map);
    let cache_index = cache_index!(state);

    alpha = max(alpha, lower_bound_cache_lookup(state, moves_made, cache_index, caches));
    if alpha >= beta {
        return alpha;
    }

    beta = min(beta, upper_bound_cache_lookup(state, moves_made, cache_index, caches));
    if alpha >= beta {
        return alpha;
    }

    let mut threats = 0;
    let mut forced_move_count = 0;
    let mut forced_move = 0;

    for (col, next_move) in next_moves(DEFAULT_MOVE_ORDER, height_map) {
        let updated_pieces = update_pieces!(curr_pieces, next_move);

        if is_win(updated_pieces) {
            return max_eval!(moves_made);
        }

        if is_win(update_pieces!(opp_pieces, next_move)) {
            forced_move_count += 1;
            forced_move = next_move;
        }

        let updated_height_map = update_height_map!(height_map, next_move);

        let next_state = state_bitboard(opp_pieces, updated_height_map);

        alpha = max(alpha, -upper_bound_cache_lookup(
            next_state,
            moves_made + 1,
            cache_index!(next_state),
            caches
        ));

        if alpha >= beta {
            return alpha;
        }

        threats |= count_threats(updated_pieces, updated_height_map) << index!(col);
    }

    if forced_move_count > 1 {
        return min_eval!(moves_made);
    }

    if forced_move_count == 1 {
        return -evaluate_position(
            opp_pieces,
            update_pieces!(curr_pieces, forced_move),
            update_height_map!(height_map, forced_move),
            moves_made + 1,
            -beta,
            -alpha,
            caches,
            pos
        );
    }

    let heuristic_move_order = sort_by_threats(threats);
    let mut moves_searched = 0;

    for (_col, next_move) in next_moves(heuristic_move_order, height_map) {
        let updated_pieces = update_pieces!(curr_pieces, next_move);
        let updated_height_map = update_height_map!(height_map, next_move);

        let eval = if moves_searched == 0 {
            -evaluate_position(
                opp_pieces,
                updated_pieces,
                updated_height_map,
                moves_made + 1,
                -beta,
                -alpha,
                caches,
                pos
            )
        } else {
            let null_window_eval = -evaluate_position(
                opp_pieces,
                updated_pieces,
                updated_height_map,
                moves_made + 1,
                -alpha - 1,
                -alpha,
                caches,
                pos
            );

            if null_window_eval > alpha && null_window_eval < beta {
                -evaluate_position(
                    opp_pieces,
                    updated_pieces,
                    updated_height_map,
                    moves_made + 1,
                    -beta,
                    -alpha,
                    caches,
                    pos
                )
            } else {
                null_window_eval
            }
        };

        alpha = max(alpha, eval);

        if alpha >= beta {

            cache_put_lower_bound(alpha, state, moves_made, cache_index, caches);
            return alpha;
        }

        moves_searched += 1;
    }

    cache_put_upper_bound(alpha, state, moves_made, cache_index, caches);
    alpha
}

pub fn best_moves(
    state: State,
    caches: &mut StateCaches,
    pos: &mut usize,
) -> Vec<u32> {
    let mut best_moves = Vec::new();
    let mut threats = 0;

    for (col, next_move) in next_moves(DEFAULT_MOVE_ORDER, state.height_map) {
        let updated_pieces = update_pieces!(state.curr_pieces, next_move);

        if is_win(updated_pieces) {
            best_moves.push(col);
        }

        let updated_height_map = update_height_map!(state.height_map, next_move);
        threats |= count_threats(updated_pieces, updated_height_map) << index!(col);
    }

    if best_moves.len() > 0 {
        return best_moves
    }

    let heuristic_move_order = sort_by_threats(threats);
    let mut max_eval = MIN_EVAL;

    for (col, next_move) in next_moves(heuristic_move_order, state.height_map) {
        let mut eval = -evaluate_position(
            state.opp_pieces,
            update_pieces!(state.curr_pieces, next_move),
            update_height_map!(state.height_map, next_move),
            state.moves_made + 1,
            -max_eval - 1,
            -max_eval + 1,
            caches,
            pos
        );

        if eval > max_eval {
            eval = -evaluate_position(
                state.opp_pieces,
                update_pieces!(state.curr_pieces, next_move),
                update_height_map!(state.height_map, next_move),
                state.moves_made + 1,
                MIN_EVAL,
                -eval,
                caches,
                pos
            );

            best_moves = vec![col];
            max_eval = eval;
        } else if eval == max_eval {
            best_moves.push(col);
        }

        println!("Col: {col}, Eval: {eval}");
    }

    best_moves
}
