use crate::game::*;
use crate::strategy::*;
use rand;
use rand::seq::IteratorRandom;

// dummy, terrible strategy, as an example
#[derive(Clone)]
pub struct RandomStrategyConfig {
    pub hint_probability: f64,
    pub play_probability: f64,
}

impl GameStrategyConfig for RandomStrategyConfig {
    fn initialize(&self, _: &GameOptions) -> Box<dyn GameStrategy> {
        Box::new(RandomStrategy {
            hint_probability: self.hint_probability,
            play_probability: self.play_probability,
        })
    }
}

pub struct RandomStrategy {
    hint_probability: f64,
    play_probability: f64,
}
impl GameStrategy for RandomStrategy {
    fn initialize<'game>(
        &self,
        player: Player,
        _: &PlayerView<'game>,
    ) -> Box<dyn PlayerStrategy<'game>> {
        Box::new(RandomStrategyPlayer {
            hint_probability: self.hint_probability,
            play_probability: self.play_probability,
            me: player,
        })
    }
}

pub struct RandomStrategyPlayer {
    hint_probability: f64,
    play_probability: f64,
    me: Player,
}

impl<'game> PlayerStrategy<'game> for RandomStrategyPlayer {
    fn name(&self) -> String {
        format!(
            "random(hint={}, play={})",
            self.hint_probability, self.play_probability
        )
    }
    fn decide(&mut self, view: &PlayerView<'_>) -> TurnChoice {
        let p = rand::random::<f64>();
        if p < self.play_probability {
            TurnChoice::Play(0)
        } else if view.board.hints_remaining == view.board.opts.num_hints
            || (view.board.hints_remaining > 0 && p < self.play_probability + self.hint_probability)
        {
            let hint_player = view.board.player_to_left(self.me);
            let hint_card = view
                .hand(hint_player)
                .choose(&mut rand::thread_rng())
                .unwrap();
            let hinted = {
                if rand::random() {
                    // hint a color
                    Hinted::Color(hint_card.color)
                } else {
                    Hinted::Value(hint_card.value)
                }
            };
            TurnChoice::Hint(Hint {
                player: hint_player,
                hinted,
            })
        } else {
            TurnChoice::Discard(0)
        }
    }
    fn update(&mut self, _: &TurnRecord, _: &PlayerView<'_>) {}
}
