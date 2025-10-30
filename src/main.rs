use crate::engine::{best_moves};
use crate::state::State;
use std::time::Instant;
use crate::caches::StateCaches;
use crate::error::Result;

mod engine;
mod threats;
mod state;
mod caches;
mod worker_threads;
mod error;

fn main() -> Result<()> {
    let board = vec![
        "   X   ",
        "  XO   ",
        "  OX   ",
        "  XO   ",
        "  OX   ",
        "  XO   ",
        "  OX   ",
    ];

    let state = State::encode(board);

    println!("{}", state);

    let time = Instant::now();

    let mut pos = 0;
    let mut caches = StateCaches::new();
    let best_moves = best_moves(state, &mut caches, &mut pos)?;

    println!("Best Moves: {best_moves:?}");
    println!("Pos: {pos}");
    println!("Time: {:?}", time.elapsed());

    Ok(())
}
