use crate::engine::evaluate_position;
use crate::state::State;
use std::time::Instant;
use std::io;

mod engine;
mod threats;
mod state;
mod caches;
mod worker_threads;


fn main() -> io::Result<()> {
    let board = vec![
        "   X   ",
        "   O   ",
        "   X   ",
        "   O   ",
        "   X   ",
        "   O   ",
        "   X   ",
    ];

    let state = State::encode(board);

    println!("{}", state);

    let time = Instant::now();

    let mut pos = 0;
    let eval = evaluate_position(state, &mut pos);

    println!("Eval: {}\nPos: {}", eval.unwrap(), pos);
    println!("Time: {:?}", time.elapsed());

    Ok(())
}
