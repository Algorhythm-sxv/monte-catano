use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the UCI console interface (default)
    Uci,
    /// Watch the engine play a single game against itself
    Play {
        /// Optional RNG seed
        seed: Option<u64>,
    },
    /// Run an SPRT gainer test against another engine
    Sprt {
        /// Path to the other engine executable
        exe: PathBuf,
        /// Number of concurrent threads to run
        #[arg(short, long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..))]
        threads: u16,
        /// Number of playouts to run for each move during a game
        #[arg(short, long, default_value_t = 100, value_parser = clap::value_parser!(u64).range(1..))]
        playouts: u64,
    },
}
