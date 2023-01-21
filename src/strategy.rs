use crate::game::*;

// Traits to implement for any valid Hanabi strategy

// Represents the strategy of a given player
pub trait PlayerStrategy<'game> {
    // A function returning the name of a strategy.
    // This is a method of PlayerStrategy rather than GameStrategyConfig
    // so that the name may incorporate useful information that's specific
    // to this player instance.
    fn name(&self) -> String;
    // A function to decide what to do on the player's turn.
    // Given a BorrowedGameView, outputs their choice.
    //
    // If `decide` returns None, the game is terminated.
    fn decide(&mut self, view: &PlayerView<'game>) -> Option<TurnChoice>;
    // A function to update internal state after other players' turns.
    // Given what happened last turn, and the new state.
    fn update(&mut self, turn_record: &TurnRecord, view: &PlayerView<'game>);
}
// Represents the overall strategy for a game
// Shouldn't do much, except store configuration parameters and
// possibility initialize some shared randomness between players
pub trait GameStrategy {
    fn initialize<'game>(&self, view: &PlayerView<'game>)
        -> Box<dyn PlayerStrategy<'game> + 'game>;
}

// Represents configuration for a strategy.
// Acts as a factory for game strategies, so we can play many rounds
pub trait GameStrategyConfig {
    fn initialize(&self, opts: &GameOptions) -> Box<dyn GameStrategy>;
}
