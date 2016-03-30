use std::collections::{HashMap, HashSet};
use std::cmp::Ordering;

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
// 0. a value hint on card i
// 1. a color hint on card i
// 2. any hint not involving card i
//
// for 4 players, can give 6 distinct hints

#[derive(Debug,Clone)]
struct ModulusInformation {
    modulus: u32,
    value: u32,
}
impl ModulusInformation {
    pub fn new(modulus: u32, value: u32) -> Self {
        assert!(value < modulus);
        ModulusInformation {
            modulus: modulus,
            value: value,
        }
    }

    pub fn none() -> Self {
        Self::new(1, 0)
    }

    pub fn combine(&mut self, other: Self) {
        self.value = self.value * other.modulus + other.value;
        self.modulus = self.modulus * other.modulus;
    }

    pub fn emit(&mut self, modulus: u32) -> Self {
        assert!(self.modulus >= modulus);
        assert!(self.modulus % modulus == 0);
        let original_modulus = self.modulus;
        let original_value = self.value;
        self.modulus = self.modulus / modulus;
        let value = self.value / self.modulus;
        self.value = self.value - value * self.modulus;
        trace!("orig value {}, orig modulus {}, self.value {}, self.modulus {}, value {}, modulus {}",
               original_value, original_modulus, self.value, self.modulus, value, modulus);
        assert!(original_value == value * self.modulus + self.value);
        Self::new(modulus, value)
    }

    pub fn cast_up(&mut self, modulus: u32) {
        assert!(self.modulus <= modulus);
        self.modulus = modulus;
    }

    pub fn cast_down(&mut self, modulus: u32) {
        assert!(self.modulus >= modulus);
        assert!(self.value < modulus);
        self.modulus = modulus;
    }

    pub fn add(&mut self, other: &Self) {
        assert!(self.modulus == other.modulus);
        self.value = (self.value + other.value) % self.modulus;
    }

    pub fn subtract(&mut self, other: &Self) {
        assert!(self.modulus == other.modulus);
        self.value = (self.modulus + self.value - other.value) % self.modulus;
    }
}

trait Question {
    // how much info does this question ask for?
    fn info_amount(&self) -> u32;
    // get the answer to this question, given cards
    fn answer(&self, &Cards, &OwnedGameView) -> u32;
    fn answer_info(&self, hand: &Cards, view: &OwnedGameView) -> ModulusInformation {
        ModulusInformation::new(
            self.info_amount(),
            self.answer(hand, view)
        )
    }
    // process the answer to this question, updating card info
    fn acknowledge_answer(
        &self, value: u32, &mut Vec<CardPossibilityTable>, &OwnedGameView
    );

    fn acknowledge_answer_info(
        &self,
        answer: ModulusInformation,
        hand_info: &mut Vec<CardPossibilityTable>,
        view: &OwnedGameView
    ) {
        assert!(self.info_amount() == answer.modulus);
        self.acknowledge_answer(answer.value, hand_info, view);
    }
}
struct IsPlayable {
    index: usize,
}
impl Question for IsPlayable {
    fn info_amount(&self) -> u32 { 2 }
    fn answer(&self, hand: &Cards, view: &OwnedGameView) -> u32 {
        let ref card = hand[self.index];
        if view.get_board().is_playable(card) { 1 } else { 0 }
    }
    fn acknowledge_answer(
        &self,
        answer: u32,
        hand_info: &mut Vec<CardPossibilityTable>,
        view: &OwnedGameView,
    ) {
        let ref mut card_table = hand_info[self.index];
        let possible = card_table.get_possibilities();
        for card in &possible {
            if view.get_board().is_playable(card) {
                if answer == 0 { card_table.mark_false(card); }
            } else {
                if answer == 1 { card_table.mark_false(card); }
            }
        }
    }
}

struct IsDead {
    index: usize,
}
impl Question for IsDead {
    fn info_amount(&self) -> u32 { 2 }
    fn answer(&self, hand: &Cards, view: &OwnedGameView) -> u32 {
        let ref card = hand[self.index];
        if view.get_board().is_dead(card) { 1 } else { 0 }
    }
    fn acknowledge_answer(
        &self,
        answer: u32,
        hand_info: &mut Vec<CardPossibilityTable>,
        view: &OwnedGameView,
    ) {
        let ref mut card_table = hand_info[self.index];
        let possible = card_table.get_possibilities();
        for card in &possible {
            if view.get_board().is_dead(card) {
                if answer == 0 { card_table.mark_false(card); }
            } else {
                if answer == 1 { card_table.mark_false(card); }
            }
        }
    }
}

struct CardPossibilityPartition {
    index: usize,
    n_partitions: u32,
    partition: HashMap<Card, u32>,
}
impl CardPossibilityPartition {
    fn new<T>(
        index: usize, max_n_partitions: u32, card_table: &CardPossibilityTable, view: &T
    ) -> CardPossibilityPartition where T: GameView {
        let mut cur_block = 0;
        let mut partition = HashMap::new();
        let mut n_partitions = 0;

        let has_dead = card_table.probability_is_dead(view.get_board()) != 0.0;

        let effective_max = if has_dead {
            max_n_partitions - 1
        } else {
            max_n_partitions
        };

        for card in card_table.get_possibilities() {
            if !view.get_board().is_dead(&card) {
                partition.insert(card.clone(), cur_block);
                cur_block = (cur_block + 1) % effective_max;
                if n_partitions < effective_max {
                    n_partitions += 1;
                }
            }
        }

        if has_dead {
            for card in card_table.get_possibilities() {
                if view.get_board().is_dead(&card) {
                    partition.insert(card.clone(), n_partitions);
                }
            }
            n_partitions += 1;
        }

        CardPossibilityPartition {
            index: index,
            n_partitions: n_partitions,
            partition: partition,
        }
    }
}
impl Question for CardPossibilityPartition {
    fn info_amount(&self) -> u32 { self.n_partitions }
    fn answer(&self, hand: &Cards, _: &OwnedGameView) -> u32 {
        let ref card = hand[self.index];
        *self.partition.get(&card).unwrap()
    }
    fn acknowledge_answer(
        &self,
        answer: u32,
        hand_info: &mut Vec<CardPossibilityTable>,
        _: &OwnedGameView,
    ) {
        let ref mut card_table = hand_info[self.index];
        let possible = card_table.get_possibilities();
        for card in &possible {
            if *self.partition.get(card).unwrap() != answer {
                card_table.mark_false(card);
            }
        }
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
    fn initialize(&self, _: &GameOptions) -> Box<GameStrategy> {
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
    fn initialize(&self, player: Player, view: &BorrowedGameView) -> Box<PlayerStrategy> {
        let mut public_info = HashMap::new();
        for player in view.board.get_players() {
            let hand_info = (0..view.board.hand_size).map(|_| { CardPossibilityTable::new() }).collect::<Vec<_>>();
            public_info.insert(player, hand_info);
        }
        Box::new(InformationPlayerStrategy {
            me: player,
            public_info: public_info,
            public_counts: CardCounts::new(),
            last_view: OwnedGameView::clone_from(view),
        })
    }
}

pub struct InformationPlayerStrategy {
    me: Player,
    public_info: HashMap<Player, Vec<CardPossibilityTable>>,
    public_counts: CardCounts, // what any newly drawn card should be
    last_view: OwnedGameView, // the view on the previous turn
}
impl InformationPlayerStrategy {

    fn get_questions<T>(
        total_info: u32,
        view: &T,
        hand_info: &Vec<CardPossibilityTable>,
    ) -> Vec<Box<Question>>
        where T: GameView
    {
        let mut questions = Vec::new();
        let mut info_remaining = total_info;

        fn add_question<T: 'static>(
            questions: &mut Vec<Box<Question>>, info_remaining: &mut u32, question: T
        ) -> bool where T: Question {
            *info_remaining = *info_remaining / question.info_amount();
            questions.push(Box::new(question) as Box<Question>);

            // if there's no more info to give, return that we should stop
            *info_remaining <= 1
        }

        let mut augmented_hand_info = hand_info.iter().enumerate().map(|(i, card_table)| {
            let p = card_table.probability_is_playable(view.get_board());
            (p, card_table, i)
        }).collect::<Vec<_>>();

        // sort by probability of play, then by index
        augmented_hand_info.sort_by(|&(p1, _, i1), &(p2, _, i2)| {
            let result = p1.partial_cmp(&p2);
            if result == None || result == Some(Ordering::Equal) {
                i1.cmp(&i2)
            } else {
                result.unwrap()
            }
        });

        // let known_playable = augmented_hand_info[0].0 == 1.0;
        // // if there is a card that is definitely playable, don't ask about playability
        // if !known_playable {
        for &(p, _, i) in &augmented_hand_info {
            if (p != 0.0) && (p != 1.0) {
                if add_question(&mut questions, &mut info_remaining, IsPlayable {index: i}) {
                    return questions;
                }
            }
        }
        // }

        for &(_, card_table, i) in &augmented_hand_info {
            if card_table.is_determined() {
                continue;
            }
            if card_table.probability_is_dead(view.get_board()) == 1.0 {
                continue;
            }
            let question = CardPossibilityPartition::new(i, info_remaining, card_table, view);
            if add_question(&mut questions, &mut info_remaining, question) {
                return questions;
            }
        }

        return questions
    }

    fn answer_questions(
        questions: &Vec<Box<Question>>, hand: &Cards, view: &OwnedGameView
    ) -> ModulusInformation {
        let mut info = ModulusInformation::none();
        for question in questions {
            let answer_info = question.answer_info(hand, view);
            info.combine(answer_info);
        }
        info
    }

    fn get_hint_info_for_player(
        &self, player: &Player, total_info: u32, view: &OwnedGameView
    ) -> ModulusInformation {
        assert!(player != &self.me);
        let hand_info = self.get_player_public_info(player);
        let questions = Self::get_questions(total_info, view, hand_info);
        trace!("Getting hint for player {}, questions {:?}", player, questions.len());
        let mut answer = Self::answer_questions(&questions, view.get_hand(player), view);
        answer.cast_up(total_info);
        trace!("Resulting answer {:?}", answer);
        answer
    }

    fn get_hint_sum_info(&self, total_info: u32, view: &OwnedGameView) -> ModulusInformation {
        let mut sum = ModulusInformation::new(total_info, 0);
        for player in view.get_board().get_players() {
            if player != self.me {
                let answer = self.get_hint_info_for_player(&player, total_info, view);
                sum.add(&answer);
            }
        }
        trace!("Summed answer {:?}\n", sum);
        sum
    }

    fn infer_own_from_hint_sum(&mut self, hint: ModulusInformation) {
        let mut hint = hint;
        let questions = {
            let view = &self.last_view;
            let hand_info = self.get_my_public_info();
            Self::get_questions(hint.modulus, view, hand_info)
        };
        trace!("{}: Questions {:?}", self.me, questions.len());
        let product = questions.iter().fold(1, |a, ref b| a * b.info_amount());
        trace!("{}: Product {}, hint: {:?}", self.me, product, hint);
        hint.cast_down(product);
        trace!("{}: Inferred for myself {:?}", self.me, hint);

        let me = self.me.clone();
        let mut hand_info = self.take_public_info(&me);

        {
            let view = &self.last_view;
            for question in questions {
                let answer_info = hint.emit(question.info_amount());
                question.acknowledge_answer_info(answer_info, &mut hand_info, view);
            }
        }
        debug!("Current state of hand_info for {}:", me);
        for (i, card_table) in hand_info.iter().enumerate() {
            debug!("  Card {}: {}", i, card_table);
        }
        self.return_public_info(&me, hand_info);
    }

    fn update_from_hint_sum(&mut self, hint: ModulusInformation) {
        let hinter = self.last_view.board.player;
        let players = {
            let view = &self.last_view;
            view.board.get_players()
        };
        trace!("{}: inferring for myself, starting with {:?}", self.me, hint);
        let mut hint = hint;
        for player in players {
            if (player != hinter) && (player != self.me) {
                {
                    let view = &self.last_view;
                    let hint_info = self.get_hint_info_for_player(&player, hint.modulus, view);
                    hint.subtract(&hint_info);
                    trace!("{}: subtracted for {}, now {:?}", self.me, player, hint);
                }

                // *take* instead of borrowing mutably, because of borrow rules...
                let mut hand_info = self.take_public_info(&player);

                {
                    let view = &self.last_view;
                    let hand = view.get_hand(&player);
                    let questions = Self::get_questions(hint.modulus, view, &mut hand_info);
                    for question in questions {
                        let answer = question.answer(hand, view);
                        question.acknowledge_answer(answer, &mut hand_info, view);
                    }
                }
                self.return_public_info(&player, hand_info);
            }
        }
        if self.me == hinter {
            assert!(hint.value == 0);
        } else {
            self.infer_own_from_hint_sum(hint);
        }
    }

    // how badly do we need to play a particular card
    fn get_average_play_score(&self, view: &OwnedGameView, card_table: &CardPossibilityTable) -> f32 {
        let f = |card: &Card| {
            self.get_play_score(view, card) as f32
        };
        card_table.weighted_score(&f)
    }

    fn get_play_score(&self, view: &OwnedGameView, card: &Card) -> i32 {
        if view.board.deck_size() > 0 {
            for player in view.board.get_players() {
                if player != self.me {
                    if view.has_card(&player, card) {
                        return 1;
                    }
                }
            }
        }
        5 + (5 - (card.value as i32))
    }

    fn find_useless_cards<T>(&self, view: &T, hand: &Vec<CardPossibilityTable>) -> Vec<usize>
        where T: GameView
    {
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

    fn someone_else_can_play(&self, view: &OwnedGameView) -> bool {
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

    fn take_public_info(&mut self, player: &Player) -> Vec<CardPossibilityTable> {
        self.public_info.remove(player).unwrap()
    }

    fn return_public_info(&mut self, player: &Player, card_info: Vec<CardPossibilityTable>) {
        self.public_info.insert(*player, card_info);
    }

    fn get_my_public_info(&self) -> &Vec<CardPossibilityTable> {
        self.get_player_public_info(&self.me)
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
            for card_table in info {
                card_table.decrement_weight_if_possible(card);
            }
        }

        self.public_counts.increment(card);
    }

    fn get_private_info(&self, view: &OwnedGameView) -> Vec<CardPossibilityTable> {
        let mut info = self.get_my_public_info().clone();
        for card_table in info.iter_mut() {
            for (_, state) in &view.other_player_states {
                for card in &state.hand {
                    card_table.decrement_weight_if_possible(card);
                }
            }
        }
        info
    }

    fn get_hint_index_score<T>(&self, card_table: &CardPossibilityTable, view: &T) -> i32
        where T: GameView
    {
        if card_table.probability_is_dead(view.get_board()) == 1.0 {
            return 0;
        }
        let mut score = -1;
        if !card_table.color_determined() {
            score -= 1;
        }
        if !card_table.value_determined() {
            score -= 1;
        }
        return score;
    }

    fn get_index_for_hint<T>(&self, info: &Vec<CardPossibilityTable>, view: &T) -> usize
        where T: GameView
    {
        let mut scores = info.iter().enumerate().map(|(i, card_table)| {
            let score = self.get_hint_index_score(card_table, view);
            (score, i)
        }).collect::<Vec<_>>();
        scores.sort();
        scores[0].1
    }

    fn get_hint(&self) -> TurnChoice {
        let view = &self.last_view;
        let total_info = 3 * (view.board.num_players - 1);

        let hint_info = self.get_hint_sum_info(total_info, view);

        let hint_type = hint_info.value % 3;
        let player_amt = (hint_info.value - hint_type) / 3;

        let hint_player = (self.me + 1 + player_amt) % view.board.num_players;

        let hand = view.get_hand(&hint_player);
        let card_index = self.get_index_for_hint(self.get_player_public_info(&hint_player), view);
        let hint_card = &hand[card_index];

        let hinted = match hint_type {
            0 => {
                Hinted::Value(hint_card.value)
            }
            1 => {
                Hinted::Color(hint_card.color)
            }
            2 => {
                let mut hinted_opt = None;
                for card in hand {
                    if card.color != hint_card.color {
                        hinted_opt = Some(Hinted::Color(card.color));
                        break;
                    }
                    if card.value != hint_card.value {
                        hinted_opt = Some(Hinted::Value(card.value));
                        break;
                    }
                }
                if let Some(hinted) = hinted_opt {
                    hinted
                } else {
                    // Technically possible, but never happens
                    panic!("Found nothing to hint!")
                }
            }
            _ => {
                panic!("Invalid hint type")
            }
        };

        TurnChoice::Hint(Hint {
            player: hint_player,
            hinted: hinted,
        })
    }

    fn infer_from_hint(&mut self, hint: &Hint, result: &Vec<bool>) {
        let n = self.last_view.board.num_players;
        let total_info = 3 * (n - 1);

        let hinter = self.last_view.board.player;
        let player_amt = (n + hint.player - hinter - 1) % n;

        let card_index = self.get_index_for_hint(self.get_player_public_info(&hint.player), &self.last_view);
        let hint_type = if result[card_index] {
            match hint.hinted {
                Hinted::Value(_) => 0,
                Hinted::Color(_) => 1,
            }
        } else {
            2
        };

        let hint_value = player_amt * 3 + hint_type;

        let mod_info = ModulusInformation::new(total_info, hint_value);

        self.update_from_hint_sum(mod_info);
    }

}
impl PlayerStrategy for InformationPlayerStrategy {
    fn decide(&mut self, _: &BorrowedGameView) -> TurnChoice {
        // we already stored the view
        let view = &self.last_view;

        let private_info = self.get_private_info(view);
        // debug!("My info:");
        // for (i, card_table) in private_info.iter().enumerate() {
        //     debug!("{}: {}", i, card_table);
        // }

        let playable_cards = private_info.iter().enumerate().filter(|&(_, card_table)| {
            card_table.probability_is_playable(&view.board) == 1.0
        }).collect::<Vec<_>>();

        if playable_cards.len() > 0 {
            // TODO: try playing things that have no chance of being indispensable
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

        // make a possibly risky play
        if view.board.lives_remaining > 1 {
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
                    c1.2.partial_cmp(&c2.2).unwrap_or(Ordering::Equal)
                });

                let maybe_play = risky_playable_cards[0];
                if maybe_play.2 > 0.7 {
                    return TurnChoice::Play(maybe_play.0);
                }
            }
        }

        let discard_threshold =
            view.board.total_cards
            - (COLORS.len() * VALUES.len()) as u32
            - (view.board.num_players * view.board.hand_size);

        let public_useless_indices = self.find_useless_cards(view, &self.get_my_public_info());
        let useless_indices = self.find_useless_cards(view, &private_info);

        if view.board.discard_size() <= discard_threshold {
            // if anything is totally useless, discard it
            if public_useless_indices.len() > 1 {
                let info = self.get_hint_sum_info(public_useless_indices.len() as u32, view);
                return TurnChoice::Discard(public_useless_indices[info.value as usize]);
            } else if useless_indices.len() > 0 {
                return TurnChoice::Discard(useless_indices[0]);
            }
        }

        // hinting is better than discarding dead cards
        // (probably because it stalls the deck-drawing).
        if view.board.hints_remaining > 0 {
            if self.someone_else_can_play(view) {
                return self.get_hint();
            } else {
                // print!("This actually happens");
            }
        }

        // TODO: if they discarded a non-useless card, despite there being hints remaining
        // infer that we have no playable cards

        // if anything is totally useless, discard it
        if public_useless_indices.len() > 1 {
            let info = self.get_hint_sum_info(public_useless_indices.len() as u32, view);
            return TurnChoice::Discard(public_useless_indices[info.value as usize]);
        } else if useless_indices.len() > 0 {
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

    fn update(&mut self, turn: &Turn, view: &BorrowedGameView) {
        match turn.choice {
            TurnChoice::Hint(ref hint) =>  {
                if let &TurnResult::Hint(ref matches) = &turn.result {
                    self.infer_from_hint(hint, matches);
                    self.update_public_info_for_hint(hint, matches);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn.choice, turn.result);
                }
            }
            TurnChoice::Discard(index) => {
                if let &TurnResult::Discard(ref card) = &turn.result {
                    let public_useless_indices = self.find_useless_cards(
                        &self.last_view, &self.get_player_public_info(&turn.player));
                    if public_useless_indices.len() > 1 {
                        // unwrap is safe because *if* a discard happened, and there were known
                        // dead cards, it must be a dead card
                        let value = public_useless_indices.iter().position(|&i| i == index).unwrap();
                        self.update_from_hint_sum(ModulusInformation::new(
                            public_useless_indices.len() as u32, value as u32
                        ));
                    }
                    self.update_public_info_for_discard_or_play(view, &turn.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn.choice, turn.result);
                }
            }
            TurnChoice::Play(index) =>  {
                if let &TurnResult::Play(ref card, _) = &turn.result {
                    self.update_public_info_for_discard_or_play(view, &turn.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn.choice, turn.result);
                }
            }
        }
        self.last_view = OwnedGameView::clone_from(view);
    }
}
