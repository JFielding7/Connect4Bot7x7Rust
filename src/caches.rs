use std::cmp::{max, min};
use std::sync::Arc;
use dashmap::DashMap;
use crate::engine::*;
use crate::state::*;


pub const CACHE_VALUE_SHIFT: u8 = 56;
pub const BEGINNING_GAME_CACHE_DEPTH: i8 = 24;
pub const CACHE_SIZE: usize = (1 << 19) + 1;


pub struct StateCaches {
    pub beg_game_lower_bound_cache: Arc<DashMap<u64, i8>>,
    pub beg_game_upper_bound_cache: Arc<DashMap<u64, i8>>,
    pub end_game_lower_bound_cache: Vec<u64>,
    pub end_game_upper_bound_cache: Vec<u64>,
}


#[macro_export]
macro_rules! cache_index {
    ($state:expr) => {
        $state as usize % CACHE_SIZE
    };
}

#[macro_export]
macro_rules! get_cache_entry_eval {
    ($cache_entry:expr) => {
        ($cache_entry >> CACHE_VALUE_SHIFT) as i8 - MAX_PLAYER_MOVES
    }
}

#[macro_export]
macro_rules! get_cache_entry_state {
    ($cache_entry:expr) => {
        ($cache_entry & BOARD_MASK)
    }
}

#[macro_export]
macro_rules! create_cache_entry {
    ($state:expr, $bound:expr) => {
        $state | ((($bound + MAX_PLAYER_MOVES) as u64) << CACHE_VALUE_SHIFT)
    };
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

    pub fn with_same_beg_caches(&self) -> Self {
        Self::from_beg_caches(
            self.beg_game_lower_bound_cache.clone(),
            self.beg_game_upper_bound_cache.clone()
        )
    }

    pub fn get_lower_bound(&self, state: u64, moves_made: i8, cache_index: usize) -> i8 {
        cache_get(
            state,
            moves_made,
            cache_index,
            &self.beg_game_lower_bound_cache,
            &self.end_game_lower_bound_cache,
            MIN_EVAL
        )
    }

    pub fn get_upper_bound(&self, state: u64, moves_made: i8, cache_index: usize) -> i8 {
        cache_get(
            state,
            moves_made,
            cache_index,
            &self.beg_game_upper_bound_cache,
            &self.end_game_upper_bound_cache,
            MAX_EVAL
        )
    }

    pub fn put_beg_game_lower_bound(&self, bound: i8, state: u64) {
        self.beg_game_lower_bound_cache.insert(state, bound);
    }

    pub fn put_lower_bound(&mut self, bound: i8, state: u64, moves_made: i8, cache_index: usize) {
        cache_put(
            bound,
            state,
            moves_made,
            cache_index,
            &self.beg_game_lower_bound_cache,
            &mut self.end_game_lower_bound_cache,
            max
        )
    }

    pub fn put_upper_bound(&mut self, bound: i8, state: u64, moves_made: i8, cache_index: usize) {
        cache_put(
            bound,
            state,
            moves_made,
            cache_index,
            &self.beg_game_upper_bound_cache,
            &mut self.end_game_upper_bound_cache,
            min
        )
    }
}

fn cache_get(state: u64, moves_made: i8, cache_index: usize, beg_game_cache: &Arc<DashMap<u64, i8>>, end_game_cache: &Vec<u64>, default_bound: i8) -> i8 {
    if moves_made <= BEGINNING_GAME_CACHE_DEPTH {
        if let Some(cache_bound) = beg_game_cache.get(&state) {
            return cache_bound.value().clone()
        }
    } else {
        let cache_entry = end_game_cache[cache_index];

        if get_cache_entry_state!(cache_entry) == state {
            return get_cache_entry_eval!(cache_entry)
        }
    }

    default_bound
}

fn cache_put(bound: i8, state: u64, moves_made: i8, cache_index: usize, beg_game_cache: &Arc<DashMap<u64, i8>>, end_game_cache: &mut Vec<u64>, cmp: fn(i8, i8) -> i8) {
    if moves_made > BEGINNING_GAME_CACHE_DEPTH {
        end_game_cache[cache_index] = create_cache_entry!(state, bound);
    } else {
        beg_game_cache.entry(state)
            .and_modify(|entry| *entry = cmp(*entry, bound))
            .or_insert(bound);
    }
}
