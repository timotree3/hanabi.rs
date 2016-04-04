use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::{Index,IndexMut};
use std::convert::From;
use std::slice;

use game::*;

// Can represent information of the form:
// this card is/isn't possible
// also, maintains integer weights for the cards
#[derive(Clone,Debug)]
pub struct CardPossibilityTable {
    possible: HashMap<Card, u32>,
}
impl CardPossibilityTable {
    pub fn new() -> CardPossibilityTable {
        Self::from(&CardCounts::new())
    }

    // whether the card is possible
    pub fn is_possible(&self, card: &Card) -> bool {
        self.possible.contains_key(card)
    }

    pub fn get_possibilities(&self) -> Vec<Card> {
        let mut cards = self.possible.keys().map(|card| {card.clone() }).collect::<Vec<_>>();
        cards.sort();
        cards
    }

    // mark a possible card as false
    pub fn mark_false(&mut self, card: &Card) {
        self.possible.remove(card);
    }

    // a bit more efficient
    // pub fn borrow_possibilities<'a>(&'a self) -> Vec<&'a Card> {
    //     self.possible.keys().collect::<Vec<_>>()
    // }

    pub fn decrement_weight_if_possible(&mut self, card: &Card) {
        if self.is_possible(card) {
            self.decrement_weight(card);
        }
    }

    pub fn decrement_weight(&mut self, card: &Card) {
        let remove = {
            let weight =
                self.possible.get_mut(card)
                    .expect(&format!("Decrementing weight for impossible card: {}", card));
            *weight -= 1;
            *weight == 0
        };
        if remove {
            self.possible.remove(card);
        }
    }

    pub fn get_card(&self) -> Option<Card> {
        let possibilities = self.get_possibilities();
        if possibilities.len() == 1 {
            Some(possibilities[0].clone())
        } else {
            None
        }
    }

    pub fn is_determined(&self) -> bool {
        self.get_possibilities().len() == 1
    }

    pub fn color_determined(&self) -> bool {
        self.get_possibilities()
            .iter().map(|card| card.color)
            .collect::<HashSet<_>>()
            .len() == 1
    }

    pub fn value_determined(&self) -> bool {
        self.get_possibilities()
            .iter().map(|card| card.value)
            .collect::<HashSet<_>>()
            .len() == 1
    }

    // get probability weight for the card
    fn get_weight(&self, card: &Card) -> f32 {
        *self.possible.get(card).unwrap_or(&0) as f32
    }

    // fn get_weighted_possibilities(&self) -> Vec<(Card, f32)> {
    //     self.get_possibilities().into_iter()
    //         .map(|card| {
    //             let weight = self.get_weight(&card);
    //             (card, weight)
    //         }).collect::<Vec<_>>()
    // }

    pub fn total_weight(&self) -> f32 {
        self.get_possibilities().iter()
            .map(|card| self.get_weight(&card))
            .fold(0.0, |a, b| a+b)
    }

    pub fn weighted_score<T>(&self, score_fn: &Fn(&Card) -> T) -> f32
        where f32: From<T>
    {
        let mut total_score = 0.;
        let mut total_weight = 0.;
        for card in self.get_possibilities() {
            let weight = self.get_weight(&card);
            let score = f32::from(score_fn(&card));
            total_weight += weight;
            total_score += weight * score;
        }
        total_score / total_weight
    }

    pub fn average_value(&self) -> f32 {
        self.weighted_score(&|card| card.value as f32 )
    }

    pub fn probability_of_predicate(&self, predicate: &Fn(&Card) -> bool) -> f32 {
        let f = |card: &Card| {
            if predicate(card) { 1.0 } else { 0.0 }
        };
        self.weighted_score(&f)
    }

    pub fn probability_is_playable(&self, board: &BoardState) -> f32 {
        self.probability_of_predicate(&|card| board.is_playable(card))
    }

    pub fn probability_is_dead(&self, board: &BoardState) -> f32 {
        self.probability_of_predicate(&|card| board.is_dead(card))
    }

    pub fn probability_is_dispensable(&self, board: &BoardState) -> f32 {
        self.probability_of_predicate(&|card| board.is_dispensable(card))
    }

    // mark a whole color as false
    fn mark_color_false(&mut self, color: Color) {
        for &value in VALUES.iter() {
            self.mark_false(&Card::new(color, value));
        }

    }
    // mark a color as correct
    fn mark_color_true(&mut self, color: Color) {
        for &other_color in COLORS.iter() {
            if other_color != color {
                self.mark_color_false(other_color);
            }
        }
    }
    pub fn mark_color(&mut self, color: Color, is_color: bool) {
        if is_color {
            self.mark_color_true(color);
        } else {
            self.mark_color_false(color);
        }
    }

    // mark a whole value as false
    fn mark_value_false(&mut self, value: Value) {
        for &color in COLORS.iter() {
            self.mark_false(&Card::new(color, value));
        }
    }
    // mark a value as correct
    fn mark_value_true(&mut self, value: Value) {
        for &other_value in VALUES.iter() {
            if other_value != value {
                self.mark_value_false(other_value);
            }
        }
    }
    pub fn mark_value(&mut self, value: Value, is_value: bool) {
        if is_value {
            self.mark_value_true(value);
        } else {
            self.mark_value_false(value);
        }
    }
}
impl <'a> From<&'a CardCounts> for CardPossibilityTable {
    fn from(counts: &'a CardCounts) -> CardPossibilityTable {
        let mut possible = HashMap::new();
        for &color in COLORS.iter() {
            for &value in VALUES.iter() {
                let card = Card::new(color, value);
                let count = counts.remaining(&card);
                if count > 0 {
                    possible.insert(card, count);
                }
            }
        }
        CardPossibilityTable {
            possible: possible,
        }
    }
}
impl fmt::Display for CardPossibilityTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (card, weight) in &self.possible {
            try!(f.write_str(&format!("{} {}, ", weight, card)));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct HandInfo {
    pub hand_info: Vec<CardPossibilityTable>
}
impl HandInfo {
    pub fn new(hand_size: u32) -> Self {
        let hand_info = (0..hand_size).map(|_| CardPossibilityTable::new()).collect::<Vec<_>>();
        HandInfo {
            hand_info: hand_info,
        }
    }

    // update for hint to me
    pub fn update_for_hint(&mut self, hinted: &Hinted, matches: &Vec<bool>) {
        match hinted {
            &Hinted::Color(color) => {
                for (card_info, &matched) in self.hand_info.iter_mut().zip(matches.iter()) {
                    card_info.mark_color(color, matched);
                }
            }
            &Hinted::Value(value) => {
                for (card_info, &matched) in self.hand_info.iter_mut().zip(matches.iter()) {
                    card_info.mark_value(value, matched);
                }
            }
        }
    }

    pub fn remove(&mut self, index: usize) -> CardPossibilityTable    { self.hand_info.remove(index) }
    pub fn push(&mut self, card_info: CardPossibilityTable)            { self.hand_info.push(card_info) }
    pub fn iter_mut(&mut self) -> slice::IterMut<CardPossibilityTable> { self.hand_info.iter_mut() }
    pub fn iter(&self) -> slice::Iter<CardPossibilityTable>            { self.hand_info.iter() }
    pub fn len(&self) -> usize                                         { self.hand_info.len() }
}
impl Index<usize> for HandInfo {
    type Output = CardPossibilityTable;
    fn index(&self, index: usize) -> &Self::Output {
        &self.hand_info[index]
    }
}
impl IndexMut<usize> for HandInfo {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.hand_info[index]
    }
}
