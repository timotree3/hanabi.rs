use rand::{self, Rng};
use std::collections::HashMap;
use std::fmt;
use crossbeam;

use game::*;

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

    while !game.is_over() {
        let player = game.board.player;

        debug!("");
        debug!("=======================================================");
        debug!("Turn {}, Player {} to go", game.board.turn, player);
        debug!("=======================================================");
        debug!("{}", game);


        let choice = {
            let mut strategy = strategies.get_mut(&player).unwrap();
            strategy.decide(&game.get_view(player))
        };

        let turn = game.process_choice(choice);

        for player in game.get_players() {
            let mut strategy = strategies.get_mut(&player).unwrap();
            strategy.update(&turn, &game.get_view(player));
        }

    }
    debug!("");
    debug!("=======================================================");
    debug!("Final state:\n{}", game);
    let score = game.score();
    debug!("SCORED: {:?}", score);
    score
}

struct Histogram {
    pub hist: HashMap<Score, u32>,
    pub sum: Score,
    pub total_count: u32,
}
impl Histogram {
    pub fn new() -> Histogram {
        Histogram {
            hist: HashMap::new(),
            sum: 0,
            total_count: 0,
        }
    }
    fn insert_many(&mut self, val: Score, count: u32) {
        let new_count = self.get_count(&val) + count;
        self.hist.insert(val, new_count);
        self.sum += val * (count as u32);
        self.total_count += count;
    }
    pub fn insert(&mut self, val: Score) {
        self.insert_many(val, 1);
    }
    pub fn get_count(&self, val: &Score) -> u32 {
        *self.hist.get(&val).unwrap_or(&0)
    }
    pub fn average(&self) -> f32 {
        (self.sum as f32) / (self.total_count as f32)
    }
    pub fn merge(&mut self, other: Histogram) {
        for (val, count) in other.hist.iter() {
            self.insert_many(*val, *count);
        }
    }
}
impl fmt::Display for Histogram {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut keys = self.hist.keys().collect::<Vec<_>>();
        keys.sort();
        for val in keys {
            try!(f.write_str(&format!(
                "{}: {}\n", val, self.get_count(val),
            )));
        }
        Ok(())
    }
}

pub fn simulate<T: ?Sized>(
        opts: &GameOptions,
        strat_config: Box<T>,
        first_seed_opt: Option<u32>,
        n_trials: u32,
        n_threads: u32,
    ) -> f32 where T: GameStrategyConfig + Sync {

    let first_seed = first_seed_opt.unwrap_or(rand::thread_rng().next_u32());

    let strat_config_ref = &strat_config;
    crossbeam::scope(|scope| {
        let mut join_handles = Vec::new();
        for i in 0..n_threads {
            let start = first_seed + ((n_trials * i) / n_threads);
            let end = first_seed + ((n_trials * (i+1)) / n_threads);
            join_handles.push(scope.spawn(move || {
                info!("Thread {} spawned: seeds {} to {}", i, start, end);
                let mut non_perfect_seeds = Vec::new();

                let mut histogram = Histogram::new();

                for seed in start..end {
                    if (seed > start) && ((seed-start) % 1000 == 0) {
                        info!(
                            "Thread {}, Trials: {}, Average so far: {}",
                            i, seed-start, histogram.average()
                        );
                    }
                    let score = simulate_once(&opts, strat_config_ref.initialize(&opts), Some(seed));
                    histogram.insert(score);
                    if score != 25 { non_perfect_seeds.push((score, seed)); }
                }
                info!("Thread {} done", i);
                (non_perfect_seeds, histogram)
            }));
        }

        let mut non_perfect_seeds : Vec<(Score,u32)> = Vec::new();
        let mut histogram = Histogram::new();
        for join_handle in join_handles {
            let (thread_non_perfect_seeds, thread_histogram) = join_handle.join();
            non_perfect_seeds.extend(thread_non_perfect_seeds.iter());
            histogram.merge(thread_histogram);
        }

        info!("Score histogram:\n{}", histogram);

        non_perfect_seeds.sort();
        // info!("Seeds with non-perfect score: {:?}", non_perfect_seeds);
        if non_perfect_seeds.len() > 0 {
            info!("Example seed with non-perfect score: {}",
                  non_perfect_seeds.get(0).unwrap().1);
        }

        let percentage = (n_trials - non_perfect_seeds.len() as u32) as f32 / n_trials as f32;
        info!("Percentage perfect: {:?}%", percentage);
        let average = histogram.average();
        info!("Average score: {:?}", average);
        average
    })
}
