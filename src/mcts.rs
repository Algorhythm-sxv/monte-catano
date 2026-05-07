use crate::{
    game::*,
    graph::{Action, Node, NodeArena, NodeRef},
};

/// MCTS context
pub struct Mcts {
    game: Game,
    board: Board,
    arena: NodeArena,
    root: NodeRef,
}

impl Mcts {
    pub fn new(game: Game) -> Self {
        let mut arena = NodeArena::new();
        let forced_continuation = game.last_action().forced_continuation();
        let root = match forced_continuation {
            Some(Action::MoveRobber(_)) => {
                arena.insert(Node::robber(*game.current_state(), NodeRef::INVALID))
            }
            Some(Action::Steal(_, _)) => {
                let Action::MoveRobber(spot) = game.last_action() else {
                    panic!("invalid action before steal: {:?}", game.last_action())
                };
                arena.insert(Node::steal(*game.current_state(), NodeRef::INVALID, spot))
            }
            Some(Action::YoPResources(_, _)) => {
                arena.insert(Node::yop_resources(*game.current_state(), NodeRef::INVALID))
            }
            Some(Action::MonopolyResource(_)) => arena.insert(Node::monopoly_resource(
                *game.current_state(),
                NodeRef::INVALID,
            )),
            Some(Action::RoadBuild1(_)) => {
                arena.insert(Node::road_build_1(*game.current_state(), NodeRef::INVALID))
            }
            Some(Action::RoadBuild2(_)) => {
                arena.insert(Node::road_build_2(*game.current_state(), NodeRef::INVALID))
            }
            _ => arena.insert(Node::root(*game.current_state(), game.board())),
        };

        let board = *game.board();
        Self {
            game,
            board,
            arena,
            root,
        }
    }

    /// Expand a non-terminal leaf node, selecting a new untried action and returning the created child node
    fn expand(&mut self, node_ref: NodeRef) -> NodeRef {
        let mut node = self.arena[node_ref];

        let action = node.select_untried_action(self.game.rng());

        let mut new_state = node.state;
        new_state.apply_action(&self.board, action, self.game.rng());

        let mut chance_node = NodeRef::INVALID;
        // the final initial road needs roll nodes afterwards
        let final_initial_road =
            matches!(action, Action::InitialRoad(_)) && !new_state.is_initial();
        if final_initial_road {
            chance_node = self.arena.insert(
                Node::initial_final(new_state, node_ref, action).with_sibling(node.first_child),
            );
            self.create_roll_nodes(chance_node, new_state);
            node.first_child = chance_node;
        }

        // no children yet means we need to make the special node for end turn (outside of forced sequences)
        if !node.first_child.is_valid()
            && !new_state.is_initial()
            && !node.parent_action.has_forced_continuation()
        {
            // after ending the turn the next player's only available action is to roll the dice, so we can automatically expand these nodes
            let end_node = self.arena.insert(Node::end_turn(new_state, node_ref));
            self.create_roll_nodes(end_node, new_state);
            node.first_child = end_node;
        }

        // buying a dev card needs random nodes afterwards
        if action == Action::BuyDevCard(Unknown) {
            // don't use the applied state for the buy node, the children will have the correct states
            chance_node = self
                .arena
                .insert(Node::buy_dev_card(node.state, node_ref).with_sibling(node.first_child));

            self.create_card_nodes(chance_node, node.state);
            node.first_child = chance_node;
        }

        let new_node = match (action, final_initial_road) {
            // if the action taken was to end the turn, we need to move to the new state and not make another new child
            (Action::EndTurn, _) => {
                let mut end_node = node.first_child;
                loop {
                    if self.arena[end_node].is_end_turn() {
                        break;
                    } else {
                        end_node = self.arena[end_node].next_sibling;
                    }
                }
                self.arena[end_node].choose_child(self.game.rng())
            }
            // if this is a chance node, randomly select a child
            (Action::BuyDevCard(Unknown), _) | (Action::InitialRoad(_), true) => {
                self.arena[chance_node].choose_child(self.game.rng())
            }
            _ => {
                // if we're trying something else, make a new child for it
                let new_node = self.arena.insert(
                    Node::with_parent(new_state, self.game.board(), node_ref, action)
                        .with_sibling(node.first_child),
                );
                node.first_child = new_node;
                new_node
            }
        };
        self.arena[node_ref] = node;

        new_node
    }

    fn create_roll_nodes(&mut self, parent: NodeRef, new_state: GameState) {
        // create 11 children for the rolls 2-12
        let mut new_state_2 = new_state;
        new_state_2.roll_resources(self.game.board(), 2);
        let first_child = self.arena.insert(Node::with_parent(
            new_state_2,
            self.game.board(),
            parent,
            Action::NONE,
        ));
        for n in 3..=12 {
            let mut new_state_n = new_state;
            new_state_n.apply_action(&self.board, Action::Roll(n), self.game.rng());
            if n == 7 {
                self.arena.insert(Node::robber(new_state_n, parent));
            } else {
                self.arena.insert(Node::with_parent(
                    new_state_n,
                    &self.board,
                    parent,
                    Action::Roll(n),
                ));
            }
        }
        self.arena[parent].first_child = first_child;
    }

    fn create_card_nodes(&mut self, parent: NodeRef, state: GameState) {
        // create 5 children for the 5 types of dev cards
        let mut first_child = NodeRef::INVALID;
        for c in 0..5 {
            let mut new_state = state;
            let action = Action::BuyDevCard(DevCard::from(c as u8));
            // only change the state if buying the dev card is possible
            if state.dev_card_deck[c] > 0 {
                new_state.apply_action(&self.board, action, self.game.rng());
            }
            let child =
                self.arena
                    .insert(Node::with_parent(new_state, &self.board, parent, action));
            if !first_child.is_valid() {
                first_child = child
            }
        }
        self.arena[parent].first_child = first_child;
    }

    /// Perform a single MCTS playout from the root
    pub fn playout(&mut self) -> Player {
        // Selection: select children until a nonterminal leaf (untried action) is reached
        let mut node_ref = self.root;
        let mut node = &self.arena[node_ref];
        while node.available_actions.is_empty() && !node.state.is_terminal() {
            // randomly select a child from chance nodes
            if node.is_chance_node() {
                node_ref = node.choose_child(self.game.rng());
                node = &self.arena[node_ref];
                continue;
            }
            let node_player = node.state.player() as usize;
            let mut best_child = node.first_child;
            let mut current_child = &self.arena[best_child];
            let mut best_uct = current_child.uct(node_player, node.visits);

            // loop through the children to find the one with the best UCT
            loop {
                let next_child = current_child.next_sibling;
                if !next_child.is_valid() {
                    // no more children
                    break;
                }
                current_child = &self.arena[next_child];
                let uct = current_child.uct(node_player, node.visits);
                if uct > best_uct {
                    best_uct = uct;
                    best_child = next_child;
                }
            }

            // move down the tree to the best child
            node_ref = best_child;
            node = &self.arena[node_ref];
        }

        // Expansion: select an untried child from the leaf and create the new node
        let new = if !node.state.is_terminal() {
            self.expand(node_ref)
        } else {
            // already terminal nodes can't have children
            node_ref
        };

        // Simulation: playout randomly until a terminal state is reached
        let winner = self
            .game
            .simulate(self.arena[new].state, self.arena[new].parent_action);

        // Backpropagation: go back up the tree and update visits and wins
        let mut node_ref = new;
        while node_ref.is_valid() {
            let node = &mut self.arena[node_ref];
            node.visits += 1;
            node.wins[winner] += 1;
            node_ref = node.parent;
        }

        winner
    }

    pub fn list_moves(&self) {
        let mut moves = Vec::new();
        let mut child = self.arena[self.root].first_child;
        let root_player = self.game.current_state().player() as usize;
        while child.is_valid() {
            let node = self.arena[child];
            moves.push((
                self.game.current_state().player(),
                node.parent_action,
                node.wins[root_player],
                node.visits,
                node.uct(root_player, self.arena[self.root].visits),
            ));

            child = node.next_sibling;
        }
        moves.sort_by_key(|m| (m.2 as u64 * 1000) / m.3.max(1) as u64);

        for m in moves.iter().rev() {
            println!(
                "P{} {:?}: {:<3.1}% {}, UCT {:<3.1}",
                m.0,
                m.1,
                100.0 * m.2 as f32 / m.3 as f32,
                m.3,
                m.4
            );
        }
    }

    /// Select the best move from the root, detemined by the most visits
    pub fn best_move(&self) -> Action {
        let mut best_child = self.arena[self.root].first_child;
        let mut current_child = self.arena[best_child];
        let mut best_visits = current_child.visits;
        loop {
            let next = current_child.next_sibling;
            if !next.is_valid() {
                break;
            }
            current_child = self.arena[next];
            let current_visits = current_child.visits;
            if current_visits > best_visits {
                best_visits = current_visits;
                best_child = next;
            }
        }
        self.arena[best_child].parent_action
    }
}
