use crate::database::generate_database;
use crate::error::Result;
use crate::state::State;
use std::time::Instant;

mod engine;
mod threats;
mod state;
mod caches;
mod worker_threads;
mod error;
mod database;

fn main() -> Result<()> {
    let time = Instant::now();

    let pos = generate_database(0, 1)?;

    println!("Pos: {pos}");
    println!("Time: {:?}", time.elapsed());

    Ok(())
}
