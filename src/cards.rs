use std::collections::HashMap;
use std::fmt;

pub type Color = char;
pub const COLORS: [Color; 5] = ['r', 'y', 'g', 'b', 'w'];

pub type Value = u32;
// list of values, assumed to be small to large
pub const VALUES : [Value; 5] = [1, 2, 3, 4, 5];
pub const FINAL_VALUE : Value = 5;

pub fn get_count_for_value(value: Value) -> u32 {
    match value {
        1         => 3,
        2 | 3 | 4 => 2,
        5         => 1,
        _ => { panic!(format!("Unexpected value: {}", value)); }
    }
}

#[derive(Debug,Clone,PartialEq,Eq,Hash,Ord,PartialOrd)]
pub struct Card {
    pub color: Color,
    pub value: Value,
}
impl Card {
    pub fn new(color: Color, value: Value) -> Card {
        Card { color: color, value: value }
    }
}
impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.color, self.value)
    }
}

pub type Cards = Vec<Card>;

#[derive(Debug,Clone)]
pub struct CardCounts {
    counts: HashMap<Card, u32>,
}
impl CardCounts {
    pub fn new() -> CardCounts {
        let mut counts = HashMap::new();
        for &color in COLORS.iter() {
            for &value in VALUES.iter() {
                counts.insert(Card::new(color, value), 0);
            }
        }
        CardCounts {
            counts: counts,
        }
    }

    pub fn get_count(&self, card: &Card) -> u32 {
        *self.counts.get(card).unwrap()
    }

    pub fn remaining(&self, card: &Card) -> u32 {
        let count = self.get_count(card);
        get_count_for_value(card.value) - count
    }

    pub fn increment(&mut self, card: &Card) {
        let count = self.counts.get_mut(card).unwrap();
        *count += 1;
    }
}
impl fmt::Display for CardCounts {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for &color in COLORS.iter() {
            try!(f.write_str(&format!(
                "{}: ", color,
            )));
            for &value in VALUES.iter() {
                let count = self.get_count(&Card::new(color, value));
                let total = get_count_for_value(value);
                try!(f.write_str(&format!(
                    "{}/{} {}s", count, total, value
                )));
                if value != FINAL_VALUE {
                    try!(f.write_str(", "));
                }
            }
            try!(f.write_str("\n"));
        }
        Ok(())
    }
}

#[derive(Debug,Clone)]
pub struct Discard {
    pub cards: Cards,
    counts: CardCounts,
}
impl Discard {
    pub fn new() -> Discard {
        Discard {
            cards: Cards::new(),
            counts: CardCounts::new(),
        }
    }

    pub fn has_all(&self, card: &Card) -> bool {
        self.counts.remaining(card) == 0
    }

    pub fn remaining(&self, card: &Card) -> u32 {
        self.counts.remaining(card)
    }

    pub fn place(&mut self, card: Card) {
        self.counts.increment(&card);
        self.cards.push(card);
    }
}
impl fmt::Display for Discard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // try!(f.write_str(&format!(
        //     "{}", self.cards,
        // )));
        write!(f, "{}", self.counts)
    }
}

pub type Score = u32;
pub const PERFECT_SCORE: Score = 25;

#[derive(Debug,Clone)]
pub struct Firework {
    pub color: Color,
    pub top: Value,
}
impl Firework {
    pub fn new(color: Color) -> Firework {
        Firework {
            color: color,
            top: 0,
        }
    }

    pub fn needed_value(&self) -> Option<Value> {
        if self.complete() { None } else { Some(self.top + 1) }
    }

    pub fn score(&self) -> Score {
        self.top
    }

    pub fn complete(&self) -> bool {
        self.top == FINAL_VALUE
    }

    pub fn place(&mut self, card: &Card) {
        assert!(
            card.color == self.color,
            "Attempted to place card on firework of wrong color!"
        );
        assert!(
            Some(card.value) == self.needed_value(),
            "Attempted to place card of wrong value on firework!"
        );
        self.top = card.value;
    }
}
impl fmt::Display for Firework {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.complete() {
            write!(f, "{} firework complete!", self.color)
        } else {
            write!(f, "{} firework at {}", self.color, self.top)
        }
    }
}
