use crate::database::generate_database;
use crate::error::Result;
use crate::state::State;
use std::time::Instant;
use crate::worker_threads::DEFAULT_NUM_WORKER_THREADS;

mod engine;
mod threats;
mod state;
mod caches;
mod worker_threads;
mod error;
mod database;

fn main() -> Result<()> {
    let time = Instant::now();

    let pos = generate_database(4, DEFAULT_NUM_WORKER_THREADS)?;

    println!("Pos: {pos}");
    println!("Time: {:?}", time.elapsed());

    Ok(())
}
