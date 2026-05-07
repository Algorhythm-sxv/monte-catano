use std::ops::{Index, IndexMut};

use rand::{
    RngExt,
    distr::{Distribution, weighted::WeightedIndex},
};
use serde::{Deserialize, Serialize};

use crate::game::*;

/// An action that a player can perform. Some actions are non-deterministic (e.g. buying a random development card)
/// and are encoded with placeholder values (e.g. [DevCard::Unknown]) until they are _determinized_ on application
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Place a village at the player's nth available settle spot
    Settle(u8),
    /// Place a city on the player's nth placed village
    City(u8),
    /// Place a road on the player's nth available road spot
    Road(u8),
    /// Buy a development card
    BuyDevCard(DevCard), // dev card can't be chosen in normal play
    /// Trade a resource with the bank for another resource
    BankTrade(Resource, Resource),
    /// Play a particular development card
    PlayDevCard(DevCard),
    /// First road placement after playing a Road Builder development card
    RoadBuild1(u8),
    /// Second road placement after playing a Road Builder development card
    RoadBuild2(u8),
    /// Choose resources after playing a Year of Plenty development card
    YoPResources(Resource, Resource),
    /// Choose a resource after playing a Monopoly development card
    MonopolyResource(Resource),
    /// Place an initial settlement at the nth vertex
    InitialSettle(u8),
    /// Place an initial road at the nth edge adjacent to the last initial settlement
    InitialRoad(u8),
    /// Move the robber after rolling a 7 or playing a Knight development card
    MoveRobber(u8),
    /// Steal a resource from an adjacent player after moving the robber
    Steal(u8, Resource), // resource can't be chosen in normal play
    /// End the player's turn
    EndTurn,
    /// Roll the dice for resource distribution/discard after 7
    Roll(u8), // roll can't be chosen in normal play
}

impl Action {
    pub const NONE: Self = Self::Settle(255);

    /// Whether an action must be followed by another type of action e.g. `Roll(7) -> MoveRobber(_)`
    pub fn has_forced_continuation(&self) -> bool {
        self.forced_continuation().is_some()
    }

    /// What is the forced continuation to this action? e.g. `Roll(7) -> MoveRobber(_)
    pub fn forced_continuation(&self) -> Option<Self> {
        match *self {
            Self::Roll(7) | Self::PlayDevCard(Knight) => Some(Self::MoveRobber(0)),
            Self::MoveRobber(_) => Some(Self::Steal(0, Desert)),
            Self::PlayDevCard(YoP) => Some(Self::YoPResources(Desert, Desert)),
            Self::PlayDevCard(Monopoly) => Some(Self::MonopolyResource(Desert)),
            Self::PlayDevCard(RoadBuild) => Some(Self::RoadBuild1(0)),
            Self::RoadBuild1(_) => Some(Self::RoadBuild2(0)),
            _ => None,
        }
    }
}

/// A collection of bitflags storing the untried actions from any particular node
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Actions {
    bitflags: u128,
}

static _SIZE_ASSERT: () = assert!(
    Actions::FLAGS_END < 127,
    "Action bitflags do not fit in u128"
);

impl Actions {
    /// Each player starts with two roads down, then 13 further roads can add one settle spot each
    const SETTLES: u8 = 13;
    /// Each player can have up to 5 villages down at once
    const CITIES: u8 = 5;
    /// The first two roads give up to 4 expansion spots each, then each road adds 1 to the total up to the 14th road
    const ROADS: u8 = 8 + 12;
    /// Only one way to buy a dev card
    const BUYS: u8 = 1;
    /// Only one way to end your turn
    const ENDS: u8 = 1;
    /// 4 different playable dev cards (no VP)
    const PLAYS: u8 = 4;
    /// Each of the 5 resources can be traded for any of the 4 others
    const TRADES: u8 = 5 * 4;

    const SETTLE_START: u8 = 0;
    const CITY_START: u8 = Self::SETTLES;
    const ROAD_START: u8 = Self::CITY_START + Self::CITIES;
    const BUYS_START: u8 = Self::ROAD_START + Self::ROADS;
    const ENDS_START: u8 = Self::BUYS_START + Self::BUYS;
    const PLAYS_START: u8 = Self::ENDS_START + Self::ENDS;
    const TRADES_START: u8 = Self::PLAYS_START + Self::PLAYS;
    const FLAGS_END: u8 = Self::TRADES_START + Self::TRADES;

    pub fn generate_robber_without(spot: u8) -> Self {
        Self {
            bitflags: 0x7FFFF ^ (1 << spot),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn add_initial_settles(&mut self, settle_mask: u128) {
        self.bitflags |= settle_mask;
    }

    pub fn add_initial_roads(&mut self, road_mask: u8) {
        self.bitflags |= road_mask as u128;
    }

    pub fn add_settle(&mut self, spot: usize) {
        debug_assert!(spot <= Self::SETTLES as usize);
        self.bitflags |= 1 << (Self::SETTLE_START as usize + spot);
    }

    pub fn add_city(&mut self, spot: usize) {
        debug_assert!(spot <= Self::CITIES as usize);
        self.bitflags |= 1 << (Self::CITY_START as usize + spot);
    }

    pub fn add_road(&mut self, spot: usize) {
        debug_assert!(spot <= Self::ROADS as usize);
        self.bitflags |= 1 << (Self::ROAD_START as usize + spot);
    }

    pub fn add_buy_dev_card(&mut self) {
        self.bitflags |= 1 << Self::BUYS_START;
    }

    pub fn add_play_dev_card(&mut self, card: usize) {
        self.bitflags |= 1 << (Self::PLAYS_START as usize + card);
    }

    pub fn add_end_turn(&mut self) {
        self.bitflags |= 1 << Self::ENDS_START;
    }

    pub fn add_bank_trades(&mut self, give: Resource) {
        let trades = 0b1111;
        self.bitflags |= trades << (Self::TRADES_START + 4 * give as u8)
    }

    fn choose_random_bit(&mut self, rng: &mut GameRng) -> u8 {
        let set_bits = self.bitflags.count_ones();
        let chosen_index = rng.random_range(0..set_bits);
        let mut bits = self.bitflags;
        for _ in 0..chosen_index {
            bits &= bits - 1;
        }
        let chosen_bit = bits.trailing_zeros() as u8;
        debug_assert!(self.bitflags & (1 << chosen_bit) != 0);
        self.bitflags ^= 1 << chosen_bit;
        chosen_bit
    }

    pub fn select_random_untried_action(
        &mut self,
        rng: &mut GameRng,
        last_action: Action,
    ) -> Action {
        match last_action.forced_continuation() {
            Some(Action::MoveRobber(_)) => self.select_random_untried_robber_action(rng),
            Some(Action::Steal(_, _)) => self.select_random_untried_steal_action(rng),
            Some(Action::YoPResources(_, _)) => self.select_random_untried_yop_resources(rng),
            Some(Action::MonopolyResource(_)) => self.select_random_untried_monopoly_resource(rng),
            Some(road_build @ (Action::RoadBuild1(_) | Action::RoadBuild2(_))) => {
                self.select_random_untried_road_build(road_build, rng)
            }
            _ => self.select_random_untried_normal_action(rng),
        }
    }

    pub fn select_random_untried_initial_action(
        &mut self,
        rng: &mut GameRng,
        settling: bool,
    ) -> Action {
        let chosen_bit = self.choose_random_bit(rng);
        if settling {
            Action::InitialSettle(chosen_bit)
        } else {
            Action::InitialRoad(chosen_bit)
        }
    }

    pub fn select_random_untried_robber_action(&mut self, rng: &mut GameRng) -> Action {
        let chosen_bit = self.choose_random_bit(rng);
        Action::MoveRobber(chosen_bit)
    }

    pub fn select_random_untried_steal_action(&mut self, rng: &mut GameRng) -> Action {
        let chosen_bit = self.choose_random_bit(rng);
        Action::Steal(chosen_bit, Desert) // actual stolen resource will be determined later
    }

    pub fn select_random_untried_yop_resources(&mut self, rng: &mut GameRng) -> Action {
        let chosen_bit = self.choose_random_bit(rng);
        let res1 = Resource::from(chosen_bit / 5);
        let res2 = Resource::from(chosen_bit % 5);
        Action::YoPResources(res1, res2)
    }

    pub fn select_random_untried_monopoly_resource(&mut self, rng: &mut GameRng) -> Action {
        let chosen_bit = self.choose_random_bit(rng);
        Action::MonopolyResource(Resource::from(chosen_bit))
    }

    pub fn select_random_untried_road_build(
        &mut self,
        road_build: Action,
        rng: &mut GameRng,
    ) -> Action {
        let chosen_bit = if self.is_empty() {
            0
        } else {
            self.choose_random_bit(rng)
        };
        match road_build {
            Action::RoadBuild1(_) => Action::RoadBuild1(chosen_bit),
            Action::RoadBuild2(_) => Action::RoadBuild2(chosen_bit),
            _ => unreachable!(),
        }
    }

    pub fn select_random_untried_normal_action(&mut self, rng: &mut GameRng) -> Action {
        let chosen_bit = self.choose_random_bit(rng);
        match chosen_bit {
            Self::SETTLE_START..Self::CITY_START => Action::Settle(chosen_bit),
            Self::CITY_START..Self::ROAD_START => Action::City(chosen_bit - Self::CITY_START),
            Self::ROAD_START..Self::BUYS_START => Action::Road(chosen_bit - Self::ROAD_START),
            Self::BUYS_START..Self::ENDS_START => Action::BuyDevCard(Unknown), // actual dev card will be determined later
            Self::ENDS_START..Self::PLAYS_START => Action::EndTurn,
            Self::PLAYS_START..Self::TRADES_START => {
                Action::PlayDevCard(DevCard::from(chosen_bit - Self::PLAYS_START))
            }
            Self::TRADES_START..Self::FLAGS_END => {
                let give = Resource::from((chosen_bit - Self::TRADES_START) / 4);
                let get_idx = (chosen_bit - Self::TRADES_START) % 4;
                let get = give.get_from_trade_index(get_idx as usize);

                Action::BankTrade(give, get)
            }
            _ => unreachable!(),
        }
    }
}

impl From<u128> for Actions {
    fn from(value: u128) -> Self {
        Self { bitflags: value }
    }
}

/// Wrapper around usize to index into the node arena
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NodeRef(pub usize);
impl NodeRef {
    /// 'Null pointer' substitute
    pub const INVALID: Self = Self(usize::MAX);
    /// Marker value to identify nodes where the parent action was [Action::EndTurn]
    pub const END_TURN: Self = Self(Self::INVALID.0 - 1);
    /// Marker value to identify nodes where the available moves are robber moves
    pub const ROBBER: Self = Self(Self::END_TURN.0 - 1);

    pub fn is_valid(&self) -> bool {
        *self != Self::INVALID && *self != Self::END_TURN && *self != Self::ROBBER
    }
}

/// A node in the game graph, representing a game state reached from a particular sequence of actions.
/// Currently transpositions aren't identified, so the same state may appear in multiple nodes in the graph
#[derive(Copy, Clone, Debug)]
pub struct Node {
    /// The current state of the game
    pub state: GameState,
    /// How many times has the MCTS algorithm visited this node?
    pub visits: u32,
    /// How many wins has each player had from all playouts beneath this node?
    pub wins: [u32; 4],
    /// Actions available to the player that haven't yet been tried
    pub available_actions: Actions,
    /// Pointer to the parent node in the graph
    pub parent: NodeRef,
    /// Action taken from the parent to reach this node
    pub parent_action: Action,
    /// Pointer to the head of the linked list of this node's children
    pub first_child: NodeRef,
    /// Pointer to the next node in the linked list of the parent node's children
    pub next_sibling: NodeRef,
}
impl Node {
    pub fn root(state: GameState, board: &Board) -> Self {
        Self::with_parent(state, board, NodeRef::INVALID, Action::NONE).with_sibling(NodeRef(0))
    }

    pub fn end_turn(state: GameState, parent: NodeRef) -> Self {
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: Actions::default(),
            parent,
            parent_action: Action::EndTurn,
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::END_TURN,
        }
    }

    pub fn robber(state: GameState, parent: NodeRef) -> Self {
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: state.generate_robber_moves(),
            parent,
            parent_action: Action::Roll(7),
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::ROBBER,
        }
    }

    pub fn steal(state: GameState, parent: NodeRef, spot: u8) -> Self {
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: state.generate_steals(spot),
            parent,
            parent_action: Action::MoveRobber(spot),
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::INVALID,
        }
    }

    pub fn initial_final(state: GameState, parent: NodeRef, parent_action: Action) -> Self {
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: Actions::default(),
            parent,
            parent_action,
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::INVALID,
        }
    }

    pub fn buy_dev_card(state: GameState, parent: NodeRef) -> Self {
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: Actions::default(),
            parent,
            parent_action: Action::BuyDevCard(Unknown),
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::INVALID,
        }
    }

    pub fn yop_resources(state: GameState, parent: NodeRef) -> Self {
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: state.generate_yop_resources(),
            parent,
            parent_action: Action::PlayDevCard(YoP),
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::INVALID,
        }
    }

    pub fn monopoly_resource(state: GameState, parent: NodeRef) -> Self {
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: state.generate_monopoly_resources(),
            parent,
            parent_action: Action::PlayDevCard(Monopoly),
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::INVALID,
        }
    }

    pub fn road_build_1(state: GameState, parent: NodeRef) -> Self {
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: state.generate_road_builds(),
            parent,
            parent_action: Action::PlayDevCard(RoadBuild),
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::INVALID,
        }
    }

    pub fn road_build_2(state: GameState, parent: NodeRef) -> Self {
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: state.generate_road_builds(),
            parent,
            parent_action: Action::RoadBuild1(0),
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::INVALID,
        }
    }

    pub fn with_parent(
        state: GameState,
        board: &Board,
        parent: NodeRef,
        parent_action: Action,
    ) -> Self {
        // don't generate actions for terminal nodes
        let actions = if state.is_terminal() {
            Actions::default()
        } else {
            state.generate_actions(board, parent_action)
        };
        Self {
            state,
            visits: 0,
            wins: [0; 4],
            available_actions: actions,
            parent,
            parent_action,
            first_child: NodeRef::INVALID,
            next_sibling: NodeRef::INVALID,
        }
    }

    pub fn with_sibling(mut self, sibling: NodeRef) -> Self {
        self.next_sibling = sibling;
        self
    }

    /// UCT: Upper Confidence bound applied to Trees.
    pub fn uct(&self, parent_player: usize, parent_visits: u32) -> f64 {
        if self.visits == 0 {
            return f64::INFINITY;
        }
        (self.wins[parent_player] as f64 / self.visits as f64)
            + consts::UCT_C * ((parent_visits as f64).ln() / self.visits as f64).sqrt()
    }

    pub fn is_end_turn(&self) -> bool {
        self.next_sibling == NodeRef::END_TURN
    }

    pub fn is_initial_final(&self) -> bool {
        matches!(self.parent_action, Action::InitialRoad(_)) && !self.state.is_initial()
    }

    pub fn is_chance_node(&self) -> bool {
        self.is_end_turn()
            || self.is_initial_final()
            || self.parent_action == Action::BuyDevCard(Unknown)
    }

    pub fn choose_child(&self, rng: &mut GameRng) -> NodeRef {
        let first = self.first_child.0;
        if self.is_end_turn() || self.is_initial_final() {
            NodeRef(first + rng.random_range(0..6) + rng.random_range(0..6))
        } else if self.parent_action == Action::BuyDevCard(Unknown) {
            // TODO: this allocates, make a custom one that doesn't?
            let dist = WeightedIndex::new(self.state.dev_card_deck).unwrap();
            NodeRef(first + dist.sample(rng))
        } else {
            self.first_child
        }
    }

    pub fn select_untried_action(&mut self, rng: &mut GameRng) -> Action {
        if self.state.is_initial() {
            self.available_actions
                .select_random_untried_initial_action(rng, self.state.is_initial_settle())
        } else {
            self.available_actions
                .select_random_untried_action(rng, self.parent_action)
        }
    }
}

/// Arena for contiguous graph node storage
pub struct NodeArena(Vec<Node>);

impl NodeArena {
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn insert(&mut self, node: Node) -> NodeRef {
        let i = self.0.len();
        self.0.push(node);
        NodeRef(i)
    }
}

impl Default for NodeArena {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<NodeRef> for NodeArena {
    type Output = Node;

    fn index(&self, index: NodeRef) -> &Self::Output {
        &self.0[index.0]
    }
}

impl IndexMut<NodeRef> for NodeArena {
    fn index_mut(&mut self, index: NodeRef) -> &mut Self::Output {
        &mut self.0[index.0]
    }
}
