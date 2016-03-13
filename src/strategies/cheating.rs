use std::rc::Rc;
use std::cell::{RefCell};
use std::collections::HashMap;

use simulator::*;
use game::*;

// strategy that cheats by using Cell
// Plays according to the following rules:
//  - if any card is playable,
//      play the card with the lowest
//  - if a card is
#[allow(dead_code)]
#[derive(Clone)]
pub struct CheatingStrategyConfig {
    player_states_cheat: Rc<RefCell<HashMap<Player, Cards>>>,
}

impl CheatingStrategyConfig {
    pub fn new() -> CheatingStrategyConfig {
        CheatingStrategyConfig {
            player_states_cheat: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}
impl <'a> StrategyConfig for CheatingStrategyConfig {
    fn initialize(&self, _: &Player, _: &GameStateView) -> Box<Strategy> {
        Box::new(CheatingStrategy {
            player_states_cheat: self.player_states_cheat.clone(),
        })
    }
}
pub struct CheatingStrategy {
    player_states_cheat: Rc<RefCell<HashMap<Player, Cards>>>,
}
impl Strategy for CheatingStrategy {
    fn decide(&mut self, me: &Player, view: &GameStateView) -> TurnChoice {
        let next = view.board.player_to_left(&me);
        self.player_states_cheat.borrow_mut().insert(
            next, view.other_player_states.get(&next).unwrap().hand.clone()
        );
        if view.board.turn == 1 {
            TurnChoice::Hint(Hint {
                player: next,
                hinted: Hinted::Value(1)
            })
        } else {
            let states = self.player_states_cheat.borrow();
            let my_cards = states.get(me).unwrap();
            let mut playable_cards = my_cards.iter().filter(|card| {
                view.board.is_playable(card)
            }).peekable();
            if playable_cards.peek() == None {
                TurnChoice::Discard(0)
            } else {
                let mut play_card = playable_cards.next().unwrap();

                let mut next_card_opt = playable_cards.next();
                while let Some(next_card) = next_card_opt {
                    if next_card.value < play_card.value {
                        play_card = next_card;
                    }
                    next_card_opt = playable_cards.next();
                }

                let index = my_cards.iter().position(|card| {
                    card == play_card
                }).unwrap();
                TurnChoice::Play(index)
            }
        }
    }
    fn update(&mut self, _: &Turn, _: &GameStateView) {
    }
}
