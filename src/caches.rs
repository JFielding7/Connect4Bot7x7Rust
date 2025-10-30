use std::sync::Arc;
use dashmap::DashMap;

pub const CACHE_VALUE_SHIFT: u8 = 56;
pub const BEGINNING_GAME_CACHE_DEPTH: i8 = 25;
pub const CACHE_SIZE: usize = (1 << 19) + 1;

pub struct StateCaches {
    pub beg_game_lower_bound_cache: Arc<DashMap<u64, i8>>,
    pub beg_game_upper_bound_cache: Arc<DashMap<u64, i8>>,
    pub end_game_lower_bound_cache: Vec<u64>,
    pub end_game_upper_bound_cache: Vec<u64>,
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
macro_rules! create_cache_entry {
    ($state:expr, $bound:expr) => {
        $state | ((($bound + MAX_PLAYER_MOVES) as u64) << CACHE_VALUE_SHIFT)
    };
}

#[macro_export]
macro_rules! cache_get {
    ($state:expr, $moves_made:expr, $cache_index:expr, $beg_game_cache:expr, $end_game_cache:expr, $default_bound:expr) => {{
        if $moves_made <= BEGINNING_GAME_CACHE_DEPTH {
            if let Some(cache_bound) = $beg_game_cache.get(&$state) {
                cache_bound.value().clone()
            } else {
                $default_bound
            }
        } else {
            let cache_entry = $end_game_cache[$cache_index];

            if (cache_entry & BOARD_MASK) == $state {
                get_cache_entry_eval!(cache_entry)
            } else {
                $default_bound
            }
        }
    }};
}

#[macro_export]
macro_rules! cache_get_lower_bound {
    ($state:expr, $moves_made:expr, $cache_index:expr, $caches:expr) => {{
        cache_get!(
            $state,
            $moves_made,
            $cache_index,
            $caches.beg_game_lower_bound_cache,
            $caches.end_game_lower_bound_cache,
            MIN_EVAL
        )
    }};
}

#[macro_export]
macro_rules! cache_get_upper_bound {
    ($state:expr, $moves_made:expr, $cache_index:expr, $caches:expr) => {{
        cache_get!(
            $state,
            $moves_made,
            $cache_index,
            $caches.beg_game_upper_bound_cache,
            $caches.end_game_upper_bound_cache,
            MAX_EVAL
        )
    }};
}

#[macro_export]
macro_rules! cache_put {
    ($bound:expr, $state:expr, $moves_made:expr, $cache_index:expr, $beg_game_cache:expr, $end_game_cache:expr, $cmp:expr) => {{
        if $moves_made > BEGINNING_GAME_CACHE_DEPTH {
            $end_game_cache[$cache_index] = create_cache_entry!($state, $bound);
        } else {
            $beg_game_cache.entry($state)
                .and_modify(|entry| *entry = $cmp(*entry, $bound))
                .or_insert($bound);
        }
    }};
}

#[macro_export]
macro_rules! cache_put_lower_bound {
    ($bound:expr, $state:expr, $moves_made:expr, $cache_index:expr, $caches:expr) => {{
        cache_put!(
            $bound,
            $state,
            $moves_made,
            $cache_index,
            $caches.beg_game_lower_bound_cache,
            $caches.end_game_lower_bound_cache,
            max
        )
    }};
}

#[macro_export]
macro_rules! cache_put_upper_bound {
    ($bound:expr, $state:expr, $moves_made:expr, $cache_index:expr, $caches:expr) => {{
        cache_put!(
            $bound,
            $state,
            $moves_made,
            $cache_index,
            $caches.beg_game_upper_bound_cache,
            $caches.end_game_upper_bound_cache,
            min
        )
    }};
}
