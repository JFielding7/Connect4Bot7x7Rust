use crate::caches::StateCaches;
use crate::engine::{evaluate_position_rec, MAX_EVAL, MIN_EVAL};
use crate::state::State;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::thread::Result;

const MAX_WORKER_THREADS: i32 = 31;

pub struct WorkerThreadHandler {
    pub terminate_flag: Arc<AtomicBool>,
    pub join_handle: JoinHandle<()>,
}

impl WorkerThreadHandler {
    pub fn terminate(&self) {
        self.terminate_flag.store(true, Ordering::Relaxed);
    }

    pub fn join(self) -> Result<()> {
        self.join_handle.join()
    }
}

fn worker_thread(game_state: State, caches: &StateCaches) -> WorkerThreadHandler {
    let mut thread_caches = StateCaches::new_same_beg_cache(caches);

    let terminate_flag = Arc::new(AtomicBool::new(false));
    let terminate_flag_clone = terminate_flag.clone();

    let join_handle = thread::spawn(move || {
        evaluate_position_rec(
            game_state.curr_pieces,
            game_state.opp_pieces,
            game_state.height_map,
            game_state.moves_made,
            MIN_EVAL,
            MAX_EVAL,
            &mut thread_caches,
            &terminate_flag_clone,
            &mut 0,
        );
    });

    WorkerThreadHandler {
        join_handle,
        terminate_flag,
    }
}

pub fn spawn_worker_threads(game_state: State, caches: &StateCaches) -> Vec<WorkerThreadHandler> {
    let mut handles = vec![];

    let states = game_state.clone().generate_states(2);

    let mut handler_count = MAX_WORKER_THREADS;

    for bitboard in states {
        if handler_count == 0 {
            break
        }

        let state = State::from_bitboard(bitboard);
        if state == game_state {
            continue;
        }

        handles.push(worker_thread(state, caches));
        handler_count -= 1;
    };

    println!("Worker Thread Count: {}", handles.len());

    handles
}
