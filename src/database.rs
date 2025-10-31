use crate::caches::StateCaches;
use crate::caches::CACHE_VALUE_SHIFT;
use crate::engine::optimal_moves;
use crate::engine::{MAX_PLAYER_MOVES};
use crate::error::Result;
use crate::state::{State, BOARD_MASK};
use crate::worker_threads::{spawn_database_generator_worker_threads, WorkerThreadHandler};
use crate::{create_cache_entry, get_cache_entry_eval, get_cache_entry_state};
use dashmap::DashMap;
use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io;
use std::io::{BufWriter, Read, Write};
use std::sync::{Arc, Mutex};


const LOWER_BOUND_DATABASE_NAME: &str = "lower_bound_database.bin";
const UPPER_BOUND_DATABASE_NAME: &str = "upper_bound_database.bin";


fn read_database_from_file(filename: &str, cache: Arc<DashMap<u64, i8>>) -> io::Result<()> {
    let mut file = File::open(filename)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let entries: Vec<u64> = buffer
        .chunks_exact(8)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
        .collect();

    for entry in entries {
        let state = get_cache_entry_state!(entry);
        let eval = get_cache_entry_eval!(entry);

        cache.insert(state, eval);
    }

    Ok(())
}

fn read_databases_into_caches(caches: &StateCaches) -> io::Result<()> {
    read_database_from_file(LOWER_BOUND_DATABASE_NAME, caches.beg_game_lower_bound_cache.clone())?;
    read_database_from_file(UPPER_BOUND_DATABASE_NAME, caches.beg_game_upper_bound_cache.clone())?;

    Ok(())
}

fn write_cache_to_file(filename: &str, cache: Arc<DashMap<u64, i8>>) -> io::Result<()> {
    let mut database_entries = Vec::with_capacity(cache.len());

    for entry in cache.iter() {
        let (state, bound) = entry.pair().clone();
        database_entries.push(create_cache_entry!(state, bound));
    }

    let mut bytes = Vec::with_capacity(database_entries.len() << 3);

    for &entry in &database_entries {
        bytes.extend_from_slice(&entry.to_le_bytes());
    }

    let mut writer = BufWriter::new(File::create(filename)?);
    writer.write_all(&bytes)?;
    writer.flush()?;

    Ok(())
}

fn write_caches_to_databases(caches: StateCaches) -> io::Result<()> {

    write_cache_to_file(LOWER_BOUND_DATABASE_NAME, caches.beg_game_lower_bound_cache)?;
    write_cache_to_file(UPPER_BOUND_DATABASE_NAME, caches.beg_game_upper_bound_cache)?;

    Ok(())
}

fn generate_optimal_reachable_states(
    state: State,
    caches: &mut StateCaches,
    depth: usize,
    seen: &mut HashSet<u64>,
    possible_states: &mut VecDeque<State>,
) -> Result<()> {
    let state_bitboard = state.to_bitboard();

    if seen.insert(state_bitboard) {
        if depth == 0 {
            possible_states.push_back(state);
            return Ok(());
        }

        let (_, best_moves) = optimal_moves(&state, caches, &mut 0)?;

        for best_move in best_moves {
            for next_state in state.play_move(best_move).next_states() {
                generate_optimal_reachable_states(next_state, caches, depth - 1, seen, possible_states)?
            }
        }
    }

    Ok(())
}

pub fn generate_database(depth: usize, num_workers: usize) -> Result<usize> {
    let mut caches = StateCaches::new();
    read_databases_into_caches(&caches)?;

    let start = State::start_state();

    let mut seen = HashSet::new();
    let mut possible_states: VecDeque<State> = VecDeque::new();

    if (depth & 1) == 0 {
        generate_optimal_reachable_states(start, &mut caches, depth >> 1, &mut seen, &mut possible_states)?;
    } else {
        for next_state in start.next_states() {
            generate_optimal_reachable_states(next_state, &mut caches, depth >> 1, &mut seen, &mut possible_states)?;
        }
    }

    println!("Possible States: {}", possible_states.len());

    let states = Arc::new(Mutex::new(possible_states));
    let worker_handlers: Vec<WorkerThreadHandler> = spawn_database_generator_worker_threads(
        num_workers, states, &caches);

    let mut pos = 0;

    for handler in worker_handlers {
        pos += handler.join()?;
    }

    write_caches_to_databases(caches)?;

    Ok(pos)
}
