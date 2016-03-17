use rand::{self, Rng};
use game::*;
use std::collections::HashMap;

// Traits to implement for any valid Hanabi strategy

// Represents the strategy of a given player
pub trait PlayerStrategy {
    fn decide(&mut self, &GameStateView) -> TurnChoice;
    fn update(&mut self, &Turn, &GameStateView);
}
// Represents the overall strategy for a game
// Shouldn't do much, except possibility e.g. initialize some shared randomness between players
pub trait GameStrategy {
    fn initialize(&self, Player, &GameStateView) -> Box<PlayerStrategy>;
}
// Represents configuration for a strategy.
// Acts as a factory for game strategies, so we can play many rounds
pub trait GameStrategyConfig {
    fn initialize(&self, &GameOptions) -> Box<GameStrategy>;
}

pub fn simulate_once(
        opts: &GameOptions,
        game_strategy: Box<GameStrategy>,
        seed_opt: Option<u32>,
    ) -> Score {

    let seed = seed_opt.unwrap_or(rand::thread_rng().next_u32());

    let mut game = GameState::new(opts, seed);

    let mut strategies : HashMap<Player, Box<PlayerStrategy>> = HashMap::new();
    for player in game.get_players() {
        strategies.insert(
            player,
            game_strategy.initialize(player.clone(), &game.get_view(player)),
        );
    }

    debug!("Initial state:\n{}", game);

    while !game.is_over() {
        debug!("Turn {}", game.board.turn);
        let player = game.board.player;
        let choice = {
            let mut strategy = strategies.get_mut(&player).unwrap();
            strategy.decide(&game.get_view(player))
        };

        game.process_choice(&choice);

        let turn = Turn {
            player: &player,
            choice: &choice,
        };

        for player in game.get_players() {
            let mut strategy = strategies.get_mut(&player).unwrap();
            strategy.update(&turn, &game.get_view(player));
        }

        // TODO: do some stuff
        debug!("State:\n{}", game);
    }
    let score = game.score();
    debug!("SCORED: {:?}", score);
    score
}

// TODO: multithreaded
pub fn simulate(
        opts: &GameOptions,
        strat_config: &GameStrategyConfig,
        first_seed_opt: Option<u32>,
        n_trials: u32,
    ) -> f32 {

    let mut total_score = 0;
    let mut non_perfect_seeds = Vec::new();

    let first_seed = first_seed_opt.unwrap_or(rand::thread_rng().next_u32());
    info!("Initial seed: {}\n", first_seed);
    let mut histogram = HashMap::<Score, usize>::new();

    for i in 0..n_trials {
        if (i > 0) && (i % 1000 == 0) {
            let average: f32 = (total_score as f32) / (i as f32);
            info!("Trials: {}, Average so far: {}", i, average);
        }
        let seed = first_seed + i;
        let score = simulate_once(&opts, strat_config.initialize(&opts), Some(seed));
        let count = histogram.get(&score).unwrap_or(&0) + 1;
        histogram.insert(score, count);
        if score != 25 {
            non_perfect_seeds.push((score, seed));
        }
        total_score += score;
    }

    non_perfect_seeds.sort();
    info!("Score histogram: {:?}", histogram);
    info!("Seeds with non-perfect score: {:?}", non_perfect_seeds);
    let average: f32 = (total_score as f32) / (n_trials as f32);
    info!("Average score: {:?}", average);
    average
}
