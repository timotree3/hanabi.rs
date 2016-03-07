use game::*;
use std::collections::HashMap;
use rand::{self, Rng};

// Trait to implement for any valid Hanabi strategy
// State management is done by the simulator, to avoid cheating
pub trait Strategy {
    type InternalState;
    fn initialize(&Player, &GameStateView) -> Self::InternalState;
    fn decide(&mut Self::InternalState, &Player, &GameStateView) -> TurnChoice;
    fn update(&mut Self::InternalState, &Turn, &GameStateView);
}

pub fn simulate_once<S: Strategy>(opts: &GameOptions, _: &S) -> Score {
    let mut game = GameState::new(opts);

    let mut internal_states : HashMap<Player, S::InternalState> = HashMap::new();
    for player in game.get_players() {
        internal_states.insert(
            player,
            S::initialize(&player, &game.get_view(player)),
        );
    }

    while !game.is_over() {
        debug!("Turn {}", game.board.turn);
        let player = game.board.player;
        let choice = {
            let ref mut internal_state = internal_states.get_mut(&player).unwrap();
            S::decide(internal_state, &player, &game.get_view(player))
        };

        game.process_choice(&choice);

        let turn = Turn {
            player: &player,
            choice: &choice,
        };

        for player in game.get_players() {
            let ref mut internal_state = internal_states.get_mut(&player).unwrap();

            S::update(internal_state, &turn, &game.get_view(player));
        }

        // TODO: do some stuff
        debug!("State: {:?}", game);
    }
    game.score()
}

pub fn simulate<S: Strategy>(opts: &GameOptions, strategy: &S, n_trials: u32) -> f32 {
    let mut total_score = 0;
    for _ in 0..n_trials {
        let score = simulate_once(&opts, strategy);
        info!("Scored: {:?}", score);
        total_score += score;
    }
    let average: f32 = (total_score as f32) / (n_trials as f32);
    info!("Average score: {:?}", average);
    average
}

// dummy, terrible strategy
#[allow(dead_code)]
pub struct AlwaysPlay;
impl Strategy for AlwaysPlay {
    type InternalState = ();
    fn initialize(_: &Player, _: &GameStateView) -> () {
        ()
    }
    fn decide(_: &mut (), _: &Player, _: &GameStateView) -> TurnChoice {
        TurnChoice::Play(0)
    }
    fn update(_: &mut (), _: &Turn, _: &GameStateView) {
    }
}

// dummy, terrible strategy
#[allow(dead_code)]
pub struct AlwaysDiscard;
impl Strategy for AlwaysDiscard {
    type InternalState = ();
    fn initialize(_: &Player, _: &GameStateView) -> () {
        ()
    }
    fn decide(_: &mut (), _: &Player, _: &GameStateView) -> TurnChoice {
        TurnChoice::Discard(0)
    }
    fn update(_: &mut (), _: &Turn, _: &GameStateView) {
    }
}

// dummy, terrible strategy
#[allow(dead_code)]
pub struct RandomStrategy;
impl Strategy for RandomStrategy {
    type InternalState = ();
    fn initialize(_: &Player, _: &GameStateView) -> () {
        ()
    }
    fn decide(_: &mut (), _: &Player, view: &GameStateView) -> TurnChoice {
        let p = rand::random::<f64>();
        if p < 0.4 {
            if view.board.hints_remaining > 0 {
                let hinted = {
                    if rand::random() {
                        // hint a color
                        Hinted::Color(rand::thread_rng().choose(&COLORS).unwrap())
                    } else {
                        Hinted::Value(*rand::thread_rng().choose(&VALUES).unwrap())
                    }
                };
                TurnChoice::Hint(Hint {
                    player: 0,
                    hinted: hinted,
                })
            } else {
                TurnChoice::Discard(0)
            }
        } else if p < 0.8 {
            TurnChoice::Discard(0)
        } else {
            TurnChoice::Play(0)
        }
    }
    fn update(_: &mut (), _: &Turn, _: &GameStateView) {
    }
}
