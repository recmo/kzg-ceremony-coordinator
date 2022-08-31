#![doc = include_str!("../Readme.md")]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::nursery)]

use cli_batteries::{run, version};
use kzg_ceremony_coordinator::main as app;

fn main() {
    run(version!(semaphore, ethers), app);
}
