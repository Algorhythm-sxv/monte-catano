use serde::{Deserialize, Serialize};

use crate::game::{EDGE_EDGES, NONE, types::*};

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
    pub(crate) longest_road: u8,
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

    // calculate the longest contiguous road including the last one placed, since if this placement increases the longest road it must be in the chain
    pub fn calculate_longest_road(&mut self, placed: usize) {
        let mut roads_left = self.roads();
        roads_left ^= 1 << placed;
        let sub = self._longest_road(placed, None, &mut roads_left);
        self.longest_road = self.longest_road.max(sub);
    }

    fn _longest_road(
        &self,
        current_road: usize,
        prev_road: Option<usize>,
        roads_left: &mut u128,
    ) -> u8 {
        let mut longest = 1;

        let mut sub_lengths = [(NONE, 0); 4];
        let mut i = 0;

        // look over connected edges with roads not yet seen
        for next in EDGE_EDGES[current_road].iter().take_while(|e| **e != NONE) {
            // a previous recursion might have already explored this road (loop)
            if *roads_left & (1 << next) == 0 {
                continue;
            }
            // or we may have hit a branch and don't want to 'double back'
            if let Some(prev) = prev_road
                && EDGE_EDGES[prev].contains(next)
            {
                continue;
            }
            *roads_left ^= 1 << next;
            sub_lengths[i] = (
                *next,
                self._longest_road(*next, Some(current_road), roads_left),
            );
            i += 1;
        }

        // find the longest sublength, which may be connected across the caller road
        for (e1, l1) in sub_lengths.iter().take_while(|(e, _)| *e != NONE) {
            // worst case the longest is the sublength plus the caller road
            longest = longest.max(l1 + 1);

            // check all edges against all others
            for (e2, l2) in sub_lengths.iter().filter(|(e, _)| *e != NONE && e != e1) {
                if !EDGE_EDGES[*e1].contains(e2) {
                    // sublengths are connect across the caller road
                    longest = longest.max(l1 + l2 +  1);
                }
                // if EDGE_EDGES[*e1].contains(e2) {
                //     // edges are connected without the caller road
                //     longest = longest.max(l1 + l2)
                // } else {
                //     // edges are connected across the caller road
                //     longest = longest.max(l1 + l2 + 1);
                // }
            }
        }

        longest
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
            longest_road: 1,
            vps: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::PlayerState;

    #[test]
    fn test_longest_road_horizontal() {
        let mut p = PlayerState::default();
        p.roads_lower = 0x3F; // first 5 edges in a line
        p.calculate_longest_road(0);
        assert!(p.longest_road == 6, "horizontal longest road failed")
    }

    #[test]
    fn test_longest_road_vertical() {
        let mut p = PlayerState::default();
        p.roads_lower = (1 << 1) | (1 << 7) | (1 << 13) | (1 << 20) | (1 << 27) | (1 << 35);
        p.calculate_longest_road(35);
        assert!(p.longest_road == 6, "vertical longest road failed")
    }

    #[test]
    fn test_longest_road_middle() {
        let mut p = PlayerState::default();
        p.roads_lower = (1 << 6) | (1 << 10) | (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 8);
        p.calculate_longest_road(0);
        assert!(p.longest_road == 7, "middle longest road failed")
    }

    #[test]
    fn test_longest_road_loop() {
        let mut p = PlayerState::default();
        p.roads_lower = (1 << 0) | (1 << 1) | (1 << 7) | (1 << 12) | (1 << 11) | (1 << 6);
        p.calculate_longest_road(0);
        assert!(p.longest_road == 6, "loop longest road failed")
    }

    #[test]
    fn test_longest_road_branch() {
        let mut p = PlayerState::default();
        p.roads_lower = (1 << 7) | (1 << 2) | (1 << 11) | (1 << 12) | (1 << 13) | (1 << 14) | (1 << 15);
        p.longest_road = 5;
        p.calculate_longest_road(7);
        assert!(p.longest_road == 5, "branch longest road failed")
    }

    #[test]
    fn test_longest_road_disjoint() {
        let mut p = PlayerState::default();
        p.roads_lower = (1 << 0) | (1 << 1) | (1 << 35) | (1 << 43) | (1 << 44);
        p.longest_road = 2;
        p.calculate_longest_road(35);
        assert!(p.longest_road == 3, "disjoint longest road failed")
    }
}
