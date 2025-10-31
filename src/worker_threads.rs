use crate::caches::StateCaches;
use crate::engine::{evaluate_position_rec, optimal_moves_with_workers, MAX_EVAL, MIN_EVAL};
use crate::error::{Connect4Error, Result};
use crate::state::State;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;


pub const DEFAULT_NUM_WORKER_THREADS: usize = 30;

pub struct WorkerThreadHandler {
    terminate_flag: Arc<AtomicBool>,
    join_handle: JoinHandle<Result<usize>>,
}

impl WorkerThreadHandler {
    pub fn terminate(&self) {
        self.terminate_flag.store(true, Ordering::Relaxed);
    }

    pub fn join(self) -> Result<usize> {
        self.join_handle.join().map_err(|_| Connect4Error::WorkerThreadJoinError)?
    }
}

fn evaluate_position_worker_thread(
    game_state: State,
    caches: &StateCaches
) -> WorkerThreadHandler {

    let mut thread_caches = caches.with_same_beg_caches();

    let terminate_flag = Arc::new(AtomicBool::new(false));
    let terminate_flag_clone = terminate_flag.clone();

    let join_handle = thread::spawn(move || {
        println!("Evaluate Position Worker Thread Started");

        let mut pos = 0;

        evaluate_position_rec(
            game_state.curr_pieces,
            game_state.opp_pieces,
            game_state.height_map,
            game_state.moves_made,
            MIN_EVAL,
            MAX_EVAL,
            &mut thread_caches,
            &terminate_flag_clone,
            &mut pos,
        ).ok_or_else(|| Connect4Error::EvaluatePositionError)?;

        Ok(pos)
    });

    WorkerThreadHandler {
        join_handle,
        terminate_flag,
    }
}

pub fn spawn_evaluate_position_worker_threads(
    num_workers: usize,
    game_state: &State,
    caches: &StateCaches
) -> Vec<WorkerThreadHandler> {

    const WORKER_THREAD_DEPTH: usize = 2;
    let states = game_state.generate_states(WORKER_THREAD_DEPTH);

    let mut handlers = vec![];

    for (_, bitboard) in (0..num_workers).zip(states) {
        let state = State::from_bitboard(bitboard);
        if &state == game_state {
            continue;
        }

        handlers.push(evaluate_position_worker_thread(state, caches));
    };

    println!("Worker Thread Count: {}", handlers.len());

    handlers
}

fn database_generator_worker_thread(
    states: Arc<Mutex<VecDeque<State>>>,
    caches: &StateCaches
) -> WorkerThreadHandler {

    let mut thread_caches = caches.with_same_beg_caches();

    let join_handle = thread::spawn(move || {
        println!("Database Generator Worker Thread Started");

        let mut pos = 0;

        loop {
            let state_opt = {
                let mut positions_lock = states.lock().unwrap();
                positions_lock.pop_front()
            };

            match state_opt {
                Some(state) => {
                    let (eval, _) = optimal_moves_with_workers(
                        &state, &mut thread_caches, &mut pos)?;

                    thread_caches.put_beg_game_lower_bound(eval, state.to_bitboard());
                },
                None => break,
            };
        }

        Ok(pos)
    });

    WorkerThreadHandler {
        terminate_flag: Arc::new(AtomicBool::new(false)),
        join_handle,
    }
}

pub fn spawn_database_generator_worker_threads(
    num_workers: usize,
    states: Arc<Mutex<VecDeque<State>>>,
    caches: &StateCaches
) -> Vec<WorkerThreadHandler> {

    (0..num_workers).map(|_| {
        database_generator_worker_thread(states.clone(), &caches)
    }).collect()
}
