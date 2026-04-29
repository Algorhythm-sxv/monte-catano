use std::fmt;

use anyhow::{Result, anyhow, bail, ensure};
use colored::Colorize;
use rand::{rngs::SmallRng, seq::SliceRandom};

use crate::game::*;

/// Represents everything about the board that is fixed during the game
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub num_players: u8,
    pub resources: [Resource; NUM_HEXES],
    pub numbers: [u8; NUM_HEXES],

    pub ports: [Resource; NUM_PORTS],
}

impl Board {
    pub fn new_random_fair(players: u8, rng: &mut SmallRng) -> Self {
        let mut resources = [
            Brick, Brick, Brick, Wood, Wood, Wood, Wood, Ore, Ore, Ore, Wheat, Wheat, Wheat, Wheat,
            Sheep, Sheep, Sheep, Sheep, Desert,
        ];
        resources.shuffle(rng);
        let desert = resources.iter().position(|r| *r == Desert).unwrap();
        let mut numbers = [2, 3, 3, 4, 4, 5, 5, 6, 6, 8, 8, 9, 9, 10, 10, 11, 11, 12, 0];
        'generate: loop {
            numbers.shuffle(rng);
            let zero = numbers.iter().position(|n| *n == 0).unwrap();
            // make sure the desert has the zero number
            numbers.swap(desert, zero);

            // check adjacency rules
            for i in 0..NUM_HEXES {
                let number = numbers[i];
                for adjacent in HEX_HEXES[i].iter().take_while(|i| **i != NONE) {
                    let neighbor = numbers[*adjacent];
                    // 6s and 8s can't touch
                    if matches!((number, neighbor), (6 | 8, 6 | 8)) {
                        continue 'generate;
                    }
                    // 2 and 12 can't touch
                    if matches!((number, neighbor), (2 | 12, 2 | 12)) {
                        continue 'generate;
                    }
                    // identical numbers can't touch
                    if number == neighbor {
                        continue 'generate;
                    }
                }
            }
            break;
        }

        let mut ports = [
            Brick, Wood, Ore, Wheat, Sheep, Desert, Desert, Desert, Desert,
        ];
        ports.shuffle(rng);
        Self {
            num_players: players,
            resources,
            numbers,
            ports,
        }
    }

    pub fn cli_string(&self) -> String {
        let mut s = self
            .resources
            .iter()
            .zip(self.numbers.iter())
            .map(|(res, n)| format!("{}{}", res.letter(), n))
            .collect::<Vec<_>>()
            .join(",");
        s.push('|');

        self.ports.iter().for_each(|r| s.push_str(r.letter()));

        s
    }

    pub fn from_cli_string<S: AsRef<str>>(players: u8, s: S) -> Result<Self> {
        let mut split = s.as_ref().split('|');
        let hexes: Vec<_> = split
            .next()
            .ok_or(anyhow!("Empty board string"))?
            .split(',')
            .collect();
        ensure!(
            hexes.len() == NUM_HEXES,
            "Expected {NUM_HEXES} resources, got {}",
            hexes.len()
        );

        let ports_string = split
            .next()
            .ok_or(anyhow!("No port list in board string"))?;
        ensure!(
            ports_string.len() == NUM_PORTS,
            "Expected {NUM_PORTS} ports, got {}",
            ports_string.len()
        );

        let mut resources = [Desert; NUM_HEXES];
        let mut numbers = [0; NUM_HEXES];
        for (i, hex) in hexes.iter().enumerate() {
            let letter = hex.get(0..1).ok_or(anyhow!("Empty hex in baord string"))?;
            let res = match letter.to_ascii_lowercase().as_str() {
                "b" => Brick,
                "w" => Wood,
                "o" => Ore,
                "g" => Wheat,
                "s" => Sheep,
                "d" => Desert,
                other => bail!("invalid resource letter in board string: {other}"),
            };
            let number = hex
                .get(1..)
                .ok_or(anyhow!("Empty number in board string"))?
                .parse::<u8>()?;
            resources[i] = res;
            numbers[i] = number;
        }

        let mut ports = [Desert; NUM_PORTS];
        for (i, letter) in ports_string.chars().enumerate() {
            ports[i] = match letter {
                'b' => Brick,
                'w' => Wood,
                'o' => Ore,
                'g' => Wheat,
                's' => Sheep,
                'd' => Desert,
                other => bail!("Invalid port letter in board string: {other}"),
            };
        }

        Ok(Self {
            num_players: players,
            resources,
            numbers,
            ports,
        })
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Rows of the board: None = water, Some(i) = land hex index
        const ROWS: &[&[Option<usize>]] = &[
            &[None, None, None, None, None],
            &[None, None, None, None, None, None],
            &[None, None, Some(0), Some(1), Some(2), None, None],
            &[None, None, Some(3), Some(4), Some(5), Some(6), None, None],
            &[
                None,
                None,
                Some(7),
                Some(8),
                Some(9),
                Some(10),
                Some(11),
                None,
                None,
            ],
            &[
                None,
                None,
                Some(12),
                Some(13),
                Some(14),
                Some(15),
                None,
                None,
            ],
            &[None, None, Some(16), Some(17), Some(18), None, None],
            &[None, None, None, None, None, None],
            &[None, None, None, None, None],
        ];
        const MAX_WIDTH: usize = 9;

        // (row, col) → port index
        fn port_at(row: usize, col: usize) -> Option<usize> {
            match (row, col) {
                (1, 1) => Some(0), // up-left of hex 0
                (1, 3) => Some(1), // up-right of hex 1
                (3, 1) => Some(2), // left of hex 3
                (2, 5) => Some(3), // up-right of hex 6
                (4, 7) => Some(4), // right of hex 11
                (5, 1) => Some(5), // left of hex 12
                (6, 5) => Some(6), // down-right of hex 15
                (7, 1) => Some(7), // down-left of hex 16
                (7, 3) => Some(8), // down-right of hex 17
                _ => Option::None,
            }
        }

        for (row_idx, row) in ROWS.iter().enumerate() {
            let indent = (MAX_WIDTH - row.len()) * 2;
            write!(f, "{:indent$}", "")?;

            for (col_idx, tile) in row.iter().enumerate() {
                if col_idx > 0 {
                    write!(f, "  ")?;
                }
                match tile {
                    None => {
                        if let Some(port_idx) = port_at(row_idx, col_idx) {
                            let res = self.ports[port_idx];
                            write!(f, "{}", res.color(res.port_letter()).bold())?;
                        } else {
                            write!(f, "{}", "~~".blue().bold())?;
                        }
                    }
                    Some(idx) => {
                        let num = self.numbers[*idx];
                        let resource = self.resources[*idx];
                        let text = format!("{:>2}", num);
                        write!(f, "{}", resource.color(&text).bold())?;
                    }
                }
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use rand::SeedableRng;

    use super::*;
    #[test]
    fn cli_string_round_trip() {
        for i in 0..100 {
            let mut rng = SmallRng::seed_from_u64(0xDEADBEEF + i);
            let board = Board::new_random_fair(4, &mut rng);

            let string = board.cli_string();

            let parsed = Board::from_cli_string(4, &string).expect("round trip failed to parse");

            assert!(board == parsed, "round trip not equal: {string}");
        }
    }
}
