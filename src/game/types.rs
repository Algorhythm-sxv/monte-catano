use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Player {
    P0 = 0,
    P1 = 1,
    P2 = 2,
    P3 = 3,
    #[default]
    PNone = 4,
}

pub use Player::*;

impl From<u8> for Player {
    fn from(value: u8) -> Self {
        const PLAYERS: [Player; 5] = [P0, P1, P2, P3, PNone];
        PLAYERS[value as usize]
    }
}

impl<T, const N: usize> Index<Player> for [T; N] {
    type Output = T;

    fn index(&self, index: Player) -> &Self::Output {
        &self[index as usize]
    }
}

impl<T, const N: usize> IndexMut<Player> for [T; N] {
    fn index_mut(&mut self, index: Player) -> &mut Self::Output {
        &mut self[index as usize]
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resource {
    Brick = 0,
    Wood = 1,
    Ore = 2,
    Wheat = 3,
    Sheep = 4,
    #[default]
    Desert = 5,
}
pub use Resource::*;

impl Resource {
    pub fn color(self, text: &str) -> colored::ColoredString {
        match self {
            Desert => text.truecolor(210, 180, 140),
            Brick => text.truecolor(178, 34, 34),
            Wood => text.truecolor(0, 100, 0),
            Ore => text.truecolor(169, 169, 169),
            Wheat => text.truecolor(255, 215, 0),
            Sheep => text.truecolor(0, 255, 0),
        }
    }

    pub fn port_letter(self) -> &'static str {
        match self {
            Brick => " B",
            Wood => " W",
            Ore => " O",
            Wheat => " G",
            Sheep => " S",
            Desert => " ?",
        }
    }

    pub fn letter(self) -> &'static str {
        match self {
            Self::Brick => "b",
            Self::Wood => "w",
            Self::Ore => "o",
            Self::Wheat => "g",
            Self::Sheep => "s",
            Self::Desert => "d",
        }
    }

    pub fn trade_index(&self, other: Self) -> u8 {
        match self {
            Self::Brick => match other {
                Self::Wood => 0,
                Self::Ore => 1,
                Self::Wheat => 2,
                Self::Sheep => 3,
                Self::Desert | Self::Brick => 4,
            },
            Self::Wood => match other {
                Self::Brick => 0,
                Self::Ore => 1,
                Self::Wheat => 2,
                Self::Sheep => 3,
                Self::Desert | Self::Wood => 4,
            },
            Self::Ore => match other {
                Self::Brick => 0,
                Self::Wood => 1,
                Self::Wheat => 2,
                Self::Sheep => 3,
                Self::Desert | Self::Ore => 4,
            },
            Self::Wheat => match other {
                Self::Brick => 0,
                Self::Wood => 1,
                Self::Ore => 2,
                Self::Sheep => 3,
                Self::Desert | Self::Wheat => 4,
            },
            Self::Sheep => match other {
                Self::Brick => 0,
                Self::Wood => 1,
                Self::Ore => 2,
                Self::Wheat => 3,
                Self::Desert | Self::Sheep => 4,
            },
            Self::Desert => 4,
        }
    }
    pub fn get_from_trade_index(&self, i: usize) -> Self {
        match self {
            Self::Brick => [Wood, Ore, Wheat, Sheep][i],
            Self::Wood => [Brick, Ore, Wheat, Sheep][i],
            Self::Ore => [Brick, Wood, Wheat, Sheep][i],
            Self::Wheat => [Brick, Wood, Ore, Sheep][i],
            Self::Sheep => [Brick, Wood, Ore, Wheat][i],
            Self::Desert => Desert,
        }
    }
}

impl From<u8> for Resource {
    fn from(value: u8) -> Self {
        const RESOURCES: [Resource; 6] = [Brick, Wood, Ore, Wheat, Sheep, Desert];
        RESOURCES[value as usize]
    }
}

impl<T, const N: usize> Index<Resource> for [T; N] {
    type Output = T;

    fn index(&self, index: Resource) -> &Self::Output {
        &self[index as usize]
    }
}

impl<T, const N: usize> IndexMut<Resource> for [T; N] {
    fn index_mut(&mut self, index: Resource) -> &mut Self::Output {
        &mut self[index as usize]
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DevCard {
    Knight = 0,
    Monopoly = 1,
    YoP = 2,
    RoadBuild = 3,
    VP = 4,
    #[default]
    Unknown = 5,
}
pub use DevCard::*;

impl From<u8> for DevCard {
    fn from(value: u8) -> Self {
        const DEV_CARDS: [DevCard; 6] = [Knight, Monopoly, YoP, RoadBuild, VP, Unknown];
        DEV_CARDS[value as usize]
    }
}
