use std::collections::{HashMap, HashSet};

use rand::distributions::WeightedIndex;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaChaRng;

use crate::game::*;
use crate::strategies::information;
use crate::strategy::*;

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
    fn initialize(&self, _: Player, view: &BorrowedGameView) -> Box<dyn PlayerStrategy> {
        Box::new(RsPlayer {
            g: GlobalUnderstanding::first_turn(&view.board),
            opts: self.opts,
        })
    }
}

#[derive(Debug)]
enum Action {
    Play(CardId),
    Discard(CardId),
    Hint {
        hinted: Hinted,
        rx: Player,
        touched: Vec<CardId>,
    },
}
type CardId = u8;

#[derive(Debug, Clone)]
struct GlobalUnderstanding {
    whose_turn: Player,
    next_card_id: CardId,
    deck_len: CardId,
    hands: Vec<Vec<CardId>>,
    instructed_plays: HashSet<CardId>,
    touched: HashSet<CardId>,
    drawn_cards: Vec<CardLocation>,
    information: Vec<HashMap<Hinted, Information>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Information {
    Positive,
    Negative,
}

#[derive(Debug, Clone)]
enum CardLocation {
    Revealed(Card),
    Held { player: Player, slot: u32 },
}

fn hint_matches(hinted: &Hinted, card: Card) -> bool {
    match hinted {
        Hinted::Color(color) => card.color == *color,
        Hinted::Value(value) => card.value == *value,
    }
}

impl GlobalUnderstanding {
    fn first_turn(board: &BoardState<'_>) -> Self {
        let hands = board
            .get_players()
            .map(|player| {
                (0..board.hand_size)
                    .map(|id| id + (player * board.hand_size))
                    .map(|id| CardId::try_from(id).unwrap())
                    .collect()
            })
            .collect();
        let drawn_cards = board
            .get_players()
            .flat_map(|player| {
                (0..board.hand_size).map(move |slot| CardLocation::Held { player, slot })
            })
            .collect();
        let deck_len = (board.deck_size + (board.hand_size * board.num_players))
            .try_into()
            .unwrap();
        GlobalUnderstanding {
            whose_turn: 0,
            next_card_id: (board.hand_size * board.num_players).try_into().unwrap(),
            deck_len,
            hands,
            instructed_plays: HashSet::new(),
            touched: HashSet::new(),
            drawn_cards,
            information: vec![HashMap::new(); deck_len as usize],
        }
    }

    fn draw(&mut self, replacing: CardId, revealed: Card) {
        self.instructed_plays.remove(&replacing);
        self.touched.remove(&replacing);
        let hand = &mut self.hands[self.whose_turn as usize];
        hand.retain(|&id| id != replacing);
        for (slot, id) in hand.iter().enumerate() {
            self.drawn_cards[*id as usize] = CardLocation::Held {
                player: self.whose_turn,
                slot: slot.try_into().unwrap(),
            }
        }
        if self.next_card_id < self.deck_len {
            hand.push(self.next_card_id);
            self.next_card_id += 1;
            self.drawn_cards.push(CardLocation::Held {
                player: self.whose_turn,
                slot: (hand.len() - 1).try_into().unwrap(),
            })
        }
        self.drawn_cards[replacing as usize] = CardLocation::Revealed(revealed);
    }

    #[tracing::instrument(ret, level = "debug")]
    fn describe(&self, action: Action) -> Description {
        // info!("entering describe");
        match action {
            Action::Play(id) => {
                if self.instructed_plays.contains(&id) {
                    Description::Play { id }
                } else {
                    Description::Misplay { id }
                }
            }
            Action::Discard(id) => Description::Discard { id },
            Action::Hint {
                hinted,
                rx,
                touched,
            } => {
                if let Hinted::Color(_) = hinted {
                    let hand = &self.hands[rx as usize];
                    let newly_touched: Vec<CardId> = touched
                        .iter()
                        .copied()
                        .filter(|id| !self.is_touched(*id))
                        .collect();
                    if newly_touched.is_empty() {
                        Description::Stall {
                            rx,
                            hinted,
                            touched,
                        }
                    } else {
                        let (newest, rest) = hand.split_last().unwrap();
                        let focus_slot = if let Some(idx) =
                            rest.iter().rposition(|id| newly_touched.contains(id))
                        {
                            idx
                        } else {
                            assert!(touched.contains(newest));
                            hand.len() - 1
                        };
                        let mut target_slot = (focus_slot + 1) % hand.len();
                        while self.is_touched(hand[target_slot]) {
                            target_slot = (target_slot + 1) % hand.len();
                        }
                        let target = hand[target_slot];
                        Description::PlayClue {
                            rx,
                            hinted,
                            touched,
                            target,
                        }
                    }
                } else {
                    Description::Stall {
                        rx,
                        hinted,
                        touched,
                    }
                }
            }
        }
    }

    fn update(&mut self, description: Description, result: &TurnResult) {
        match (description, result) {
            (
                Description::Misplay { id }
                | Description::Play { id }
                | Description::Discard { id },
                TurnResult::Discard(card) | TurnResult::Play(card, _),
            ) => self.draw(id, card.clone()),
            (
                Description::Stall {
                    rx,
                    hinted,
                    touched,
                },
                TurnResult::Hint(_),
            ) => {
                self.touched.extend(touched.iter());
                for &id in &touched {
                    self.information[id as usize].insert(hinted, Information::Positive);
                }
                for &id in &self.hands[rx as usize] {
                    self.information[id as usize]
                        .entry(hinted)
                        .or_insert(Information::Negative);
                }
            }
            (
                Description::PlayClue {
                    rx,
                    hinted,
                    touched,
                    target,
                },
                TurnResult::Hint(_),
            ) => {
                self.touched.extend(touched.iter());
                for &id in &touched {
                    self.information[id as usize].insert(hinted, Information::Positive);
                }
                for &id in &self.hands[rx as usize] {
                    self.information[id as usize]
                        .entry(hinted)
                        .or_insert(Information::Negative);
                }
                self.instructed_plays.insert(target);
            }
            x => unreachable!("unexpected combination of description and result: {x:?}"),
        }
        self.whose_turn = (self.whose_turn + 1) % u32::try_from(self.hands.len()).unwrap()
    }

    fn is_touched(&self, id: CardId) -> bool {
        self.touched.contains(&id) || self.instructed_plays.contains(&id)
    }

    fn action_from_my_choice(&self, choice: &TurnChoice, view: &BorrowedGameView) -> Action {
        match choice {
            TurnChoice::Hint(Hint { player, hinted }) => Action::Hint {
                hinted: *hinted,
                rx: *player,
                touched: self.hands[*player as usize]
                    .iter()
                    .enumerate()
                    .filter(|(slot, _)| {
                        if let Some(card) = &view.get_hand(player).get(*slot) {
                            hint_matches(hinted, card)
                        } else {
                            false
                        }
                    })
                    .map(|(_, id)| *id)
                    .collect(),
            },
            TurnChoice::Discard(slot) => {
                Action::Discard(self.hands[self.whose_turn as usize][*slot])
            }
            TurnChoice::Play(slot) => Action::Play(self.hands[self.whose_turn as usize][*slot]),
        }
    }

    fn action_from_record(&self, record: &TurnRecord) -> Action {
        match record {
            TurnRecord {
                choice: TurnChoice::Hint(Hint { player: rx, hinted }),
                result: TurnResult::Hint(touched),
                ..
            } => Action::Hint {
                hinted: *hinted,
                rx: *rx,
                touched: self.hands[*rx as usize]
                    .iter()
                    .zip(touched)
                    .filter(|(_, touched)| **touched)
                    .map(|(id, _)| *id)
                    .collect(),
            },
            TurnRecord {
                choice: TurnChoice::Discard(slot),
                ..
            } => Action::Discard(self.hands[self.whose_turn as usize][*slot]),
            TurnRecord {
                choice: TurnChoice::Play(slot),
                ..
            } => Action::Play(self.hands[self.whose_turn as usize][*slot]),
            TurnRecord {
                choice: TurnChoice::Hint(_),
                ..
            } => unreachable!("TurnChoice variant must match TurnResult variant"),
        }
    }

    fn card<'v>(&self, id: CardId, view: &'v BorrowedGameView) -> &'v Card {
        for (player, hand) in self.hands.iter().enumerate() {
            if let Some(slot) = hand.iter().position(|&id2| id2 == id) {
                return &view.get_hand(&(player as u32))[slot];
            }
        }
        panic!("card not in hands {id}")
    }

    fn is_reasonable(&self, candidate: &Description, view: &BorrowedGameView) -> bool {
        match candidate {
            Description::Misplay { id: _ } => false,
            Description::Stall {
                rx: _,
                hinted: _,
                touched: _,
            } => view.board.hints_remaining == view.board.hints_total,
            Description::Discard { id: _ } => !self.can_play(),
            Description::PlayClue {
                rx: _,
                hinted: _,
                touched: _,
                target,
            } => view.board.is_playable(self.card(*target, view)),
            Description::Play { id: _ } => true,
        }
    }

    fn can_play(&self) -> bool {
        self.hands[self.whose_turn as usize]
            .iter()
            .any(|id| self.instructed_plays.contains(id))
    }
}

pub struct RsPlayer {
    g: GlobalUnderstanding,
    opts: GameOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Description {
    Misplay {
        id: CardId,
    },
    Stall {
        rx: Player,
        hinted: Hinted,
        touched: Vec<CardId>,
    },
    Discard {
        id: CardId,
    },
    PlayClue {
        rx: Player,
        hinted: Hinted,
        touched: Vec<CardId>,
        target: CardId,
    },
    Play {
        id: CardId,
    },
}

#[tracing::instrument(ret, level = "trace")]
fn list_possible_choices(view: &BorrowedGameView) -> Vec<TurnChoice> {
    let plays = (0..view.my_hand_size()).map(TurnChoice::Play);
    let discards = (0..view.my_hand_size()).map(TurnChoice::Discard);
    let clues = (0..view.me())
        .chain((view.me() + 1)..view.board.num_players)
        .flat_map(|receiver| {
            view.get_hand(&receiver)
                .iter()
                .flat_map(|card| {
                    [
                        TurnChoice::Hint(Hint {
                            player: receiver,
                            hinted: Hinted::Color(card.color),
                        }),
                        TurnChoice::Hint(Hint {
                            player: receiver,
                            hinted: Hinted::Value(card.value),
                        }),
                    ]
                })
                .collect::<HashSet<_>>()
        });
    if view.board.hints_remaining < 1 {
        plays.chain(discards).collect()
    } else if view.board.hints_remaining == 8 {
        plays.chain(clues).collect()
    } else {
        plays.chain(discards).chain(clues).collect()
    }
}

impl PlayerStrategy for RsPlayer {
    fn decide(&mut self, view: &BorrowedGameView) -> TurnChoice {
        let mut possible_choices: Vec<TurnChoice> = list_possible_choices(view);
        possible_choices.retain(|choice| {
            let action = self.g.action_from_my_choice(choice, view);
            let description = self.g.describe(action);
            self.g.is_reasonable(&description, view)
        });
        let mut rng = rand::thread_rng();
        let mut seeds: Vec<u64> = Vec::with_capacity(100);
        for _ in 0..1 {
            seeds.push(rng.gen());
        }
        possible_choices
            .iter()
            .map(|choice| {
                (
                    choice,
                    seeds
                        .iter()
                        .map(|seed| rollout(choice.clone(), &self.g, view, &self.opts, *seed))
                        .sum::<u32>(),
                )
            })
            .max_by_key(|(_, sum)| *sum)
            .expect("at least one reasonable option")
            .0
            .clone()
    }

    fn update(&mut self, turn: &TurnRecord, _: &BorrowedGameView) {
        let description = self.g.describe(self.g.action_from_record(turn));
        self.g.update(description, &turn.result);
    }

    fn name(&self) -> String {
        String::from("ref_sieve")
    }
}

/// Simulates the rest of the game after choosing `choice` and returns the score.
///
/// 1. Generates a random consistent deck from `seed` and `view`.
/// 2. Makes random reasonable moves from `g`
fn rollout(
    choice: TurnChoice,
    g: &GlobalUnderstanding,
    view: &BorrowedGameView,
    opts: &GameOptions,
    seed: u64,
) -> u32 {
    let mut rng = ChaChaRng::seed_from_u64(seed);
    let deck = consistent_deck(&mut rng, g, view, opts);
    let mut g = g.clone();

    let mut game = GameState::new(opts, deck);
    for turn in &view.board.turn_history {
        game.process_choice(turn.choice.clone());
    }

    for player in view.get_other_players() {
        assert_eq!(game.unannotated_hands..of(player), view.get_hand(&player))
    }

    // Make the initial move of the line
    let turn = game.process_choice(choice);
    let description = g.describe(g.action_from_record(&turn));
    g.update(description, &turn.result);

    while !game.is_over() {
        let player = game.board.player;
        let choice = {
            let view = &game.get_view(player);
            let mut possible_choices: Vec<TurnChoice> = list_possible_choices(view);
            possible_choices.retain(|choice| {
                // dbg!(
                //     g.next_card_id,
                //     game.board.deck_size,
                //     &game.unannotated_hands,
                //     player,
                //     view.my_hand_size(),
                // );
                let action = g.action_from_my_choice(choice, view);
                let description = g.describe(action);
                g.is_reasonable(&description, view)
            });
            possible_choices
                .choose(&mut rng)
                .expect("at least one reasonable option")
                .clone()
        };

        let turn = game.process_choice(choice);

        let description = g.describe(g.action_from_record(&turn));
        g.update(description, &turn.result);
    }

    game.score()
}

fn consistent_deck(
    rng: &mut ChaChaRng,
    g: &GlobalUnderstanding,
    view: &BorrowedGameView,
    opts: &GameOptions,
) -> Vec<Card> {
    fn card_to_index(card: Card) -> usize {
        let color_idx = COLORS.iter().position(|c| c == &card.color).unwrap();
        let value_idx = card.value - 1;
        (color_idx * VALUES.len()) + usize::try_from(value_idx).unwrap()
    }
    fn index_to_card(index: usize) -> Card {
        let color_idx = index / VALUES.len();
        let value_idx = index % VALUES.len();
        Card::new(COLORS[color_idx], u32::try_from(value_idx).unwrap() + 1)
    }
    let mut card_counts: Vec<u32> = COLORS
        .iter()
        .flat_map(|_color| VALUES.iter().map(|value| get_count_for_value(*value)))
        .collect();

    let mut visible_cards: Vec<(CardId, Card)> = Vec::new();
    let mut my_hand: Vec<CardId> = Vec::new();

    for (id, location) in g.drawn_cards.iter().enumerate() {
        match location {
            CardLocation::Revealed(card) => visible_cards.push((id as CardId, card.clone())),
            CardLocation::Held { player, slot } => {
                if let Some(hand) = view.other_hands.get(player) {
                    let card = hand[*slot as usize].clone();
                    card_counts[card_to_index(&card)] -= 1;
                    visible_cards.push((id as CardId, card));
                } else {
                    my_hand.push(id as CardId);
                }
            }
        }
    }

    let my_cards = 'rejection_sample_hand: loop {
        let mut my_cards: Vec<Card> = Vec::new();
        let mut card_distribution = WeightedIndex::new(card_counts.iter().copied()).unwrap();
        for &id in &my_hand {
            let card_idx = rng.sample(&card_distribution);
            let card = index_to_card(card_idx);
            let information = &g.information[id as usize];
            let consistent = information.iter().all(|(hinted, result)| {
                if hint_matches(hinted, &card) {
                    *result == Information::Positive
                } else {
                    *result == Information::Negative
                }
            });
            if !consistent {
                continue 'rejection_sample_hand;
            }
            my_cards.push(card.clone());
            let copies_of_card_in_my_cards = my_cards.iter().filter(|c| **c == card).count();
            card_distribution
                .update_weights(&[(
                    card_idx,
                    &(card_counts[card_idx] - u32::try_from(copies_of_card_in_my_cards).unwrap()),
                )])
                .unwrap();
        }
        break my_cards;
    };

    visible_cards.extend(my_hand.into_iter().zip(my_cards));

    let mut deck: Vec<Card> = Vec::new();

    for &color in COLORS.iter() {
        for &value in VALUES.iter() {
            for _ in 0..get_count_for_value(value) {
                deck.push(Card::new(color, value));
            }
        }
    }

    for (_, card) in &visible_cards {
        let first_occurrence = deck.iter().position(|c| c == card).unwrap();
        deck.swap_remove(first_occurrence);
    }

    deck.shuffle(rng);

    for (id, card) in &visible_cards {
        deck.push(card.clone());
        let last = deck.len() - 1;
        deck.swap(*id as usize, last);
    }

    for (id, card) in &visible_cards {
        debug_assert_eq!(&deck[*id as usize], card)
    }

    // Deck is stored by the game in reverse order
    deck.reverse();
    deck
    // TODO: Check for context
}

#[test]
fn test_consistent_deck() {
    use crate::simulator::new_deck;
    use rand::RngCore;

    let mut rng = ChaChaRng::from_entropy();
    let original_deck = new_deck(rng.next_u64());
    let opts = GameOptions {
        num_players: 2,
        hand_size: 5,
        num_hints: 8,
        num_lives: 3,
        allow_empty_hints: false,
    };
    let mut game = GameState::new(&opts, original_deck.clone());
    let mut g = GlobalUnderstanding::first_turn(&game.board);
    dbg!(&original_deck);
    {
        let view = game.get_view(1);
        let recreated_deck = consistent_deck(&mut rng, &g, &view, &opts);
        dbg!(&recreated_deck);
        assert_eq!(original_deck.len(), recreated_deck.len());
        assert_eq!(recreated_deck[recreated_deck.len()-5..], original_deck[original_deck.len()-5..], "player 0's cards should be consistent (recreated {recreated_deck:?}) (original {original_deck:?})")
    }
    {
        let view = game.get_view(0);
        let recreated_deck = consistent_deck(&mut rng, &g, &view, &opts);
        dbg!(&recreated_deck);
        assert_eq!(original_deck.len(), recreated_deck.len());
        assert_eq!(recreated_deck[recreated_deck.len()-10..recreated_deck.len()-5], original_deck[original_deck.len()-10..original_deck.len()-5], "player 1's cards should be consistent (recreated {recreated_deck:?}) (original {original_deck:?})")
    }
    let turn = game.process_choice(TurnChoice::Play(0));
    let description = g.describe(g.action_from_record(&turn));
    g.update(description, &turn.result);
    {
        let view = game.get_view(1);
        let recreated_deck = consistent_deck(&mut rng, &g, &view, &opts);
        dbg!(&recreated_deck);
        assert_eq!(original_deck.len(), recreated_deck.len());
        assert_eq!(recreated_deck[recreated_deck.len()-5..], original_deck[original_deck.len()-5..], "player 0's starting hand should be consistent after drawing (recreated {recreated_deck:?}) (original {original_deck:?})")
    }
    {
        let view = game.get_view(0);
        let recreated_deck = consistent_deck(&mut rng, &g, &view, &opts);
        dbg!(&recreated_deck);
        assert_eq!(original_deck.len(), recreated_deck.len());
        assert_eq!(recreated_deck[recreated_deck.len()-10..recreated_deck.len()-5], original_deck[original_deck.len()-10..original_deck.len()-5], "player 1's starting hand should be consistent after drawing (recreated {recreated_deck:?}) (original {original_deck:?})")
    }
}
