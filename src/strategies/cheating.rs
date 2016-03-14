use std::rc::Rc;
use std::cell::{RefCell};
use std::collections::{HashMap, HashSet};

use simulator::*;
use game::*;

// strategy that explicitly cheats by using Rc/RefCell
// serves as a reference point for other strategies
//
// Plays according to the following rules:
//  - if any card is playable,
//      play the card with the lowest value
//  - if a card is dead, discard it
//  - if another player has same card in hand, discard it
//  - if a card is discardable, discard it
//  - if a hint exists, hint
//  - discard the first card

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

    // given a hand of cards, represents how badly it will need to play things
    fn hand_play_value(&self, view: &GameStateView, hand: &Cards/*, all_viewable: HashMap<Color, <Value, usize>> */) -> u32 {
        // dead = 0 points
        // indispensible = 5 + (5 - value) points
        // playable, not in another hand = 2 point
        // playable = 1 point
        let mut value = 0;
        for card in hand {
            if view.board.is_dead(card) {
                continue
            }
            if !view.board.is_dispensable(card) {
                value += 10 - card.value;
            } else {
                value += 1;
            }
        }
        value
    }

    // how badly do we need to play a particular card
    fn get_play_score(&self, view: &GameStateView, card: &Card) -> i32 {
        let states  = self.player_states_cheat.borrow();
        let my_hand = states.get(&self.me).unwrap();

        let my_hand_value = self.hand_play_value(view, my_hand);

        for player in view.board.get_players() {
            if player != self.me {
                if view.has_card(&player, card) {
                    let their_hand_value = self.hand_play_value(view, states.get(&player).unwrap());
                    // they can play this card, and have less urgent plays than i do
                    if their_hand_value <= my_hand_value {
                        return 1;
                    }
                }
            }
        }
        // there are no hints
        // maybe value 5s more?
        5 + (5 - (card.value as i32))
    }

    fn find_useless_card(&self, view: &GameStateView, hand: &Cards) -> Option<usize> {
        let mut set: HashSet<Card> = HashSet::new();

        for (i, card) in hand.iter().enumerate() {
            if view.board.is_dead(card) {
                return Some(i);
            }
            if set.contains(card) {
                // found a duplicate card
                return Some(i);
            }
            set.insert(card.clone());
        }
        return None
    }

    fn someone_else_can_play(&self, view: &GameStateView) -> bool {
        for player in view.board.get_players() {
            if player != self.me {
                for card in view.get_hand(&player) {
                    if view.board.is_playable(card) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
impl Strategy for CheatingStrategy {
    fn decide(&mut self, view: &GameStateView) -> TurnChoice {
        self.inform_next_player_cards(view);
        if view.board.turn <= view.board.num_players {
            // don't know my cards yet, just give a random hint
            return self.throwaway_hint(view);
        }

        let states = self.player_states_cheat.borrow();
        let my_cards = states.get(&self.me).unwrap();
        let mut playable_cards = my_cards.iter().filter(|card| {
            view.board.is_playable(card)
        }).peekable();

        if playable_cards.peek() == None {
            // if view.board.deck_size() > 10 {
            if view.board.discard.cards.len() < 5 {
                // if anything is totally useless, discard it
                if let Some(i) = self.find_useless_card(view, my_cards) {
                    return TurnChoice::Discard(i);
                }
            }

            // hinting is better than discarding dead cards
            // (probably because it stalls the deck-drawing).
            if view.board.hints_remaining > 1 {
                if self.someone_else_can_play(view) {
                    return self.throwaway_hint(view);
                }
            }
            // if anything is totally useless, discard it
            if let Some(i) = self.find_useless_card(view, my_cards) {
                return TurnChoice::Discard(i);
            }

            // All cards are plausibly useful.
            // Play the best discardable card, according to the ordering induced by comparing
            //   (is in another hand, is dispensable, value)
            // The higher, the better to discard
            let mut discard_card = None;
            let mut compval = (false, false, 0);
            for card in my_cards {
                let my_compval = (
                    view.can_see(card),
                    view.board.is_dispensable(card),
                    card.value,
                );
                if my_compval > compval {
                    discard_card = Some(card);
                    compval = my_compval;
                }
            }
            if let Some(card) = discard_card {
                if view.board.hints_remaining > 0 {
                    if !view.can_see(card) {
                        return self.throwaway_hint(view);
                    }
                }

                let index = my_cards.iter().position(|iter_card| {
                    card == iter_card
                }).unwrap();
                TurnChoice::Discard(index)
            } else {
                panic!("This shouldn't happen!  No discardable card");
            }
        } else {
            // play the best playable card
            // the higher the play_score, the better to play
            let mut play_card = None;
            let mut play_score = -1;

            while playable_cards.peek().is_some() {
                let next_card = playable_cards.next().unwrap();
                let next_play_score = self.get_play_score(view, next_card);
                if next_play_score > play_score {
                    play_card = Some(next_card);
                    play_score = next_play_score;
                }
            }

            let index = my_cards.iter().position(|card| {
                card == play_card.unwrap()
            }).unwrap();
            TurnChoice::Play(index)
        }
    }
    fn update(&mut self, _: &Turn, _: &GameStateView) {
    }
}
