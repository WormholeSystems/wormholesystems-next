// The SDE structs model the full external schema; many fields aren't read yet
// (they're only deserialized), and the download/archive bootstrap is WIP.
#![allow(dead_code)]

use crate::sde::SolarSystem;

mod config;
mod esi;
mod sde;
mod util;

fn main() {
    let systems = sde::load::<SolarSystem>().expect("failed to load solar systems");

    println!("Loaded {} solar systems", systems.len());
    println!("{:#?}", systems.get(&30000777).unwrap());
}
