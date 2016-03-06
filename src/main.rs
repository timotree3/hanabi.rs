extern crate rand;

mod game;

fn main() {
    game::GameState::new(game::GameOptions {
        num_players: 4,
        hand_size: 4,
        total_hints: 8,
        total_lives: 3,
    });
}
