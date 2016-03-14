use rand::{self, Rng};
use game::*;
use std::collections::HashMap;

// Trait to implement for any valid Hanabi strategy
// State management is done by the simulator, to avoid cheating
pub trait Strategy {
    fn decide(&mut self, &GameStateView) -> TurnChoice;
    fn update(&mut self, &Turn, &GameStateView);
}
pub trait StrategyConfig {
    fn initialize(&self, Player, &GameStateView) -> Box<Strategy>;
}

pub fn simulate_once<'a>(
        opts: &GameOptions,
        strat_configs: &Vec<Box<StrategyConfig + 'a>>,
        seed_opt: Option<u32>,
    ) -> Score {

    let seed = seed_opt.unwrap_or(rand::thread_rng().next_u32());

    let mut game = GameState::new(opts, seed);

    assert_eq!(opts.num_players, (strat_configs.len() as u32));

    let mut strategies : HashMap<Player, Box<Strategy>> = HashMap::new();
    let mut i = 0;
    for player in game.get_players() {
        strategies.insert(
            player,
            (*strat_configs[i]).initialize(player.clone(), &game.get_view(player)),
        );
        i += 1;
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
pub fn simulate<'a>(
        opts: &GameOptions,
        strat_configs: &Vec<Box<StrategyConfig + 'a>>,
        n_trials: u32,
        first_seed_opt: Option<u32>
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
        let score = simulate_once(&opts, strat_configs, Some(seed));
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

pub fn simulate_symmetric_once<'a, S: StrategyConfig + Clone + 'a>(
        opts: &GameOptions,
        strat_config: S,
        seed_opt: Option<u32>,
    ) -> Score {

    let mut strat_configs = Vec::new();
    for _ in 0..opts.num_players {
        strat_configs.push(Box::new(strat_config.clone()) as Box<StrategyConfig + 'a>);
    }
    simulate_once(opts, &strat_configs, seed_opt)
}

pub fn simulate_symmetric<'a, S: StrategyConfig + Clone + 'a>(
        opts: &GameOptions,
        strat_config: S,
        n_trials: u32,
        first_seed_opt: Option<u32>,
    ) -> f32 {

    let mut strat_configs = Vec::new();
    for _ in 0..opts.num_players {
        strat_configs.push(Box::new(strat_config.clone()) as Box<StrategyConfig + 'a>);
    }
    simulate(opts, &strat_configs, n_trials, first_seed_opt)
}
