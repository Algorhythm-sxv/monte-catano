use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        atomic::Ordering,
        mpsc::{Sender, channel},
    },
    thread,
};

use rand::Rng;

use crate::{game::Game, graph::Action, mcts::Mcts, signals::STOP_SEARCH};

#[derive(Copy, Clone)]
struct SprtConfig {
    pub elo0: f64,
    pub elo1: f64,
    pub alpha: f64,
    pub beta: f64,
}

struct SprtState {
    config: SprtConfig,
    init_wins: u64,
    init_losses: u64,
    quadranomial: [u64; 4],
}

enum SprtResult {
    Continue,
    AcceptH0,
    AcceptH1,
}

impl SprtState {
    fn elo_diff_to_win_probability(elo_diff: f64) -> f64 {
        1.0 / (1.0 + 10f64.powf(-elo_diff / 400.0))
    }

    fn llr(&self) -> f64 {
        let n = self.quadranomial.iter().sum::<u64>() * 2 + self.init_wins + self.init_losses; // each unit in the trinomial represents 2 games
        if n == 0 {
            return 0.0;
        }

        let wins =
            2 * self.quadranomial[3] + self.quadranomial[2] + self.quadranomial[1] + self.init_wins;

        // Generalized LLR: model the data as normally distributed and calculate LLR from observed mean and variance
        let sample_mean = wins as f64 / n as f64;
        let variance = sample_mean * (1.0 - sample_mean); // Bernoulli variance with no draws

        if variance <= f64::EPSILON {
            return 0.0; // all wins or losses, not enough information
        }

        let p0 = Self::elo_diff_to_win_probability(self.config.elo0);
        let p1 = Self::elo_diff_to_win_probability(self.config.elo1);

        let n = n as f64;
        n * (p1 - p0) * (2.0 * sample_mean - p0 - p1) / (2.0 * variance)
    }

    fn check(&self) -> SprtResult {
        let upper = self.upper_llr_threshold();
        let lower = self.lower_llr_threshold();
        let llr = self.llr();
        if llr >= upper {
            SprtResult::AcceptH1
        } else if llr <= lower {
            SprtResult::AcceptH0
        } else {
            SprtResult::Continue
        }
    }

    fn upper_llr_threshold(&self) -> f64 {
        ((1.0 - self.config.beta) / self.config.alpha).ln()
    }

    fn lower_llr_threshold(&self) -> f64 {
        (self.config.beta / (1.0 - self.config.alpha)).ln()
    }

    fn record_game_pair(&mut self, wins: (bool, bool)) {
        self.quadranomial[match wins {
            (true, true) => 3,
            (false, true) => 2,
            (true, false) => 1,
            (false, false) => 0,
        }] += 1;
    }
}

pub fn sprt(exe: PathBuf, threads: u16, playouts: u64, init_wins: u64, init_losses: u64) {
    let config = SprtConfig {
        elo0: 0.0,
        elo1: 10.0,
        alpha: 0.05,
        beta: 0.05,
    };
    let mut state = SprtState {
        config,
        init_wins,
        init_losses,
        quadranomial: [0; 4],
    };

    let (tx, rx) = channel();
    let mut workers = vec![];

    println!(
        "Starting SPRT: elo0: {}, elo1: {}, alpha: {}, beta: {}",
        config.elo0, config.elo1, config.alpha, config.beta
    );

    STOP_SEARCH.store(false, Ordering::Relaxed);
    for _ in 0..threads {
        let tx = tx.clone();
        let exe = exe.clone();
        workers.push(thread::spawn(move || sprt_worker(tx, exe, playouts)))
    }

    let result = loop {
        let wins = rx.recv().unwrap();
        state.record_game_pair(wins);
        let n = 2 * state.quadranomial.iter().sum::<u64>() + state.init_wins + state.init_losses;
        println!(
            "Games: {n}, W/L: {}/{}, Quad: {:?}, LLR: {:.2} [{:.2} {:.2}]",
            2 * state.quadranomial[3]
                + state.quadranomial[2]
                + state.quadranomial[1]
                + state.init_wins,
            2 * state.quadranomial[0]
                + state.quadranomial[1]
                + state.quadranomial[2]
                + state.init_losses,
            state.quadranomial,
            state.llr(),
            state.upper_llr_threshold(),
            state.lower_llr_threshold(),
        );
        match state.check() {
            SprtResult::Continue => continue,
            other => break other,
        }
    };

    STOP_SEARCH.store(true, Ordering::Relaxed);
    for w in workers {
        let _ = w.join();
    }

    match result {
        SprtResult::Continue => unreachable!(),
        SprtResult::AcceptH0 => println!("H0 accepted"),
        SprtResult::AcceptH1 => println!("H1 accepted"),
    }
}

fn sprt_worker(tx: Sender<(bool, bool)>, exe: PathBuf, playouts: u64) {
    let mut opponent = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("other engine failed to start");
    let mut opponent_stdin = opponent.stdin.take().unwrap();
    let mut opponent_stdout = BufReader::new(opponent.stdout.take().unwrap());
    let mut func = || -> anyhow::Result<()> {
        writeln!(opponent_stdin, "init")?;
        let mut reply = String::new();
        opponent_stdout.read_line(&mut reply)?;

        while !STOP_SEARCH.load(Ordering::Relaxed) {
            let seed = rand::rng().next_u64();

            // game 1, local is P0
            let game = Game::new_random(2, seed);
            writeln!(opponent_stdin, "newgame players 2 seed {seed}")?;
            writeln!(opponent_stdin, "isready")?;
            opponent_stdout.read_line(&mut reply)?;

            let game_1 = run_game(
                game.clone(),
                0,
                &mut opponent_stdin,
                &mut opponent_stdout,
                playouts,
            )?;
            let result1 = game_1.scores()[0] >= 10;

            // game 2: local is P1
            writeln!(opponent_stdin, "newgame players 2 seed {seed}")?;
            writeln!(opponent_stdin, "isready")?;
            opponent_stdout.read_line(&mut reply)?;

            let game_2 = run_game(
                game.clone(),
                1,
                &mut opponent_stdin,
                &mut opponent_stdout,
                playouts,
            )?;
            let result2 = game_2.scores()[1] >= 10;
            tx.send((result1, result2))?;
        }

        writeln!(opponent_stdin, "quit")?;
        Ok(())
    };

    let _ = func();
    opponent.wait().unwrap();
}

fn run_game(
    mut game: Game,
    local_player: u8,
    opponent_stdin: &mut ChildStdin,
    opponent_stdout: &mut BufReader<ChildStdout>,
    playouts: u64,
) -> anyhow::Result<Game> {
    let mut reply = String::new();
    'game: while !game.is_terminal() {
        // roll if necessary
        if !game.current_state().is_initial() {
            let roll = game.roll_2d6() as u8;
            let determined = game.apply_action(Action::Roll(roll));
            writeln!(
                opponent_stdin,
                "apply {}",
                ron::ser::to_string(&determined)?
            )?;
            opponent_stdout.read_line(&mut reply)?;
        }
        while game.current_state().current_player == local_player {
            let mut mcts = Mcts::new(game.clone());
            for _ in 0..playouts {
                mcts.playout();
            }
            let p0_best = mcts.best_move();
            let determined = game.apply_action(p0_best);
            writeln!(
                opponent_stdin,
                "apply {}",
                ron::ser::to_string(&determined)?
            )?;
            opponent_stdout.read_line(&mut reply)?;

            if game.is_terminal() {
                break 'game;
            }
        }

        // roll if necessary
        if !game.current_state().is_initial() {
            let roll = game.roll_2d6() as u8;
            let determined = game.apply_action(Action::Roll(roll));
            writeln!(
                opponent_stdin,
                "apply {}",
                ron::ser::to_string(&determined)?
            )?;
            opponent_stdout.read_line(&mut reply)?;
        }
        while game.current_state().current_player != local_player {
            // writeln!(
            //     opponent_stdin,
            //     "position {} state {}",
            //     game.board().cli_string(),
            //     ron::ser::to_string(game.current_state())?
            // )?;
            writeln!(opponent_stdin, "isready")?;
            opponent_stdout.read_line(&mut reply)?;
            writeln!(opponent_stdin, "go playouts {playouts}")?;
            reply.clear();
            opponent_stdout.read_line(&mut reply)?;

            let p1_best = reply.split(' ').nth(1).map(ron::de::from_str).unwrap()?;
            let determined = game.apply_action(p1_best);
            writeln!(
                opponent_stdin,
                "apply {}",
                ron::ser::to_string(&determined)?
            )?;
            opponent_stdout.read_line(&mut reply)?;

            // verify state is the same
            #[cfg(debug_assertions)]
            {
                writeln!(opponent_stdin, "print")?;
                reply.clear();
                opponent_stdout.read_line(&mut reply)?;

                let parts = reply.trim().split(' ').collect::<Vec<_>>();
                debug_assert!(
                    parts[1] == ron::ser::to_string(&game.last_action())?,
                    "Action mismatch after {determined:?}: {} != {:?}",
                    parts[1],
                    game.last_action()
                );
                debug_assert!(
                    parts[3] == ron::ser::to_string(game.current_state())?,
                    "State mismatch after {determined:?}:\n{}\n!=\n{}",
                    parts[3],
                    ron::ser::to_string(game.current_state())?
                );
            }

            if game.is_terminal() {
                break 'game;
            }
        }
    }
    Ok(game)
}
