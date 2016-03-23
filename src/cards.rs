use std::collections::HashMap;
use std::fmt;

pub type Color = &'static str;
pub const COLORS: [Color; 5] = ["red", "yellow", "green", "blue", "white"];
pub fn display_color(color: Color) -> char {
    color.chars().next().unwrap()
}

pub type Value = u32;
// list of values, assumed to be small to large
pub const VALUES : [Value; 5] = [1, 2, 3, 4, 5];
pub const FINAL_VALUE : Value = 5;

pub fn get_count_for_value(value: &Value) -> u32 {
    match *value {
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
        write!(f, "{}{}", display_color(self.color), self.value)
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
        for color in COLORS.iter() {
            for value in VALUES.iter() {
                counts.insert(Card::new(*color, *value), 0);
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
        get_count_for_value(&card.value) - count
    }

    pub fn add(&mut self, card: &Card) {
        let count = self.counts.get_mut(card).unwrap();
        *count += 1;
    }
}
impl fmt::Display for CardCounts {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for color in COLORS.iter() {
            try!(f.write_str(&format!(
                "{}: ", display_color(color),
            )));
            for value in VALUES.iter() {
                let count = self.get_count(&Card::new(color, *value));
                let total = get_count_for_value(value);
                try!(f.write_str(&format!(
                    "{}/{} {}s", count, total, value
                )));
                if *value != FINAL_VALUE {
                    try!(f.write_str(", "));
                }
            }
            try!(f.write_str("\n"));
        }
        Ok(())
    }
}

#[derive(Debug)]
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

    pub fn get_count(&self, card: &Card) -> u32 {
        self.counts.get_count(card)
    }

    pub fn has_all(&self, card: &Card) -> bool {
        self.counts.remaining(card) == 0
    }

    pub fn remaining(&self, card: &Card) -> u32 {
        self.counts.remaining(card)
    }

    pub fn place(&mut self, card: Card) {
        self.counts.add(&card);
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

