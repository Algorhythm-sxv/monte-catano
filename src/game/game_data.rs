use rand::{RngExt, SeedableRng, rngs::SmallRng};

use crate::{game::*, graph::Action};

pub type GameRng = SmallRng;

/// Represents a game in progress or finished
#[derive(Clone, Debug)]
pub struct Game {
    board: Board,
    actions: Vec<Action>,
    state: GameState,
    rng: GameRng,
}

impl Game {
    pub fn new_random(players: u8, seed: u64) -> Self {
        assert!((2..=4).contains(&players), "must have 2-4 players");
        let mut rng = SmallRng::seed_from_u64(seed);
        let board = Board::new_random_fair(players, &mut rng);
        Self {
            board,
            actions: vec![],
            state: GameState::new(&board),
            rng,
        }
    }

    pub fn new_from_board(board: Board, seed: u64) -> Self {
        let rng = GameRng::seed_from_u64(seed);
        Self {
            board,
            actions: vec![],
            state: GameState::new(&board),
            rng,
        }
    }

    pub fn print_board(&self) {
        println!("{}", self.board);
        println!("{}", self.board.cli_string());
    }

    pub fn set_board_state(&mut self, board: Board, state: GameState) {
        self.board = board;
        self.state = state;
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn current_state(&self) -> &GameState {
        &self.state
    }

    pub fn rng(&mut self) -> &mut SmallRng {
        &mut self.rng
    }

    pub fn last_action(&self) -> Action {
        *self.actions.last().unwrap_or(&Action::NONE)
    }

    pub fn scores(&self) -> [u8; 4] {
        let state = self.current_state();
        [
            state.players.first().map(|p| p.vps).unwrap_or_default(),
            state.players.get(1).map(|p| p.vps).unwrap_or_default(),
            state.players.get(2).map(|p| p.vps).unwrap_or_default(),
            state.players.get(3).map(|p| p.vps).unwrap_or_default(),
        ]
    }

    pub fn is_terminal(&self) -> bool {
        self.current_state().is_terminal()
    }

    pub fn apply_action(&mut self, action: Action) -> Action {
        let action = self.state.apply_action(&self.board, action, &mut self.rng);
        self.actions.push(action);
        action
    }

    pub fn roll_2d6(&mut self) -> usize {
        self.rng.random_range(1..=6) + self.rng.random_range(1..=6)
    }

    pub fn simulate(&mut self, mut state: GameState, mut last_action: Action) -> Player {
        if state.is_initial() {
            while state.is_initial() {
                let mut actions = if state.is_initial_settle() {
                    state.generate_initial_settles()
                } else {
                    state.generate_initial_roads()
                };
                let action = actions
                    .select_random_untried_initial_action(&mut self.rng, state.is_initial_settle());
                state.apply_action(&self.board, action, &mut self.rng);
                last_action = action;
            }
            // first roll
            let roll = self.roll_2d6();
            state.apply_action(&self.board, Action::Roll(roll as u8), &mut self.rng);
            if roll == 7 {
                last_action = Action::Roll(7);
            }
            debug_assert!(state.current_player == 0);
        }
        while !state.is_terminal() {
            let mut actions = state.generate_actions(&self.board, last_action);
            let action = actions.select_random_untried_action(&mut self.rng, last_action);
            state.apply_action(&self.board, action, &mut self.rng);
            last_action = action;
            if action == Action::EndTurn {
                let roll = self.rng.random_range(1..=6) + self.rng.random_range(1..=6);
                state.apply_action(&self.board, Action::Roll(roll as u8), &mut self.rng);
                if roll == 7 {
                    let mut robbers = state.generate_robber_moves();
                    let robber = robbers.select_random_untried_robber_action(&mut self.rng);
                    state.apply_action(&self.board, robber, &mut self.rng);
                    if let Action::MoveRobber(spot) = robber {
                        let mut steals = state.generate_steals(spot);
                        let steal = steals.select_random_untried_steal_action(&mut self.rng);
                        state.apply_action(&self.board, steal, &mut self.rng);
                    }
                }
            }
        }
        Player::from(
            state
                .players
                .iter()
                .position(|p| p.vps >= 10)
                .expect("no winner in terminal position") as u8,
        )
    }
}
