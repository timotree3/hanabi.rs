use rand::{thread_rng, Rng};
use std::collections::HashSet;
use std::collections::HashMap;
use std::fmt;

// Type definitions

pub type Color = &'static str;
pub type Value = i32;

pub struct Card {
    pub color: Color,
    pub value: Value,
}
impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.color, self.value)
    }
}

pub type Pile = Vec<Card>;
pub type Hand = Vec<Card>;
pub type Player = i32;

pub struct GameOptions {
    pub num_players: i32,
    pub hand_size: i32,
    pub total_hints: i32,
    pub total_lives: i32,
}

// The state of a given player:  all other players may see this
struct PlayerState {
    hand: Hand,
}

pub struct GameState {
    pub deck: Pile,
    // pub players: PlayerState,
    // pub discard: Pile,
    // pub fireworks: HashMap<Color, Pile>,
    // // whose turn is it?
    // pub next: Player,
    // pub hints_remaining: i32,
    // pub lives_remaining: i32,
    // // only relevant when deck runs out
    // pub turns_remaining: i32,
}

impl GameState {
    pub fn new(opts: GameOptions) -> GameState {
        let deck = GameState::make_deck();
        GameState {
            deck: deck,
        }
    }

    fn make_deck() -> Pile {
        let mut deck: Pile = Vec::new();
        for color in COLORS.iter() {
            for (value, count) in VALUE_COUNTS.iter() {
                for _ in 0..*count {
                    deck.push(Card {color: color, value: *value});
                }
            }
        };
        thread_rng().shuffle(&mut deck[..]);
        println!("Created deck: {:?}", deck);
        deck
    }
}

lazy_static! {
    static ref COLORS: HashSet<Color> = {
        vec!["blue", "red", "yellow", "white", "green"].into_iter().collect::<HashSet<_,_>>()
    };
    // map from value to count
    static ref VALUE_COUNTS: HashMap<Value, i32> = {
        let mut map = HashMap::new();
        map.insert(1, 3);
        map.insert(2, 2);
        map.insert(3, 2);
        map.insert(4, 2);
        map.insert(5, 1);
        map
    };
}

fn validate_card(card: &Card) {
}

trait Strategy {
    fn decide(&self) -> f64;
    fn update(&self) -> f64;
}

fn simulate() {
}
