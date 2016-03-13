use std::rc::Rc;
use std::cell::{RefCell};
use std::collections::HashMap;

use simulator::*;
use game::*;

// strategy that cheats by using Rc/RefCell
// Plays according to the following rules:
//  - if any card is playable,
//      play the card with the lowest value
//  - if a card is dead, discard it
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
    fn initialize(&self, player: Player, _: &GameStateView) -> Box<Strategy> {
        Box::new(CheatingStrategy {
            player_states_cheat: self.player_states_cheat.clone(),
            me: player,
        })
    }
}
pub struct CheatingStrategy {
    player_states_cheat: Rc<RefCell<HashMap<Player, Cards>>>,
    me: Player,
}
impl CheatingStrategy {
    // help next player cheat!
    fn inform_next_player_cards(&self, view: &GameStateView) {
        let next = view.board.player_to_left(&self.me);
        self.player_states_cheat.borrow_mut().insert(
            next, view.other_player_states.get(&next).unwrap().hand.clone()
        );
    }
    // give a throwaway hint - we only do this when we have nothing to do
    fn throwaway_hint(&self, view: &GameStateView) -> TurnChoice {
        TurnChoice::Hint(Hint {
                player: view.board.player_to_left(&self.me),
                hinted: Hinted::Value(1)
        })
    }
}
impl Strategy for CheatingStrategy {
    fn decide(&mut self, view: &GameStateView) -> TurnChoice {
        self.inform_next_player_cards(view);
        if view.board.turn == 1 {
            // don't know my cards yet, just give a random hint
            return self.throwaway_hint(view);
        }

        let states = self.player_states_cheat.borrow();
        let my_cards = states.get(&self.me).unwrap();
        let mut playable_cards = my_cards.iter().filter(|card| {
            view.board.is_playable(card)
        }).peekable();

        if playable_cards.peek() == None {
            for card in my_cards {
                if view.board.is_unplayable(card) {
                    let index = my_cards.iter().position(|iter_card| {
                        card == iter_card
                    }).unwrap();
                    return TurnChoice::Discard(index);
                }
            }
            for card in my_cards {
                if !view.board.is_undiscardable(card) {
                    let index = my_cards.iter().position(|iter_card| {
                        card == iter_card
                    }).unwrap();
                    return TurnChoice::Discard(index);
                }
            }
            // all my cards are undiscardable!
            if view.board.hints_remaining > 0 {
                return self.throwaway_hint(view);
            }
            TurnChoice::Discard(0)
        } else {
            // play the lowest playable card
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
    fn update(&mut self, _: &Turn, _: &GameStateView) {
    }
}
