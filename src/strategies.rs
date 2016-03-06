use game::*;
use std::collections::HashMap;

// Trait to implement for any valid Hanabi strategy
// State management is done by the simulator, to avoid cheating
pub trait Strategy {
    type InternalState;
    fn initialize(&Player, &GameStateView) -> Self::InternalState;
    fn decide(&mut Self::InternalState, &Player, &GameStateView) -> TurnChoice;
    fn update(&mut Self::InternalState, &Turn, &GameStateView);
}

pub fn simulate<S: Strategy>(opts: GameOptions, strategy: S) -> Score {
    let mut game = GameState::new(opts);

    let mut internal_states : HashMap<Player, S::InternalState> = HashMap::new();
    for player in game.get_players() {
        internal_states.insert(
            player,
            S::initialize(&player, &game.get_view(player)),
        );
    }

    while !game.is_over() {
        let player = game.board.player;
        let choice = {
            let ref mut internal_state = internal_states.get_mut(&player).unwrap();
            S::decide(internal_state, &player, &game.get_view(player))
        };
        println!("Player {:?} decided to {:?}", player, choice);
        let turn = Turn {
            player: &player,
            choice: &choice,
        };

        for player in game.get_players() {
            let ref mut internal_state = internal_states.get_mut(&player).unwrap();

            S::update(internal_state, &turn, &game.get_view(player));
        }

        // TODO: do some stuff
        println!("State: {:?}", game);
    }
    game.score()
}

pub struct AlwaysPlay;
impl Strategy for AlwaysPlay {
    type InternalState = ();
    fn initialize(player: &Player, view: &GameStateView) -> () {
        ()
    }
    fn decide(_: &mut (), player: &Player, view: &GameStateView) -> TurnChoice {
        TurnChoice::Play(0)
    }
    fn update(_: &mut (), turn: &Turn, view: &GameStateView) {
    }
}
