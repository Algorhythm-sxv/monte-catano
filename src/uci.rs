use std::{
    io::stdin,
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

use crate::{game::*, mcts::Mcts, signals::*};

pub fn uci_loop() {
    let mut players = 4;
    let mut game = Game::new_random(players, 0);

    let stdin_lines = stdin().lines();
    'command: for line in stdin_lines {
        let line = line.unwrap();
        let mut words = line.split(' ');
        if let Some(cmd) = words.next() {
            let args: Vec<_> = words.collect();
            match cmd {
                // init -> id name <NAME> author <AUTHOR>
                "init" => println!("id 'Monte Catano'"),
                // isready -> readyok
                "isready" => println!("readyok"),
                // newgame players <N> seed <SEED>
                "newgame" => {
                    players = if let Some(text) = args.get(1) {
                        text.parse::<u8>().unwrap_or_else(|_| {
                            println!("infor Error: invalid player count: {text}");
                            4
                        })
                    } else {
                        4
                    };
                    let seed = if let Some(text) = args.get(3) {
                        text.parse::<u64>().unwrap_or_else(|_| {
                            println!("info Error: invalid seed: {text}");
                            0
                        })
                    } else {
                        0
                    };
                    game = Game::new_random(players, seed);
                }
                // position <BOARD_STRING> state <STATE_STRING>
                "position" => {
                    if args.len() < 3 {
                        println!(
                            "info Error: expected at least 3 arguments for 'position' command, got {}",
                            args.len()
                        );
                        continue;
                    }
                    let board_string = args[0];
                    let board = match Board::from_cli_string(players, board_string) {
                        Ok(board) => board,
                        Err(e) => {
                            println!("info Error: invalid board string, {e}: {board_string}");
                            continue;
                        }
                    };
                    match args[1] {
                        "state" => {
                            let state_string = args[2];
                            let Ok(state) = ron::de::from_str(state_string) else {
                                println!("info Error: invalid board state");
                                continue;
                            };
                            game.set_board_state(board, state);
                        }
                        "actions" => {
                            game = Game::new_from_board(board, 0);
                            for action_string in &args[2..] {
                                let Ok(action) = ron::de::from_str(action_string) else {
                                    println!("info Error: invalid action: {action_string}");
                                    continue 'command;
                                };
                                game.apply_action(action);
                            }
                        }
                        other => {
                            println!("info Error: invalid subcommand for 'position': {other}")
                        }
                    }
                }
                // go (movetime <T_MS>, playouts <N>)
                "go" if args.len() == 2 => match args[0] {
                    "movetime" => {
                        let Ok(movetime_ms) = args[1].parse::<u64>() else {
                            println!("info Error: invalid movetime: {}", args[1]);
                            continue;
                        };

                        let game = game.clone();
                        thread::spawn(move || uci_go_thread(game, Some(movetime_ms), None));
                    }
                    "playouts" => {
                        let Ok(playouts) = args[1].parse::<u64>() else {
                            println!("info Error: invalid movetime: {}", args[1]);
                            continue;
                        };

                        let game = game.clone();
                        thread::spawn(move || uci_go_thread(game, None, Some(playouts)));
                    }
                    other => println!("info Error: invalid subcommand for 'go': {other}"),
                },
                // stop
                "stop" => {
                    STOP_SEARCH.store(true, Ordering::Relaxed);
                }
                // apply <ACTION>
                "apply" => {
                    let Some(action_string) = args.first() else {
                        println!("info Error: no action specified");
                        continue;
                    };
                    let Ok(action) = ron::de::from_str(action_string) else {
                        println!("info Error: invalid action: {action_string}");
                        continue;
                    };
                    let determined = game.apply_action(action);
                    println!("determined {}", ron::ser::to_string(&determined).unwrap());
                }
                // quit
                "quit" => {
                    STOP_SEARCH.store(true, Ordering::Relaxed);
                    return;
                }
                _ => {}
            }
        }
    }
}

pub fn uci_go_thread(game: Game, movetime_ms: Option<u64>, playouts: Option<u64>) {
    let new = game.clone();
    let mcts = if let Some(movetime_ms) = movetime_ms {
        let start_time = Instant::now();
        let end_time = start_time + Duration::from_millis(movetime_ms);
        STOP_SEARCH.store(false, Ordering::Relaxed);
        let search = thread::spawn(move || search_thread(new));
        let mut prev_playouts = 0;
        loop {
            // search stopped from elsewhere
            if STOP_SEARCH.load(Ordering::Relaxed) {
                break;
            }
            if (end_time - Instant::now()).as_millis() > 1000 {
                thread::sleep(Duration::from_millis(1000));
                let playouts = PLAYOUTS.load(Ordering::Relaxed);
                let diff = playouts - prev_playouts;
                prev_playouts = playouts;
                println!("info playouts {playouts} pps {diff} ");
            } else {
                thread::sleep(end_time - Instant::now());
                STOP_SEARCH.store(true, Ordering::Relaxed);
                break;
            }
        }
        search.join().unwrap()
    } else {
        let playouts = playouts.unwrap_or(1000);

        let mut mcts = Mcts::new(game);
        for _ in 0..playouts {
            // search stopped from elsewhere
            if STOP_SEARCH.load(Ordering::Relaxed) {
                break;
            }
            mcts.playout();
        }
        mcts
    };
    let best_move = mcts.best_move();
    println!("bestmove {}", ron::ser::to_string(&best_move).unwrap());
}

pub fn search_thread(game: Game) -> Mcts {
    let mut mcts = Mcts::new(game);

    loop {
        if STOP_SEARCH.load(Ordering::Relaxed) {
            break;
        }
        mcts.playout();
        PLAYOUTS.fetch_add(1, Ordering::Relaxed);
    }
    mcts
}
