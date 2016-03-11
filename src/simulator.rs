use game::*;
use std::collections::HashMap;

// Trait to implement for any valid Hanabi strategy
// State management is done by the simulator, to avoid cheating
pub trait Strategy {
    fn decide(&mut self, &Player, &GameStateView) -> TurnChoice;
    fn update(&mut self, &Turn, &GameStateView);
}
pub trait StrategyConfig {
    fn initialize(&self, &Player, &GameStateView) -> Box<Strategy>;
}

pub fn simulate_once<'a>(opts: &GameOptions, strat_configs: &Vec<Box<StrategyConfig + 'a>>) -> Score {
    let mut game = GameState::new(opts);

    assert_eq!(opts.num_players, (strat_configs.len() as u32));

    let mut strategies : HashMap<Player, Box<Strategy>> = HashMap::new();
    let mut i = 0;
    for player in game.get_players() {
        strategies.insert(
            player,
            (*strat_configs[i]).initialize(&player, &game.get_view(player)),
        );
        i += 1;
    }

    while !game.is_over() {
        debug!("Turn {}", game.board.turn);
        let player = game.board.player;
        let choice = {
            let mut strategy = strategies.get_mut(&player).unwrap();
            strategy.decide(&player, &game.get_view(player))
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
    for _ in 0..n_trials {
        let score = simulate_once(&opts, strat_configs);
        info!("Scored: {:?}", score);
        total_score += score;
    }
    let average: f32 = (total_score as f32) / (n_trials as f32);
    info!("Average score: {:?}", average);
    average
}

pub fn simulate_symmetric<'a, S: StrategyConfig + Clone + 'a>(opts: &GameOptions, strat_config: S, n_trials: u32) -> f32 {
    let mut strat_configs = Vec::new();
    for _ in 0..opts.num_players {
        strat_configs.push(Box::new(strat_config.clone()) as Box<StrategyConfig + 'a>);
    }
    simulate(opts, &strat_configs, n_trials)
}
