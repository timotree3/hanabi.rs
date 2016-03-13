use simulator::*;
use game::*;
use rand::{self, Rng};

// dummy, terrible strategy
#[allow(dead_code)]
#[derive(Clone)]
pub struct AlwaysPlayConfig;
impl StrategyConfig for AlwaysPlayConfig {
    fn initialize(&self, _: Player, _: &GameStateView) -> Box<Strategy> {
        Box::new(AlwaysPlay)
    }
}
pub struct AlwaysPlay;
impl Strategy for AlwaysPlay {
    fn decide(&mut self, _: &GameStateView) -> TurnChoice {
        TurnChoice::Play(0)
    }
    fn update(&mut self, _: &Turn, _: &GameStateView) {
    }
}

// dummy, terrible strategy
#[allow(dead_code)]
#[derive(Clone)]
pub struct AlwaysDiscardConfig;
impl StrategyConfig for AlwaysDiscardConfig {
    fn initialize(&self, _: Player, _: &GameStateView) -> Box<Strategy> {
        Box::new(AlwaysDiscard)
    }
}
pub struct AlwaysDiscard;
impl Strategy for AlwaysDiscard {
    fn decide(&mut self, _: &GameStateView) -> TurnChoice {
        TurnChoice::Discard(0)
    }
    fn update(&mut self, _: &Turn, _: &GameStateView) {
    }
}


// dummy, terrible strategy
#[allow(dead_code)]
#[derive(Clone)]
pub struct RandomStrategyConfig {
    pub hint_probability: f64,
    pub play_probability: f64,
}

impl StrategyConfig for RandomStrategyConfig {
    fn initialize(&self, player: Player, _: &GameStateView) -> Box<Strategy> {
        Box::new(RandomStrategy {
            hint_probability: self.hint_probability,
            play_probability: self.play_probability,
            me: player,
        })
    }
}
pub struct RandomStrategy {
    hint_probability: f64,
    play_probability: f64,
    me: Player,
}

impl Strategy for RandomStrategy {
    fn decide(&mut self, view: &GameStateView) -> TurnChoice {
        let p = rand::random::<f64>();
        if p < self.hint_probability {
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
                    player: view.board.player_to_left(&self.me),
                    hinted: hinted,
                })
            } else {
                TurnChoice::Discard(0)
            }
        } else if p < self.hint_probability + self.play_probability {
            TurnChoice::Play(0)
        } else {
            TurnChoice::Discard(0)
        }
    }
    fn update(&mut self, _: &Turn, _: &GameStateView) {
    }
}
