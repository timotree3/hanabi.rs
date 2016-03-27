use std::cell::{RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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
pub struct CheatingStrategyConfig;

impl CheatingStrategyConfig {
    pub fn new() -> CheatingStrategyConfig {
        CheatingStrategyConfig
    }
}
impl GameStrategyConfig for CheatingStrategyConfig {
    fn initialize(&self, _: &GameOptions) -> Box<GameStrategy> {
        Box::new(CheatingStrategy::new())
    }
}

pub struct CheatingStrategy {
    player_states_cheat: Rc<RefCell<HashMap<Player, Cards>>>,
}

impl CheatingStrategy {
    pub fn new() -> CheatingStrategy {
        CheatingStrategy {
            player_states_cheat: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}
impl GameStrategy for CheatingStrategy {
    fn initialize(&self, player: Player, view: &GameStateView) -> Box<PlayerStrategy> {
        for (player, state) in &view.other_player_states {
            self.player_states_cheat.borrow_mut().insert(
                *player, state.hand.clone()
            );
        }
        Box::new(CheatingPlayerStrategy {
            player_states_cheat: self.player_states_cheat.clone(),
            me: player,
        })
    }
}

pub struct CheatingPlayerStrategy {
    player_states_cheat: Rc<RefCell<HashMap<Player, Cards>>>,
    me: Player,
}
impl CheatingPlayerStrategy {
    // last player might've drawn a new card, let him know!
    fn inform_last_player_cards(&self, view: &GameStateView) {
        let next = view.board.player_to_right(&self.me);
        self.player_states_cheat.borrow_mut().insert(
            next, view.other_player_states.get(&next).unwrap().hand.clone()
        );
    }

    // give a throwaway hint - we only do this when we have nothing to do
    fn throwaway_hint(&self, view: &GameStateView) -> TurnChoice {
        let hint_player = view.board.player_to_left(&self.me);
        let hint_card = &view.get_hand(&hint_player).first().unwrap();
        TurnChoice::Hint(Hint {
            player: hint_player,
            hinted: Hinted::Value(hint_card.value)
        })
    }

    // given a hand of cards, represents how badly it will need to play things
    fn hand_play_value(&self, view: &GameStateView, hand: &Cards/*, all_viewable: HashMap<Color, <Value, u32>> */) -> u32 {
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
                value += 20 - card.value;
            } else if view.board.is_playable(card) {
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
                    if their_hand_value < my_hand_value {
                        return 10 - (card.value as i32)
                    }
                }
            }
        }
        // there are no hints
        // maybe value 5s more?
        20 - (card.value as i32)
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
impl PlayerStrategy for CheatingPlayerStrategy {
    fn decide(&mut self, view: &GameStateView) -> TurnChoice {
        self.inform_last_player_cards(view);

        let states = self.player_states_cheat.borrow();
        let my_cards = states.get(&self.me).unwrap();
        let playable_cards = my_cards.iter().filter(|card| {
            view.board.is_playable(card)
        }).collect::<Vec<_>>();

        if playable_cards.len() > 0 {
            // play the best playable card
            // the higher the play_score, the better to play
            let mut play_card = None;
            let mut play_score = -1;

            for card in playable_cards {
                let score = self.get_play_score(view, card);
                if score > play_score {
                    play_card = Some(card);
                    play_score = score;
                }
            }

            let index = my_cards.iter().position(|card| {
                card == play_card.unwrap()
            }).unwrap();
            TurnChoice::Play(index)
        } else {
            // discard threshold is how many cards we're willing to discard
            // such that if we only played,
            // we would not reach the final countdown round
            // e.g. 50 total, 25 to play, 20 in hand
            let discard_threshold =
                view.board.total_cards
                - (COLORS.len() * VALUES.len()) as u32
                - (view.board.num_players * view.board.hand_size);
            if view.board.discard_size() <= discard_threshold {
                // if anything is totally useless, discard it
                if let Some(i) = self.find_useless_card(view, my_cards) {
                    return TurnChoice::Discard(i);
                }
            }

            // hinting is better than discarding dead cards
            // (probably because it stalls the deck-drawing).
            if view.board.hints_remaining > 0 {
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
                let index = my_cards.iter().position(|iter_card| {
                    card == iter_card
                }).unwrap();
                TurnChoice::Discard(index)
            } else {
                panic!("This shouldn't happen!  No discardable card");
            }
        }
    }
    fn update(&mut self, _: &Turn, _: &GameStateView) {
    }
}
