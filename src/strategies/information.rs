use std::collections::{HashMap, HashSet};
use rand::{self, Rng};

use simulator::*;
use game::*;

// strategy that recommends other players an action.
//
// 50 cards, 25 plays, 25 left
// with 5 players:
//  - only 5 + 8 hints total.  each player goes through 10 cards
// with 4 players:
//  - only 9 + 8 hints total.  each player goes through 12.5 cards
//
// For any given player with at least 4 cards, and index i, there are at least 3 hints that can be given.
// 1. a value hint on card i
// 2. a color hint on card i
// 3. any hint not involving card i
//
// for 4 players, can give 6 distinct hints

struct ModulusInformation {
    modulus: u32,
    value: u32,
}

enum Question {
    IsPlayable(usize),
    IsDead(usize),
}

fn answer_question(question: Question, hand: &Cards, view: &GameStateView) -> ModulusInformation {
    match question {
        Question::IsPlayable(index) => {
            let ref card = hand[index];
            ModulusInformation {
                modulus: 2,
                value: if view.board.is_playable(card) { 1 } else { 0 },
            }
        },
        Question::IsDead(index) => {
            let ref card = hand[index];
            ModulusInformation {
                modulus: 2,
                value: if view.board.is_dead(card) { 1 } else { 0 },
            }
        },
    }
}

#[allow(dead_code)]
pub struct InformationStrategyConfig;

impl InformationStrategyConfig {
    pub fn new() -> InformationStrategyConfig {
        InformationStrategyConfig
    }
}
impl GameStrategyConfig for InformationStrategyConfig {
    fn initialize(&self, opts: &GameOptions) -> Box<GameStrategy> {
        if opts.num_players < 4 {
            panic!("Information strategy doesn't work with less than 4 players");
        }
        Box::new(InformationStrategy::new())
    }
}

pub struct InformationStrategy;

impl InformationStrategy {
    pub fn new() -> InformationStrategy {
        InformationStrategy
    }
}
impl GameStrategy for InformationStrategy {
    fn initialize(&self, player: Player, view: &GameStateView) -> Box<PlayerStrategy> {
        let mut public_info = HashMap::new();
        for player in view.board.get_players() {
            let hand_info = (0..view.board.hand_size).map(|_| { CardPossibilityTable::new() }).collect::<Vec<_>>();
            public_info.insert(player, hand_info);
        }
        Box::new(InformationPlayerStrategy {
            me: player,
            public_info: public_info,
            public_counts: CardCounts::new(),
        })
    }
}

pub struct InformationPlayerStrategy {
    me: Player,
    public_info: HashMap<Player, Vec<CardPossibilityTable>>,
    public_counts: CardCounts, // what any newly drawn card should be
}
impl InformationPlayerStrategy {
    // given a hand of cards, represents how badly it will need to play things
    fn hand_play_value(&self, view: &GameStateView, hand: &Cards/*, all_viewable: HashMap<Color, <Value, usize>> */) -> u32 {
        // dead = 0 points
        // indispensible = 5 + (5 - value) points
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

    fn estimate_hand_play_value(&self, view: &GameStateView) -> u32 {
        0
    }

    // how badly do we need to play a particular card
    fn get_average_play_score(&self, view: &GameStateView, card_table: &CardPossibilityTable) -> f32 {
        let f = |card: &Card| {
            self.get_play_score(view, card) as f32
        };
        card_table.weighted_score(&f)
    }

    fn get_play_score(&self, view: &GameStateView, card: &Card) -> i32 {
        let my_hand_value = self.estimate_hand_play_value(view);

        for player in view.board.get_players() {
            if player != self.me {
                if view.has_card(&player, card) {
                    let their_hand_value = self.hand_play_value(view, view.get_hand(&player));
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

    fn get_player_public_info(&self, player: &Player) -> &Vec<CardPossibilityTable> {
        self.public_info.get(player).unwrap()
    }

    fn get_player_public_info_mut(&mut self, player: &Player) -> &mut Vec<CardPossibilityTable> {
        self.public_info.get_mut(player).unwrap()
    }

    fn update_public_info_for_hint(&mut self, hint: &Hint, matches: &Vec<bool>) {
        let mut info = self.get_player_public_info_mut(&hint.player);
        let zip_iter = info.iter_mut().zip(matches);
        match hint.hinted {
            Hinted::Color(ref color) => {
                for (card_info, matched) in zip_iter {
                    card_info.mark_color(color, *matched);
                }
            }
            Hinted::Value(ref value) => {
                for (card_info, matched) in zip_iter {
                    card_info.mark_value(value, *matched);
                }
            }

        }
    }

    fn update_public_info_for_discard_or_play(
        &mut self,
        view: &GameStateView,
        player: &Player,
        index: usize,
        card: &Card
    ) {
        let new_card_table = CardPossibilityTable::from(&self.public_counts);
        {
            let mut info = self.get_player_public_info_mut(&player);
            assert!(info[index].is_possible(card));
            info.remove(index);

            // push *before* incrementing public counts
            if info.len() < view.info.len() {
                info.push(new_card_table);
            }
        }

        // note: other_player could be player, as well
        // in particular, we will decrement the newly drawn card
        for other_player in view.board.get_players() {
            let mut info = self.get_player_public_info_mut(&other_player);
            for card_table in info {
                card_table.decrement_weight_if_possible(card);
            }
        }

        self.public_counts.increment(card);
    }

    fn get_private_info(&self, view: &GameStateView) -> Vec<CardPossibilityTable> {
        let mut info = self.get_player_public_info(&self.me).clone();
        for card_table in info.iter_mut() {
            for (other_player, state) in &view.other_player_states {
                for card in &state.hand {
                    card_table.decrement_weight_if_possible(card);
                }
            }
        }
        info
    }

}
impl PlayerStrategy for InformationPlayerStrategy {
    fn decide(&mut self, view: &GameStateView) -> TurnChoice {
        let private_info = self.get_private_info(view);
        // debug!("My info:");
        // for (i, card_table) in private_info.iter().enumerate() {
        //     debug!("{}: {}", i, card_table);
        // }

        let playable_cards = private_info.iter().enumerate().filter(|&(_, card_table)| {
            view.board.probability_is_playable(card_table) == 1.0
        }).collect::<Vec<_>>();

        if playable_cards.len() > 0 {
            // play the best playable card
            // the higher the play_score, the better to play
            let mut play_score = -1.0;
            let mut play_index = 0;

            for (index, card_table) in playable_cards {
                let score = self.get_average_play_score(view, card_table);
                if score > play_score {
                    play_score = score;
                    play_index = index;
                }
            }

            TurnChoice::Play(play_index)
        } else {
            if view.board.hints_remaining > 0 {
                let hint_player = view.board.player_to_left(&self.me);
                let hint_card = rand::thread_rng().choose(&view.get_hand(&hint_player)).unwrap();
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
                    hinted: hinted,
                })
            } else {
                TurnChoice::Discard(0)
            }
        }

        //     // 50 total, 25 to play, 20 in hand
        //     if view.board.discard.cards.len() < 6 {
        //         // if anything is totally useless, discard it
        //         if let Some(i) = self.find_useless_card(view) {
        //             return TurnChoice::Discard(i);
        //         }
        //     }

        //     // hinting is better than discarding dead cards
        //     // (probably because it stalls the deck-drawing).
        //     if view.board.hints_remaining > 1 {
        //         if self.someone_else_can_play(view) {
        //             return self.throwaway_hint(view);
        //         }
        //     }

        //     // if anything is totally useless, discard it
        //     if let Some(i) = self.find_useless_card(view) {
        //         return TurnChoice::Discard(i);
        //     }

        //     // All cards are plausibly useful.
        //     // Play the best discardable card, according to the ordering induced by comparing
        //     //   (is in another hand, is dispensable, value)
        //     // The higher, the better to discard
        //     let mut discard_card = None;
        //     let mut compval = (false, false, 0);
        //     for card in my_cards {
        //         let my_compval = (
        //             view.can_see(card),
        //             view.board.is_dispensable(card),
        //             card.value,
        //         );
        //         if my_compval > compval {
        //             discard_card = Some(card);
        //             compval = my_compval;
        //         }
        //     }
        //     if let Some(card) = discard_card {
        //         if view.board.hints_remaining > 0 {
        //             if !view.can_see(card) {
        //                 return self.throwaway_hint(view);
        //             }
        //         }

        //         let index = my_cards.iter().position(|iter_card| {
        //             card == iter_card
        //         }).unwrap();
        //         TurnChoice::Discard(index)
        //     } else {
        //         panic!("This shouldn't happen!  No discardable card");
        //     }
        // }
    }

    fn update(&mut self, turn: &Turn, view: &GameStateView) {
        match turn.choice {
            TurnChoice::Hint(ref hint) =>  {
                if let &TurnResult::Hint(ref matches) = &turn.result {
                    self.update_public_info_for_hint(hint, matches);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn.choice, turn.result);
                }
            }
            TurnChoice::Discard(index) => {
                if let &TurnResult::Discard(ref card) = &turn.result {
                    self.update_public_info_for_discard_or_play(view, &turn.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn.choice, turn.result);
                }
            }
            TurnChoice::Play(index) =>  {
                if let &TurnResult::Play(ref card, played) = &turn.result {
                    self.update_public_info_for_discard_or_play(view, &turn.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn.choice, turn.result);
                }
            }
        }
    }
}
