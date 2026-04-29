use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::{
    game::*,
    graph::{Action, Actions},
};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct GameState {
    pub(crate) players: [PlayerState; 4],
    pub(crate) current_player: u8,
    dev_card_deck: [u8; 5],
    longest_road: (Player, u8),
    largest_army: (Player, u8),
    robber: u8,
}

impl GameState {
    pub fn new(board: &Board) -> Self {
        let robber = board.resources.iter().position(|r| *r == Desert).unwrap() as u8;
        Self {
            robber,
            ..Default::default()
        }
    }
    pub fn current_player(&self) -> &PlayerState {
        &self.players[self.current_player as usize]
    }

    pub fn current_player_mut(&mut self) -> &mut PlayerState {
        &mut self.players[self.current_player as usize]
    }

    pub fn is_vertex_empty(&self, vertex: usize) -> bool {
        vertex == NONE
            || self
                .players
                .iter()
                .all(|p| (p.villages | p.cities) & (1 << vertex) == 0)
    }

    pub fn is_vertex_buildable(&self, vertex: usize) -> bool {
        self.is_vertex_empty(vertex)
            && VERTEX_VERTICES[vertex]
                .iter()
                .all(|v| self.is_vertex_empty(*v))
    }

    pub fn is_edge_buildable(&self, edge: usize) -> bool {
        edge != NONE && self.players.iter().all(|p| p.roads() & (1 << edge) == 0)
    }

    pub fn is_initial(&self) -> bool {
        // if the first player has played less than 2 roads we are still in the inital phase
        self.players[0].roads_left > 13
    }

    pub fn is_initial_settle(&self) -> bool {
        5 - self.current_player().villages_left == 15 - self.current_player().roads_left
    }

    pub fn is_terminal(&self) -> bool {
        self.players.iter().any(|p| p.vps >= 10)
    }

    pub fn build_village(&mut self, vertex: usize, player: Player) {
        let p = &mut self.players[player];
        p.resources[Brick] -= 1;
        p.resources[Wood] -= 1;
        p.resources[Wheat] -= 1;
        p.resources[Sheep] -= 1;
        p.villages |= 1 << vertex;
        p.villages_left -= 1;
        p.vps += 1;
    }

    pub fn build_city(&mut self, vertex: usize, player: Player) {
        let p = &mut self.players[player];
        p.resources[Ore] -= 3;
        p.resources[Wheat] -= 2;
        p.cities |= 1 << vertex;
        p.villages ^= 1 << vertex;
        p.cities_left -= 1;
        p.villages_left += 1;
        p.vps += 1
    }

    pub fn build_road(&mut self, edge: usize, player: Player) {
        let p = &mut self.players[player];
        p.resources[Brick] -= 1;
        p.resources[Wood] -= 1;
        if edge < 64 {
            p.roads_lower |= 1 << edge;
        } else {
            p.roads_upper |= 1 << (edge - 64);
        }
        p.roads_left -= 1;

        if 15 - self.players[player].roads_left > self.longest_road.1 {
            if self.longest_road.0 != Player::PNone {
                self.players[self.longest_road.0].vps -= 2;
            }
            self.players[player].vps += 2;
            self.longest_road = (player, 15 - self.players[player].roads_left)
        }
    }

    pub fn generate_initial_settles(&self) -> Actions {
        let mut actions = Actions::default();
        let mut settles = 0;
        for v in 0..NUM_VERTICES {
            if self.is_vertex_buildable(v) {
                settles |= 1 << v;
            }
        }
        actions.add_initial_settles(settles);
        actions
    }

    pub fn generate_initial_roads(&self) -> Actions {
        let mut actions = Actions::default();
        let mut roads = 0;
        // initial roads must be placed next to villages with no joining roads
        let mut vs = self.current_player().villages;
        let mut last_village = NONE;
        while vs != 0 {
            let v = vs.trailing_zeros();
            if VERTEX_EDGES[v as usize]
                .iter()
                .take_while(|e| **e != NONE)
                .any(|e| self.current_player().roads() & (1 << e) != 0)
            {
                // this village has an adjacent road, skip it
            } else {
                last_village = v as usize;
                break;
            }
            vs &= vs - 1;
        }
        debug_assert!(last_village != NONE);
        for r in VERTEX_EDGES[last_village] {
            if r == NONE {
                break;
            }
            roads = (roads << 1) | 1
        }
        actions.add_initial_roads(roads);
        actions
    }

    pub fn generate_actions(&self, board: &Board, parent_action: Action) -> Actions {
        match parent_action.forced_continuation() {
            Some(Action::MoveRobber(_)) => self.generate_robber_moves(),
            Some(Action::Steal(_, _)) => {
                let Action::MoveRobber(spot) = parent_action else {
                    panic!("steal following invalid action: {parent_action:?}")
                };
                self.generate_steals(spot)
            }
            Some(Action::YoPResources(_, _)) => self.generate_yop_resources(),
            Some(Action::MonopolyResource(_)) => self.generate_monopoly_resources(),
            Some(Action::RoadBuild1(_) | Action::RoadBuild2(_)) => self.generate_road_builds(),
            _ => self.generate_actions_normal(board),
        }
    }

    pub fn generate_robber_moves(&self) -> Actions {
        Actions::generate_robber_without(self.robber)
    }

    pub fn generate_steals(&self, spot: u8) -> Actions {
        let mut player_mask = 0;

        for v in HEX_VERTICES[spot as usize] {
            if let Some(player) = self.players.iter().position(|p| p.has_built_on_vertex(v))
                && player != self.current_player as usize
            {
                player_mask |= 1 << player
            }
        }

        // if no steals from others are available, just 'steal' from yourself
        if player_mask == 0 {
            player_mask |= 1 << self.current_player;
        }
        Actions::from(player_mask)
    }

    pub fn generate_yop_resources(&self) -> Actions {
        // 25 bits for 5x5 resource combinations
        let mask = 0x1FFFFFFu128;
        Actions::from(mask)
    }

    pub fn generate_monopoly_resources(&self) -> Actions {
        // 5 bits for 5 resources
        let mask = 0x1Fu128;
        Actions::from(mask)
    }

    pub fn generate_road_builds(&self) -> Actions {
        let mut mask = 0;
        let road_spots = (0..NUM_EDGES).filter(|e| {
            self.is_edge_buildable(*e)
                && EDGE_EDGES[*e]
                    .iter()
                    .take_while(|e| **e != NONE)
                    .any(|e| self.current_player().roads() & (1 << e) != 0)
        });
        road_spots.enumerate().for_each(|(i, _)| mask |= 1 << i);
        Actions::from(mask)
    }

    fn generate_actions_normal(&self, board: &Board) -> Actions {
        if self.is_initial() {
            return if self.is_initial_settle() {
                self.generate_initial_settles()
            } else {
                self.generate_initial_roads()
            };
        }
        let mut actions = Actions::default();

        let player = Player::from(self.current_player);
        let player_state = self.current_player();
        // settles
        if player_state.can_settle() {
            let settle_spots = (0..NUM_VERTICES).filter(|v| {
                self.is_vertex_buildable(*v)
                    && VERTEX_EDGES[*v]
                        .iter()
                        .take_while(|e| **e != NONE)
                        .any(|e| self.players[player].roads() & (1 << e) != 0)
            });
            settle_spots
                .enumerate()
                .for_each(|(i, _)| actions.add_settle(i));
        }
        // cities
        if player_state.can_city() {
            let city_spots = self.players[player].villages;
            for i in 0..city_spots.count_ones() {
                actions.add_city(i as usize);
            }
        }
        // roads
        if player_state.can_road() {
            let road_spots = (0..NUM_EDGES).filter(|e| {
                self.is_edge_buildable(*e)
                    && EDGE_EDGES[*e]
                        .iter()
                        .take_while(|e| **e != NONE)
                        .any(|e| self.players[player].roads() & (1 << e) != 0)
            });
            road_spots
                .enumerate()
                .for_each(|(i, _)| actions.add_road(i));
        }

        // buy dev card
        if player_state.can_buy_dev_card() && self.dev_card_deck.iter().any(|c| *c > 0) {
            actions.add_buy_dev_card()
        }
        // play_dev_card
        if player_state.can_play_dev_card() {
            for (i, (n, bought)) in player_state
                .dev_cards
                .iter()
                .zip(player_state.bought_dev_cards.iter())
                .take(4)
                .enumerate()
            {
                if *n > *bought {
                    actions.add_play_dev_card(i);
                }
            }
        }

        let mut has_3_1 = false;
        for (res, verts) in board
            .ports
            .iter()
            .zip(PORT_VERTICES)
            .filter(|(r, _)| **r == Desert)
        {
            let port_mask = (1 << verts[0]) | (1 << verts[1]);
            let has_port = (player_state.villages | player_state.cities) & port_mask != 0;
            if has_port && *res == Desert {
                has_3_1 = true;
                break;
            }
        }
        for (res, verts) in board
            .ports
            .iter()
            .zip(PORT_VERTICES)
            .filter(|(r, _)| **r != Desert)
        {
            let port_mask = (1 << verts[0]) | (1 << verts[1]);
            let has_port = (player_state.villages | player_state.cities) & port_mask != 0;
            let min = if has_port {
                2
            } else if has_3_1 {
                3
            } else {
                4
            };
            if player_state.resources[*res] >= min {
                actions.add_bank_trades(*res);
            }
        }
        actions.add_end_turn();

        actions
    }

    pub fn apply_action(&mut self, board: &Board, action: Action, rng: &mut GameRng) -> Action {
        let player = Player::from(self.current_player);
        match action {
            Action::InitialSettle(v) => {
                self.build_village(v as usize, player);
                // second initial settle gets resources
                if self.current_player().villages_left == 3 {
                    for h in VERTEX_HEXES[v as usize].iter().take_while(|h| **h != NONE) {
                        if board.resources[*h] != Desert {
                            self.current_player_mut().resources[board.resources[*h]] += 1;
                        }
                    }
                }
                action
            }
            Action::InitialRoad(spot) => {
                // find the village with an unbuilt road
                let mut vs = self.current_player().villages;
                let mut last_village = NONE;
                while vs != 0 {
                    let v = vs.trailing_zeros();
                    if VERTEX_EDGES[v as usize]
                        .iter()
                        .take_while(|e| **e != NONE)
                        .any(|e| self.current_player().roads() & (1 << e) != 0)
                    {
                        // this village has an adjacent road, skip it
                    } else {
                        last_village = v as usize;
                        break;
                    }
                    vs &= vs - 1;
                }
                debug_assert!(last_village != NONE);
                let road = VERTEX_EDGES[last_village][spot as usize];
                self.build_road(road, player);

                let forward = self.players[board.num_players as usize - 1]
                    .villages
                    .count_ones()
                    < 2;
                // initial roads move to the next player
                self.current_player = if forward {
                    (self.current_player + 1).min(board.num_players - 1)
                } else {
                    self.current_player.saturating_sub(1)
                };
                action
            }
            Action::MoveRobber(spot) => {
                self.robber = spot;
                action
            }
            Action::Steal(player, mut chosen_res) => {
                if chosen_res == Desert {
                    // steal a random card if not specified
                    let res_total = self.players[player as usize].resources.iter().sum();
                    if res_total == 0 {
                        return Action::Steal(player, Desert);
                    }
                    let mut i = rng.random_range(0..res_total);
                    for res in [Brick, Wood, Ore, Wheat, Sheep] {
                        let count = self.players[player as usize].resources[res as usize];
                        if count > i {
                            chosen_res = res;
                            break;
                        }
                        i -= count
                    }
                }

                if chosen_res != Desert {
                    self.players[player as usize].resources[chosen_res] -= 1;
                    self.current_player_mut().resources[chosen_res] += 1;
                }
                Action::Steal(player, chosen_res)
            }
            Action::Settle(spot) => {
                // find vertex of given spot
                let mut settle_spots = (0..NUM_VERTICES).filter(|v| {
                    self.is_vertex_buildable(*v)
                        && VERTEX_EDGES[*v]
                            .iter()
                            .take_while(|e| **e != NONE)
                            .any(|e| self.current_player().roads() & (1 << e) != 0)
                });
                let vertex = settle_spots
                    .nth(spot as usize)
                    .expect("not enough settle spots found");
                self.build_village(vertex, player);
                action
            }
            Action::City(spot) => {
                // find vertex of given spot
                let mut city_spots = self.current_player().villages;
                for _ in 0..spot {
                    city_spots &= city_spots - 1;
                }
                let vertex = city_spots.trailing_zeros() as usize;
                self.build_city(vertex, player);
                action
            }
            Action::Road(spot) | Action::RoadBuild1(spot) | Action::RoadBuild2(spot) => {
                let mut road_spots = (0..NUM_EDGES).filter(|e| {
                    self.is_edge_buildable(*e)
                        && EDGE_EDGES[*e]
                            .iter()
                            .take_while(|e| **e != NONE)
                            .any(|e| self.current_player().roads() & (1 << e) != 0)
                });
                // find edge of given spot
                let edge = road_spots.nth(spot as usize);
                match action {
                    Action::RoadBuild1(_) | Action::RoadBuild2(_) => {
                        // give fake resources for road builder
                        if self.current_player().roads_left > 0 {
                            self.current_player_mut().resources[Brick] += 1;
                            self.current_player_mut().resources[Wood] += 1;
                        } else {
                            // can't place another road
                            return action;
                        }
                    }
                    Action::Road(_) => {
                        if edge.is_none() {
                            panic!("Not enough road spots found")
                        }
                    }
                    _ => unreachable!(),
                }
                if let Some(edge) = edge {
                    self.build_road(edge, player);
                }
                action
            }
            Action::BuyDevCard(card) => {
                self.players[player].resources[Ore] -= 1;
                self.players[player].resources[Wheat] -= 1;
                self.players[player].resources[Sheep] -= 1;
                let card = match card {
                    Unknown => {
                        let cards: u8 = self.dev_card_deck.iter().sum();
                        let mut card_index = rng.random_range(0..cards);
                        let mut card = Unknown;
                        for (i, c) in self.dev_card_deck.iter().enumerate() {
                            if *c > card_index {
                                self.dev_card_deck[i] -= 1;
                                card = DevCard::from(i as u8);
                                break;
                            }
                            card_index -= c;
                        }
                        card
                    }
                    _ => card,
                };
                self.players[player].dev_cards[card as usize] += 1;
                self.players[player].bought_dev_cards[card as usize] += 1;
                if card == DevCard::VP {
                    self.players[player].vps += 1;
                }
                Action::BuyDevCard(card)
            }
            Action::BankTrade(give, get) => {
                let has_3_1 = board
                    .ports
                    .iter()
                    .zip(PORT_VERTICES)
                    .filter(|(r, _)| **r == Desert)
                    .any(|(_, v)| {
                        let settles = self.players[player].villages | self.players[player].cities;
                        let port_mask = (1 << v[0]) | (1 << v[1]);
                        settles & port_mask != 0
                    });
                let port_idx = board.ports.iter().position(|p| *p == give).unwrap();
                let has_port = PORT_VERTICES[port_idx]
                    .iter()
                    .any(|v| self.players[player].has_built_on_vertex(*v));
                let give_n = if has_port {
                    2
                } else if has_3_1 {
                    3
                } else {
                    4
                };
                self.players[player].resources[give] -= give_n;
                self.players[player].resources[get] =
                    self.players[player].resources[get].saturating_add(1);
                action
            }
            Action::PlayDevCard(dev_card) => {
                self.players[player].dev_cards[dev_card as usize] -= 1;
                match dev_card {
                    Knight => {
                        self.players[player].knights_played += 1;
                        let largest_army = self.largest_army.1;
                        if self.players[player].knights_played > largest_army {
                            if self.largest_army.0 != PNone {
                                self.players[self.largest_army.0].vps -= 2;
                            }
                            self.largest_army = (player, self.players[player].knights_played);
                            self.players[player].vps += 2;
                        }
                    }
                    VP | Unknown => unreachable!(),
                    _ => {}
                }
                action
            }
            Action::EndTurn => {
                self.players[player].bought_dev_cards = [0; 5];
                self.current_player = (self.current_player + 1) % board.num_players;
                action
            }
            Action::Roll(n) => {
                if n != 7 {
                    self.roll_resources(board, n as usize);
                } else {
                    self.discard_excess();
                }
                action
            }
            Action::YoPResources(res1, res2) => {
                let p = self.current_player_mut();
                p.resources[res1] += 1;
                p.resources[res2] += 1;
                action
            }
            Action::MonopolyResource(res) => {
                let total = self.players.iter().map(|p| p.resources[res]).sum();
                for (i, p) in self.players.iter_mut().enumerate() {
                    if i == self.current_player as usize {
                        p.resources[res] = total;
                    } else {
                        p.resources[res] = 0;
                    }
                }
                action
            }
        }
    }

    pub fn roll_resources(&mut self, board: &Board, roll: usize) {
        let rolled_hexes = board
            .numbers
            .iter()
            .enumerate()
            .filter(|(_, n)| **n as usize == roll);
        for (hex, _) in rolled_hexes.filter(|(h, _)| *h != self.robber as usize) {
            let resource = board.resources[hex];
            for v in HEX_VERTICES[hex].iter().take_while(|v| **v != NONE) {
                self.players.iter_mut().for_each(|p| {
                    if p.villages & (1 << v) != 0 {
                        p.resources[resource as usize] =
                            p.resources[resource as usize].saturating_add(1)
                    } else if p.cities & (1 << v) != 0 {
                        p.resources[resource as usize] =
                            p.resources[resource as usize].saturating_add(2);
                    }
                });
            }
        }
    }

    pub fn discard_excess(&mut self) {
        for p in self.players.iter_mut() {
            let total: u8 = p.resources.iter().sum();
            if total > 7 {
                for _ in 0..(total / 2) {
                    let most = p
                        .resources
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, r)| **r)
                        .unwrap()
                        .0;
                    p.resources[most] -= 1;
                }
            }
        }
    }

    pub fn player(&self) -> u8 {
        self.current_player
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            players: [Default::default(); 4],
            current_player: Default::default(),
            dev_card_deck: [14, 2, 2, 2, 5],
            longest_road: Default::default(),
            largest_army: Default::default(),
            robber: u8::MAX,
        }
    }
}
