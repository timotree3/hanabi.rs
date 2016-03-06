extern crate rand;

mod game;
mod strategies;

fn main() {
    let opts = game::GameOptions {
        num_players: 4,
        hand_size: 4,
        num_hints: 8,
        num_lives: 3,
    };
    strategies::simulate(opts, strategies::AlwaysPlay);
}
