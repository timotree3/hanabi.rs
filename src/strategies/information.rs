use std::collections::{HashMap, HashSet};

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

// TODO: currently, you need to be very careful due to
// answers changing from the old view to the new view

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
        self.modulus = self.modulus / modulus;
        let value = self.value / self.modulus;
        assert!((self.value - value) % modulus == 0);
        self.value = (self.value - value) / modulus;

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
    fn answer(&self, &Cards, Box<&GameView>) -> u32;
    fn answer_info(&self, hand: &Cards, view: Box<&GameView>) -> ModulusInformation {
        ModulusInformation::new(
            self.info_amount(),
            self.answer(hand, view)
        )
    }
    // process the answer to this question, updating card info
    fn acknowledge_answer(
        &self, value: u32, &mut Vec<CardPossibilityTable>, Box<&GameView>
    );

    fn acknowledge_answer_info(
        &self,
        answer: ModulusInformation,
        hand_info: &mut Vec<CardPossibilityTable>,
        view: Box<&GameView>
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
    fn answer(&self, hand: &Cards, view: Box<&GameView>) -> u32 {
        let ref card = hand[self.index];
        if view.get_board().is_playable(card) { 1 } else { 0 }
    }
    fn acknowledge_answer(
        &self,
        answer: u32,
        hand_info: &mut Vec<CardPossibilityTable>,
        view: Box<&GameView>,
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
// struct IsDead {
//     index: usize,
// }
// impl Question for IsDead {
//     fn info_amount(&self) -> u32 { 2 }
//     fn answer(&self, hand: &Cards, view: &Box<GameView>) -> u32 {
//         let ref card = hand[self.index];
//         if view.get_board().is_dead(card) { 1 } else { 0 }
//     }
//     fn acknowledge_answer(
//         &self,
//         answer: u32,
//         hand_info: &mut Vec<CardPossibilityTable>,
//         view: &Box<GameView>,
//     ) {
//         let ref mut card_table = hand_info[self.index];
//         let possible = card_table.get_possibilities();
//         for card in &possible {
//             if view.get_board().is_dead(card) {
//                 if answer == 0 { card_table.mark_false(card); }
//             } else {
//                 if answer == 1 { card_table.mark_false(card); }
//             }
//         }
//     }
// }

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

        while info_remaining > 1 {
            let mut question = None;
            for (i, card_table) in hand_info.iter().enumerate() {
                let p = view.get_board().probability_is_playable(card_table);
                if (p != 0.0) && (p != 1.0) {
                    question = Some(Box::new(IsPlayable {index: i}) as Box<Question>);
                    break;
                }
            }
            if let Some(q) = question {
                info_remaining = info_remaining / q.info_amount();
                questions.push(q);
            } else {
                break;
            }
        }
        questions
    }

    fn answer_questions<T>(
        questions: &Vec<Box<Question>>, hand: &Cards, view: &T
    ) -> ModulusInformation
        where T: GameView
    {
        let mut info = ModulusInformation::none();
        for question in questions {
            let answer_info = question.answer_info(hand, Box::new(view as &GameView));
            info.combine(answer_info);
        }
        info
    }

    fn get_hint_info_for_player<T>(
        &self, player: &Player, total_info: u32, view: &T
    ) -> ModulusInformation where T: GameView
    {
        assert!(player != &self.me);
        let hand_info = self.get_player_public_info(player);
        let questions = Self::get_questions(total_info, view, hand_info);
        trace!("Getting hint for player {}, questions {:?}", player, questions.len());
        let mut answer = Self::answer_questions(&questions, view.get_hand(player), view);
        answer.cast_up(total_info);
        trace!("Resulting answer {:?}", answer);
        answer
    }

    fn get_hint_sum<T>(&self, total_info: u32, view: &T) -> ModulusInformation
        where T: GameView
    {
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
                question.acknowledge_answer_info(answer_info, &mut hand_info, Box::new(view as &GameView));
            }
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
                        let answer = question.answer(hand, Box::new(view as &GameView));
                        question.acknowledge_answer(answer, &mut hand_info, Box::new(view as &GameView));
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

    // given a hand of cards, represents how badly it will need to play things
    fn hand_play_value(&self, view: &BorrowedGameView, hand: &Cards/*, all_viewable: HashMap<Color, <Value, usize>> */) -> u32 {
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

    fn estimate_hand_play_value(&self, view: &BorrowedGameView) -> u32 {
        // TODO: fix this
        0
    }

    // how badly do we need to play a particular card
    fn get_average_play_score(&self, view: &BorrowedGameView, card_table: &CardPossibilityTable) -> f32 {
        let f = |card: &Card| {
            self.get_play_score(view, card) as f32
        };
        card_table.weighted_score(&f)
    }

    fn get_play_score(&self, view: &BorrowedGameView, card: &Card) -> i32 {
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

    fn find_useless_card(&self, view: &BorrowedGameView, hand: &Vec<CardPossibilityTable>) -> Option<usize> {
        let mut set: HashSet<Card> = HashSet::new();

        for (i, card_table) in hand.iter().enumerate() {
            if view.board.probability_is_dead(card_table) == 1.0 {
                return Some(i);
            }
            if let Some(card) = card_table.get_card() {
                if set.contains(&card) {
                    // found a duplicate card
                    return Some(i);
                }
                set.insert(card);
            }
        }
        return None
    }

    fn someone_else_can_play(&self, view: &BorrowedGameView) -> bool {
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

    fn get_private_info(&self, view: &BorrowedGameView) -> Vec<CardPossibilityTable> {
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

    fn get_hint(&self, view: &BorrowedGameView) -> TurnChoice {
        let total_info = 3 * (view.board.num_players - 1);

        let hint_info = self.get_hint_sum(total_info, view);

        let hint_type = hint_info.value % 3;
        let player_amt = (hint_info.value - hint_type) / 3;

        let hint_player = (self.me + 1 + player_amt) % view.board.num_players;
        let card_index = 0;

        let hand = view.get_hand(&hint_player);
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

    fn infer_from_hint(&mut self, view: &BorrowedGameView, hint: &Hint, result: &Vec<bool>) {
        let total_info = 3 * (view.board.num_players - 1);

        let hinter = self.last_view.board.player;
        let player_amt = (view.board.num_players + hint.player - hinter - 1) % view.board.num_players;

        let hint_type = if result[0] {
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
    fn decide(&mut self, view: &BorrowedGameView) -> TurnChoice {
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

            return TurnChoice::Play(play_index)
        }

        let discard_threshold =
            view.board.total_cards
            - (COLORS.len() * VALUES.len()) as u32
            - (view.board.num_players * view.board.hand_size);

        if view.board.discard_size() <= discard_threshold {
            // if anything is totally useless, discard it
            if let Some(i) = self.find_useless_card(view, &private_info) {
                return TurnChoice::Discard(i);
            }
        }

        // hinting is better than discarding dead cards
        // (probably because it stalls the deck-drawing).
        if view.board.hints_remaining > 1 {
            if self.someone_else_can_play(view) {
                return self.get_hint(view);
            }
        }

        // if anything is totally useless, discard it
        if let Some(i) = self.find_useless_card(view, &private_info) {
            return TurnChoice::Discard(i);
        }

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

        if view.board.hints_remaining > 0 {
            self.get_hint(view)
        } else {
            TurnChoice::Discard(0)
        }
    }

    fn update(&mut self, turn: &Turn, view: &BorrowedGameView) {
        match turn.choice {
            TurnChoice::Hint(ref hint) =>  {
                if let &TurnResult::Hint(ref matches) = &turn.result {
                    self.infer_from_hint(view, hint, matches);
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
