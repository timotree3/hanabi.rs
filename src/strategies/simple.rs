use std::collections::{HashMap, HashSet};
use std::cmp::Ordering;

use strategy::*;
use game::*;
use helpers::*;

pub struct SimpleStrategyConfig;

impl SimpleStrategyConfig {
    pub fn new() -> SimpleStrategyConfig {
        SimpleStrategyConfig
    }
}
impl GameStrategyConfig for SimpleStrategyConfig {
    fn initialize(&self, _: &GameOptions) -> Box<GameStrategy> {
        Box::new(SimpleStrategy::new())
    }
}

pub struct SimpleStrategy;

impl SimpleStrategy {
    pub fn new() -> SimpleStrategy {
        SimpleStrategy
    }
}
impl GameStrategy for SimpleStrategy {
    fn initialize(&self, player: Player, view: &BorrowedGameView) -> Box<PlayerStrategy> {
        let public_info =
            view.board.get_players().map(|player| {
                let hand_info = HandInfo::new(view.board.hand_size);
                (player, hand_info)
            }).collect::<HashMap<_,_>>();

        Box::new(SimplePlayerStrategy {
            me: player,
            public_info: public_info,
            public_counts: CardCounts::new(),
        })
    }
}

pub struct SimplePlayerStrategy {
    me: Player,
    public_info: HashMap<Player, HandInfo<CardPossibilityTable>>,
    public_counts: CardCounts, // what any newly drawn card should be
}

impl SimplePlayerStrategy {

    // how badly do we need to play a particular card
    fn get_average_play_score(&self, view: &BorrowedGameView, card_table: &CardPossibilityTable) -> f32 {
        let f = |card: &Card| { self.get_play_score(view, card) };
        card_table.weighted_score(&f)
    }

    fn get_play_score(&self, view: &BorrowedGameView, card: &Card) -> f32 {
        let mut num_with = 1;
        if view.board.deck_size > 0 {
            for player in view.board.get_players() {
                if player != self.me {
                    if view.has_card(&player, card) {
                        num_with += 1;
                    }
                }
            }
        }
        (10.0 - card.value as f32) / (num_with as f32)
    }

    fn find_useless_cards(&self, view: &BorrowedGameView, hand: &HandInfo<CardPossibilityTable>) -> Vec<usize> {
        let mut useless: HashSet<usize> = HashSet::new();
        let mut seen: HashMap<Card, usize> = HashMap::new();

        for (i, card_table) in hand.iter().enumerate() {
            if card_table.probability_is_dead(view.get_board()) == 1.0 {
                useless.insert(i);
            } else {
                if let Some(card) = card_table.get_card() {
                    if seen.contains_key(&card) {
                        // found a duplicate card
                        useless.insert(i);
                        useless.insert(*seen.get(&card).unwrap());
                    } else {
                        seen.insert(card, i);
                    }
                }
            }
        }
        let mut useless_vec : Vec<usize> = useless.into_iter().collect();
        useless_vec.sort();
        return useless_vec;
    }

    fn get_player_public_info(&self, player: &Player) -> &HandInfo<CardPossibilityTable> {
        self.public_info.get(player).unwrap()
    }

    fn get_player_public_info_mut(&mut self, player: &Player) -> &mut HandInfo<CardPossibilityTable> {
        self.public_info.get_mut(player).unwrap()
    }

    fn update_public_info_for_hint(&mut self, hint: &Hint, matches: &Vec<bool>) {
        let mut info = self.get_player_public_info_mut(&hint.player);
        info.update_for_hint(&hint.hinted, matches);
    }

    fn update_public_info_for_discard_or_play(
        &mut self,
        view: &BorrowedGameView,
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
            if info.len() < view.hand_size(&player) {
                info.push(new_card_table);
            }
        }

        // note: other_player could be player, as well
        // in particular, we will decrement the newly drawn card
        for other_player in view.board.get_players() {
            let info = self.get_player_public_info_mut(&other_player);
            for card_table in info.iter_mut() {
                card_table.decrement_weight_if_possible(card);
            }
        }

        self.public_counts.increment(card);
    }

    fn get_private_info(&self, view: &BorrowedGameView) -> HandInfo<CardPossibilityTable> {
        let mut info = self.get_player_public_info(&self.me).clone();

        for card_table in info.iter_mut() {
            for (_, hand) in &view.other_hands {
                for card in hand.iter() {
                    card_table.decrement_weight_if_possible(card);
                }
            }
        }
        info
    }

    // how good is it to give this hint to this player?
    fn hint_goodness(&self, hint: &Hint, view: &BorrowedGameView) -> f32 {
        let hand = view.get_hand(&hint.player);

        // get post-hint hand_info
        let mut hand_info = self.get_player_public_info(&hint.player).clone();

        let mut goodness = 1.0;
        for (i, card_table) in hand_info.iter_mut().enumerate() {
            let card = &hand[i];
            if card_table.probability_is_dead(&view.board) == 1.0 {
                continue;
            }
            if card_table.is_determined() {
                continue;
            }
            let old_weight = card_table.total_weight();
            match hint.hinted {
                Hinted::Color(color) => {
                    card_table.mark_color(color, color == card.color)
                }
                Hinted::Value(value) => {
                    card_table.mark_value(value, value == card.value)
                }
            };
            let new_weight = card_table.total_weight();
            assert!(new_weight <= old_weight);
            let mut bonus = {
                if view.board.is_playable(card) {
                    100
                } else if view.board.is_dispensable(card) {
                    10
                } else {
                    1
                }
            };

            if card_table.is_determined() {
                bonus *= 2;
            } else if card_table.probability_is_dead(&view.board) == 1.0 {
                bonus *= 2;
            }

            goodness *= (bonus as f32) * (old_weight / new_weight);
        }
        goodness
    }

    fn get_hint(&self, view: &BorrowedGameView) -> TurnChoice {
        let mut hint_option_set = HashSet::new();
        for hinted_player in view.board.get_players() {
            if hinted_player == self.me {
                continue;
            }

            let hand = view.get_hand(&hinted_player);

            for card in hand {
                hint_option_set.insert(
                    Hint {player: hinted_player, hinted: Hinted::Color(card.color)}
                );
                hint_option_set.insert(
                    Hint {player: hinted_player, hinted: Hinted::Value(card.value)}
                );
            }
        }

        // using hint goodness barely helps
        let mut hint_options = hint_option_set.into_iter().map(|hint| {
            (self.hint_goodness(&hint, view), hint)
        }).collect::<Vec<_>>();

        hint_options.sort_by(|h1, h2| {
            h2.0.partial_cmp(&h1.0).unwrap_or(Ordering::Equal)
        });

        TurnChoice::Hint(hint_options.remove(0).1)
    }
}

impl PlayerStrategy for SimplePlayerStrategy {
    fn decide(&mut self, view: &BorrowedGameView) -> TurnChoice {
        for player in view.board.get_players() {
           let hand_info = self.get_player_public_info(&player);
            debug!("Current state of hand_info for {}:", player);
            for (i, card_table) in hand_info.iter().enumerate() {
                debug!("  Card {}: {}", i, card_table);
            }
        }

        let private_info = self.get_private_info(view);
        // debug!("My info:");
        // for (i, card_table) in private_info.iter().enumerate() {
        //     debug!("{}: {}", i, card_table);
        // }

        let playable_cards = private_info.iter().enumerate().filter(|&(_, card_table)| {
            card_table.probability_is_playable(&view.board) == 1.0
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

            return TurnChoice::Play(play_index)
        }

        let discard_threshold =
            view.board.total_cards
            - (COLORS.len() * VALUES.len()) as u32
            - (view.board.num_players * view.board.hand_size);

        // make a possibly risky play
        if view.board.lives_remaining > 1 &&
           view.board.discard_size() <= discard_threshold
        {
            let mut risky_playable_cards = private_info.iter().enumerate().filter(|&(_, card_table)| {
                // card is either playable or dead
                card_table.probability_of_predicate(&|card| {
                    view.board.is_playable(card) || view.board.is_dead(card)
                }) == 1.0
            }).map(|(i, card_table)| {
                let p = card_table.probability_is_playable(&view.board);
                (i, card_table, p)
            }).collect::<Vec<_>>();

            if risky_playable_cards.len() > 0 {
                risky_playable_cards.sort_by(|c1, c2| {
                    c2.2.partial_cmp(&c1.2).unwrap_or(Ordering::Equal)
                });

                let maybe_play = risky_playable_cards[0];
                if maybe_play.2 > 0.75 {
                    return TurnChoice::Play(maybe_play.0);
                }
            }
        }

        let useless_indices = self.find_useless_cards(view, &private_info);

        if view.board.discard_size() <= discard_threshold {
            if useless_indices.len() > 0 {
                return TurnChoice::Discard(useless_indices[0]);
            }
        }

        // hinting is better than discarding dead cards
        // (probably because it stalls the deck-drawing).
        if view.board.hints_remaining > 0 {
            return self.get_hint(view);
        }

        // if anything is totally useless, discard it
        if useless_indices.len() > 0 {
            return TurnChoice::Discard(useless_indices[0]);
        }

        // Play the best discardable card
        let mut compval = 0.0;
        let mut index = 0;
        for (i, card_table) in private_info.iter().enumerate() {
            let probability_is_seen = card_table.probability_of_predicate(&|card| {
                view.can_see(card)
            });
            let my_compval =
                20.0 * probability_is_seen
                + 10.0 * card_table.probability_is_dispensable(&view.board)
                + card_table.average_value();

            if my_compval > compval {
                compval = my_compval;
                index = i;
            }
        }
        TurnChoice::Discard(index)
    }

    fn update(&mut self, turn_record: &TurnRecord, view: &BorrowedGameView) {
        match turn_record.choice {
            TurnChoice::Hint(ref hint) =>  {
                if let &TurnResult::Hint(ref matches) = &turn_record.result {
                    self.update_public_info_for_hint(hint, matches);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn_record.choice, turn_record.result);
                }
            }
            TurnChoice::Discard(index) => {
                if let &TurnResult::Discard(ref card) = &turn_record.result {
                    self.update_public_info_for_discard_or_play(view, &turn_record.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn_record.choice, turn_record.result);
                }
            }
            TurnChoice::Play(index) =>  {
                if let &TurnResult::Play(ref card, _) = &turn_record.result {
                    self.update_public_info_for_discard_or_play(view, &turn_record.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn_record.choice, turn_record.result);
                }
            }
        }
    }
}
