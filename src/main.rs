use crate::engine::{evaluate_position, StateCaches, MAX_EVAL, MIN_EVAL};
use crate::state::State;
use std::{io, thread};
use std::thread::Thread;

mod engine;
mod threat_sort;
mod state;

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

    let mut game_state = State::encode(board);
    if (game_state.moves_made & 1) == 1 {
        let temp = game_state.curr_pieces;
        game_state.curr_pieces = game_state.opp_pieces;
        game_state.opp_pieces = temp;
    }

    let bitboard = game_state.to_bitboard();
    let state = State::from_bitboard(bitboard);

    println!("{}", state);

    let mut caches = StateCaches::new();
    let mut pos = 0;

    for next_state in state.next_states() {
        let mut thread_caches = StateCaches::new_same_beg_cache(&caches);

        thread::spawn(move || {
            evaluate_position(
                game_state.curr_pieces,
                game_state.opp_pieces,
                game_state.height_map,
                game_state.moves_made,
                MIN_EVAL,
                MAX_EVAL,
                &mut thread_caches,
                &mut 0,
            );

            println!("Helper done!");
        });

    }

    println!("Evaluating");

    let eval = evaluate_position(
        game_state.curr_pieces,
        game_state.opp_pieces,
        game_state.height_map,
        game_state.moves_made,
        MIN_EVAL,
        MAX_EVAL,
        &mut caches,
        &mut pos,
    );

    println!("Eval: {}\nPos: {}", eval, pos);

    Ok(())
}
