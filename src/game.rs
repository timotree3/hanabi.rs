use rand::{self, Rng};
use std::collections::HashSet;
use std::collections::HashMap;
use std::fmt;

/*
* Type definitions
*/

pub type Color = &'static str;
const COLORS: [Color; 5] = ["blue", "red", "yellow", "white", "green"];

pub type Value = u32;
// list of (value, count) pairs
const VALUE_COUNTS : [(Value, u32); 5] = [(1, 3), (2, 2), (3, 2), (4, 2), (5, 1)];

pub struct Card {
    pub color: Color,
    pub value: Value,
}
impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.color, self.value)
    }
}

#[derive(Debug)]
pub struct Pile(Vec<Card>);
// basically a stack of cards
impl Pile {
    pub fn new() -> Pile {
        Pile(Vec::new())
    }
    pub fn draw(&mut self) -> Option<Card> {
        self.0.pop()
    }
    pub fn place(&mut self, card: Card) {
        self.0.push(card);
    }
    pub fn take(&mut self, index: usize) -> Card {
        self.0.remove(index)
    }
    pub fn shuffle(&mut self) {
        rand::thread_rng().shuffle(&mut self.0[..]);
    }
}

pub type Hand = Vec<Card>;
pub type Player = u32;

pub struct GameOptions {
    pub num_players: u32,
    pub hand_size: u32,
    // when hits 0, you cannot hint
    pub total_hints: u32,
    // when hits 0, you lose
    pub total_lives: u32,
}

// The state of a given player:  all other players may see this
pub struct PlayerState {
    hand: Hand,
}

// State of everything except the player's hands
// Is completely common knowledge
pub struct BoardState {
    pub deck: Pile,
    pub discard: Pile,
    pub fireworks: HashMap<Color, Pile>,

    // // whose turn is it?
    pub next: Player,

    pub hints_remaining: u32,
    pub lives_remaining: u32,
    // only relevant when deck runs out
    turns_remaining: u32,
}

// complete game state (known to nobody!)
pub struct GameState {
    pub player_states: HashMap<Player, PlayerState>,
    pub board_state: BoardState,
}

// complete game view of a given player
pub struct GameStateView {
    // not yet implemented
    pub other_player_states: HashMap<Player, PlayerState>,
    pub board_state: BoardState,
}

impl GameState {
    pub fn new(opts: GameOptions) -> GameState {
        let mut deck = GameState::make_deck();

        let mut player_states : HashMap<Player, PlayerState> = HashMap::new();
        for i in 0..opts.num_players {
            let hand : Hand = (0..opts.hand_size)
                .map(|i| {
                    // we can assume the deck is big enough to draw initial hands
                    deck.draw().unwrap()
                })
                .collect::<Vec<_>>();
            let state = PlayerState {
                hand: hand,
            };
            player_states.insert(i,  state);
        }

        let mut fireworks : HashMap<Color, Pile> = HashMap::new();
        for color in COLORS.iter() {
            let mut pile = Pile::new();
            let card = Card { value: 0, color: color };
            pile.place(card);
            fireworks.insert(color, pile);
        }

        GameState {
            player_states: player_states,
            board_state: BoardState {
                deck: deck,
                fireworks: fireworks,
                discard: Pile::new(),
                next: 0,
                hints_remaining: opts.total_hints,
                lives_remaining: opts.total_lives,
                // only relevant when deck runs out
                turns_remaining: opts.num_players,
            }
        }
    }

    fn make_deck() -> Pile {
        let mut deck: Pile = Pile(Vec::new());

        for color in COLORS.iter() {
            for &(value, count) in VALUE_COUNTS.iter() {
                for _ in 0..3 {
                    deck.place(Card {color: color, value: 1});
                }
            }
        };
        deck.shuffle();
        println!("Created deck: {:?}", deck);
        deck
    }
}

enum Hint {
    Color,
    Value,
}

enum Turn {
    Hint,
    Discard,
    Play,
}

// Trait to implement for any valid Hanabi strategy
pub trait Strategy {
    fn decide(&mut self, &GameStateView) -> Turn;
    fn update(&mut self, Turn);
}

pub fn simulate_symmetric(opts: GameOptions, strategy: &Strategy) {
    let strategies = (0..opts.num_players).map(|_| { Box::new(strategy) }).collect();
    simulate(opts, strategies)
}

pub fn simulate(opts: GameOptions, strategies: Vec<Box<&Strategy>>) {
}
