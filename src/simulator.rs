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
        seed_opt: Option<u32>,
        strat_configs: &Vec<Box<StrategyConfig + 'a>>
    ) -> Score {

    let seed = if let Some(seed) = seed_opt {
        seed
    } else {
        rand::thread_rng().next_u32()
    };

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
    game.score()
}

pub fn simulate<'a>(opts: &GameOptions, strat_configs: &Vec<Box<StrategyConfig + 'a>>, n_trials: u32) -> f32 {
    let mut total_score = 0;
    for seed in 0..n_trials {
        let score = simulate_once(&opts, Some(seed), strat_configs);
        debug!("Scored: {:?}", score);
        if score != 25 {
            info!("Seed with non-perfect score: {:?}", seed);
        }
        total_score += score;
    }
    let average: f32 = (total_score as f32) / (n_trials as f32);
    info!("Average score: {:?}", average);
    average
}

pub fn simulate_symmetric_once<'a, S: StrategyConfig + Clone + 'a>(
        opts: &GameOptions,
        seed_opt: Option<u32>,
        strat_config: S
    ) -> Score {

    let mut strat_configs = Vec::new();
    for _ in 0..opts.num_players {
        strat_configs.push(Box::new(strat_config.clone()) as Box<StrategyConfig + 'a>);
    }
    simulate_once(opts, seed_opt, &strat_configs)
}

pub fn simulate_symmetric<'a, S: StrategyConfig + Clone + 'a>(opts: &GameOptions, strat_config: S, n_trials: u32) -> f32 {
    let mut strat_configs = Vec::new();
    for _ in 0..opts.num_players {
        strat_configs.push(Box::new(strat_config.clone()) as Box<StrategyConfig + 'a>);
    }
    simulate(opts, &strat_configs, n_trials)
}
