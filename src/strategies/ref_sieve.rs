use crate::{
    game::{BoardState, GameOptions, Player, PlayerView},
    strategy::{GameStrategy, GameStrategyConfig, PlayerStrategy},
};

#[derive(Clone)]
pub struct Config;

impl GameStrategyConfig for Config {
    fn initialize(&self, opts: &GameOptions) -> Box<dyn GameStrategy> {
        Box::new(Strategy { opts: *opts })
    }
}

pub struct Strategy {
    opts: GameOptions,
}
impl GameStrategy for Strategy {
    fn initialize<'game>(
        &self,
        _: Player,
        view: &PlayerView<'game>,
    ) -> Box<dyn PlayerStrategy<'game>> {
        Box::new(RsPlayer {
            public: Public::first_turn(&view.board),
            opts: self.opts,
        })
    }
}

// How do we deal with plays that are stacked on top of plays in giver's hand?
// Simple options:
// - Never do it
// - Superposition includes duplicate of first card in each suit played from giver's hand,
//   as well as cards on top of each card played from giver's hand at time of clue before the card had permission to play
// Complicated option: Pay attention to which plays are publicly known are for the non-public ones, consider what it would take from the hand to make them known
//   - For each card, keep track of its useful-unplayable identities

struct Public {}

impl Public {
    fn first_turn(board: &BoardState<'_>) -> Public {
        Public {}
    }
}

struct RsPlayer {
    /// The public knowledge shared amongst the players
    public: Public,
    opts: GameOptions,
}

impl<'game> PlayerStrategy<'game> for RsPlayer {
    fn name(&self) -> String {
        "rs".to_owned()
    }

    fn decide(&mut self, view: &crate::game::PlayerView) -> crate::game::TurnChoice {
        todo!()
    }

    fn update(&mut self, turn_record: &crate::game::TurnRecord, view: &crate::game::PlayerView) {
        todo!()
    }
}
