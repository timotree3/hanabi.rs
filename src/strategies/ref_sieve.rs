use crate::game::*;
use crate::strategy::*;

#[derive(Clone)]
pub struct Config;

impl GameStrategyConfig for Config {
    fn initialize(&self, _: &GameOptions) -> Box<dyn GameStrategy> {
        Box::new(Strategy)
    }
}

pub struct Strategy;
impl GameStrategy for Strategy {
    fn initialize(&self, player: Player, _: &BorrowedGameView) -> Box<dyn PlayerStrategy> {
        Box::new(RsPlayer { me: player })
    }
}

pub struct RsPlayer {
    me: Player,
}

impl PlayerStrategy for RsPlayer {
    fn decide(&mut self, view: &BorrowedGameView) -> TurnChoice {
        TurnChoice::Play(0)
    }
    fn update(&mut self, _: &TurnRecord, _: &BorrowedGameView) {}
}
