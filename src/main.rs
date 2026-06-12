use std::{io::Write, sync::atomic::Ordering, thread, time::Duration};

use clap::Parser;

use crate::{
    cli::{Args, Commands},
    game::Game,
    signals::*,
    sprt::sprt,
    uci::{search_thread, uci_loop},
};

mod cli;
mod game;
mod graph;
mod mcts;
mod signals;
mod sprt;
mod uci;

fn main() {
    let args = Args::parse();
    match args.command {
        Some(Commands::Play { seed }) => {
            let seed = seed.unwrap_or(12345678);
            let mut game = Game::new_random(4, seed);
            game.print_board();

            while !game.is_terminal() {
                PLAYOUTS.store(0, Ordering::Relaxed);
                STOP_SEARCH.store(false, Ordering::Relaxed);

                let best_move = if let Some(m) = game.single_action() {
                    m
                } else {
                    let new = game.clone();
                    let search = thread::spawn(move || search_thread(new));
                    let mut prev_playouts = 0;

                    for _ in 0..5 {
                        thread::sleep(Duration::from_secs(1));
                        let playouts = PLAYOUTS.load(Ordering::Relaxed);
                        let diff = playouts - prev_playouts;
                        prev_playouts = playouts;
                        print!("\rPlayouts: {}, P/s: {}             ", playouts, diff);
                        let _ = std::io::stdout().flush();
                    }
                    println!();

                    STOP_SEARCH.store(true, Ordering::Relaxed);
                    let mcts = search.join().unwrap();

                    mcts.list_moves();
                    mcts.best_move()
                };

                let determined_action = game.apply_action(best_move);

                println!("P{} {:?} -> {:?}\n", game.current_state().current_player, best_move, determined_action);
            }

            println!("{:?}", game.scores());
        }

        Some(Commands::Sprt {
            exe,
            threads,
            playouts,
            init_wins,
            init_losses,
        }) => sprt(exe, threads, playouts, init_wins, init_losses),

        Some(Commands::HTest { seed }) => {
            let mut game = Game::new_random(2, seed);
            game.heuristic_test();
        }

        _ => uci_loop(),
    }
}
