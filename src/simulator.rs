use fnv::FnvHashMap;
use rand::prelude::SliceRandom;
use rand::RngCore;
use rand::{self, SeedableRng};
use rand_chacha::ChaChaRng;
use std::fmt;
use tracing::{debug, info};

use crate::game::*;
use crate::helpers::PerPlayer;
use crate::json_output::*;
use crate::strategy::*;

pub fn new_deck(seed: u64) -> Cards {
    let mut deck: Cards = Cards::new();

    for &color in COLORS.iter() {
        for &value in VALUES.iter() {
            for _ in 0..get_count_for_value(value) {
                deck.push(Card::new(color, value));
            }
        }
    }

    deck.shuffle(&mut ChaChaRng::seed_from_u64(seed));
    debug!("Deck: {:?}", deck);
    deck
}

pub fn simulate_once(
    opts: &GameOptions,
    game_strategy: Box<dyn GameStrategy>,
    seed: u64,
    output_json: bool,
) -> (GameState, Option<serde_json::Value>) {
    let deck = new_deck(seed);

    let mut game = GameState::new(opts, deck.clone());

    let mut strategies = PerPlayer::new(opts.num_players, |player| {
        game_strategy.initialize(player, &game.get_view(player))
    });

    let mut actions = Vec::new();

    while !game.is_over() {
        let player = game.board.player;

        debug!("");
        debug!("=======================================================");
        debug!("Turn {}, Player {} to go", game.board.turn, player);
        debug!("=======================================================");
        debug!("{}", game);

        let choice = strategies[player].decide(&game.get_view(player));
        if output_json {
            actions.push(match choice {
                TurnChoice::Hint(ref hint) => action_clue(hint),
                TurnChoice::Play(index) => {
                    let card = &game.hands[player][index];
                    action_play(card)
                }
                TurnChoice::Discard(index) => {
                    let card = &game.hands[player][index];
                    action_discard(card)
                }
            });
        }

        let turn = game.process_choice(choice);

        for player in game.get_players() {
            strategies[player].update(&turn, &game.get_view(player));
        }
    }
    debug!("");
    debug!("=======================================================");
    debug!("Final state:\n{}", game);
    debug!("SCORE: {:?}", game.score());
    let json_output = if output_json {
        let player_names = game
            .get_players()
            .map(|player| strategies[player].name())
            .collect();
        Some(json_format(&deck, &actions, &player_names))
    } else {
        None
    };
    (game, json_output)
}

#[derive(Debug)]
pub struct Histogram {
    pub hist: FnvHashMap<Score, u32>,
    pub sum: Score,
    pub total_count: u32,
}
impl Histogram {
    pub fn new() -> Histogram {
        Histogram {
            hist: FnvHashMap::default(),
            sum: 0,
            total_count: 0,
        }
    }
    fn insert_many(&mut self, val: Score, count: u32) {
        let new_count = self.get_count(&val) + count;
        self.hist.insert(val, new_count);
        self.sum += val * count;
        self.total_count += count;
    }
    pub fn insert(&mut self, val: Score) {
        self.insert_many(val, 1);
    }
    pub fn get_count(&self, val: &Score) -> u32 {
        *self.hist.get(val).unwrap_or(&0)
    }
    pub fn percentage_with(&self, val: &Score) -> f32 {
        self.get_count(val) as f32 / self.total_count as f32
    }
    pub fn average(&self) -> f32 {
        (self.sum as f32) / (self.total_count as f32)
    }
    pub fn stdev_of_average(&self) -> f32 {
        let average = self.average();
        let mut var_sum = 0.0;
        for (&val, &count) in self.hist.iter() {
            var_sum += (val as f32 - average).powi(2) * count as f32;
        }
        // Divide by (self.total_count - 1) estimate the variance of the distribution,
        // then divide by self.total_count estimate the variance of the sample average,
        // then take the sqrt to get the stdev.
        (var_sum / (((self.total_count - 1) * self.total_count) as f32)).sqrt()
    }
    pub fn merge(&mut self, other: Histogram) {
        for (val, count) in other.hist.into_iter() {
            self.insert_many(val, count);
        }
    }
}
impl fmt::Display for Histogram {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut keys = self.hist.keys().collect::<Vec<_>>();
        keys.sort();
        for val in keys {
            write!(f, "\n{}: {}", val, self.get_count(val))?;
        }
        Ok(())
    }
}

pub fn simulate<T: ?Sized>(
    opts: &GameOptions,
    strat_config: Box<T>,
    first_seed_opt: Option<u64>,
    n_trials: u32,
    n_threads: u32,
    progress_info: Option<u32>,
    json_output_pattern: Option<String>,
    json_losses_only: bool,
) -> SimResult
where
    T: GameStrategyConfig + Sync,
{
    let first_seed = first_seed_opt.unwrap_or_else(|| rand::thread_rng().next_u64());

    let strat_config_ref = &strat_config;
    let json_output_pattern_ref = &json_output_pattern;
    crossbeam::scope(|scope| {
        let mut join_handles = Vec::new();
        for i in 0..n_threads {
            let start = first_seed + u64::from((n_trials * i) / n_threads);
            let end = first_seed + u64::from((n_trials * (i + 1)) / n_threads);
            join_handles.push(scope.spawn(move || {
                if progress_info.is_some() {
                    info!("Thread {} spawned: seeds {} to {}", i, start, end);
                }
                let mut non_perfect_seeds = Vec::new();

                let mut score_histogram = Histogram::new();
                let mut lives_histogram = Histogram::new();

                for seed in start..end {
                    if let Some(progress_info_frequency) = progress_info {
                        if (seed > start)
                            && ((seed - start) % u64::from(progress_info_frequency) == 0)
                        {
                            info!(
                                "Thread {}, Trials: {}, Stats so far: {} score, {} lives, {}% win",
                                i,
                                seed - start,
                                score_histogram.average(),
                                lives_histogram.average(),
                                score_histogram.percentage_with(&PERFECT_SCORE) * 100.0
                            );
                        }
                    }
                    let (game, json_output) = simulate_once(
                        opts,
                        strat_config_ref.initialize(opts),
                        seed,
                        json_output_pattern_ref.is_some(),
                    );
                    let score = game.score();
                    lives_histogram.insert(game.board.lives_remaining);
                    score_histogram.insert(score);
                    if score != PERFECT_SCORE {
                        non_perfect_seeds.push(seed);
                    }
                    if let Some(file_pattern) = json_output_pattern_ref {
                        if !(score == PERFECT_SCORE && json_losses_only) {
                            let file_pattern =
                                file_pattern.clone().replace("%s", &seed.to_string());
                            let path = std::path::Path::new(&file_pattern);
                            let file = std::fs::File::create(path).unwrap();
                            serde_json::to_writer(file, &json_output.unwrap()).unwrap();
                        }
                    }
                }
                if progress_info.is_some() {
                    info!("Thread {} done", i);
                }
                (non_perfect_seeds, score_histogram, lives_histogram)
            }));
        }

        let mut non_perfect_seeds: Vec<u64> = Vec::new();
        let mut score_histogram = Histogram::new();
        let mut lives_histogram = Histogram::new();
        for join_handle in join_handles {
            let (thread_non_perfect_seeds, thread_score_histogram, thread_lives_histogram) =
                join_handle.join();
            non_perfect_seeds.extend(thread_non_perfect_seeds.iter());
            score_histogram.merge(thread_score_histogram);
            lives_histogram.merge(thread_lives_histogram);
        }

        non_perfect_seeds.sort_unstable();
        SimResult {
            scores: score_histogram,
            lives: lives_histogram,
            non_perfect_seed: non_perfect_seeds.first().cloned(),
        }
    })
}

pub struct SimResult {
    pub scores: Histogram,
    pub lives: Histogram,
    pub non_perfect_seed: Option<u64>,
}

impl SimResult {
    pub fn percent_perfect(&self) -> f32 {
        self.scores.percentage_with(&PERFECT_SCORE) * 100.0
    }

    pub fn percent_perfect_stderr(&self) -> f32 {
        let pp = self.percent_perfect() / 100.0;
        let stdev = (pp * (1.0 - pp) / ((self.scores.total_count - 1) as f32)).sqrt();
        stdev * 100.0
    }

    pub fn average_score(&self) -> f32 {
        self.scores.average()
    }

    pub fn score_stderr(&self) -> f32 {
        self.scores.stdev_of_average()
    }

    pub fn average_lives(&self) -> f32 {
        self.lives.average()
    }

    pub fn info(&self) {
        info!("Score histogram:\n{}", self.scores);

        // info!("Seeds with non-perfect score: {:?}", non_perfect_seeds);
        if let Some(seed) = self.non_perfect_seed {
            info!("Example seed with non-perfect score: {}", seed);
        }

        info!("Percentage perfect: {:?}%", self.percent_perfect());
        info!("Average score: {:?}", self.average_score());
        info!("Average lives: {:?}", self.average_lives());
    }
}
