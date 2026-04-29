use serde::{Deserialize, Serialize};

use crate::game::types::*;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct PlayerState {
    pub(crate) villages: u64,
    pub(crate) cities: u64,
    pub(crate) roads_lower: u64,
    pub(crate) roads_upper: u8,
    pub(crate) resources: [u8; 5],
    pub(crate) dev_cards: [u8; 5],
    pub(crate) bought_dev_cards: [u8; 5],
    pub(crate) knights_played: u8,
    pub(crate) vps: u8,
    pub(crate) roads_left: u8,
    pub(crate) villages_left: u8,
    pub(crate) cities_left: u8,
}

impl PlayerState {
    pub fn roads(&self) -> u128 {
        ((self.roads_upper as u128) << 64) | self.roads_lower as u128
    }
    pub fn has_built_on_vertex(&self, vertex: usize) -> bool {
        (self.villages | self.cities) & (1 << vertex) != 0
    }
    pub fn can_settle(&self) -> bool {
        self.villages_left > 0
            && self.resources[Brick as usize] > 0
            && self.resources[Wood as usize] > 0
            && self.resources[Wheat as usize] > 0
            && self.resources[Sheep as usize] > 0
    }
    pub fn can_city(&self) -> bool {
        self.cities_left > 0
            && self.resources[Ore as usize] >= 3
            && self.resources[Wheat as usize] >= 2
    }
    pub fn can_road(&self) -> bool {
        self.roads_left > 0
            && self.resources[Brick as usize] > 0
            && self.resources[Wood as usize] > 0
    }
    pub fn can_buy_dev_card(&self) -> bool {
        self.resources[Ore as usize] > 0
            && self.resources[Wheat as usize] > 0
            && self.resources[Sheep as usize] > 0
    }
    pub fn can_play_dev_card(&self) -> bool {
        // VPs aren't playable -> take(4)
        for (i, c) in self.bought_dev_cards.iter().take(4).enumerate() {
            if self.dev_cards[i] > *c {
                // player already had a copy of this card
                return true;
            }
        }
        false
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            villages: Default::default(),
            cities: Default::default(),
            roads_lower: Default::default(),
            roads_upper: Default::default(),
            roads_left: 15,
            villages_left: 5,
            cities_left: 4,
            resources: [4, 4, 0, 2, 2], // fake resources for the starting settlements/roads
            dev_cards: Default::default(),
            bought_dev_cards: [0; 5],
            knights_played: Default::default(),
            vps: Default::default(),
        }
    }
}
