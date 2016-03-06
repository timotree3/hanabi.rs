
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
const FINAL_VALUE : Value = 5;

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
    pub fn top(&self) -> Option<&Card> {
        self.0.last()
    }
    pub fn shuffle(&mut self) {
        rand::thread_rng().shuffle(&mut self.0[..]);
    }
    pub fn size(&self) -> usize {
        self.0.len()
    }
}

pub type Player = u32;

#[derive(Debug)]
pub enum Hint {
    Color,
    Value,
}

// represents the choice a player made in a given turn
#[derive(Debug)]
pub enum TurnChoice {
    Hint,
    Discard(usize),
    Play(usize),
}

// represents a turn taken in the game
pub struct Turn<'a> {
    pub player: &'a Player,
    pub choice: &'a TurnChoice,
}

// represents possible settings for the game
pub struct GameOptions {
    pub num_players: u32,
    pub hand_size: u32,
    // when hits 0, you cannot hint
    pub num_hints: u32,
    // when hits 0, you lose
    pub num_lives: u32,
}

// The state of a given player:  all other players may see this
#[derive(Debug)]
pub struct PlayerState {
    // the player's actual hand
    pub hand: Pile,
    // represents what is common knowledge about the player's hand
    // pub known: ,
}

// State of everything except the player's hands
// Is all completely common knowledge
#[derive(Debug)]
pub struct BoardState {
    deck: Pile,
    pub discard: Pile,
    pub fireworks: HashMap<Color, Pile>,

    pub num_players: u32,

    // which turn is it?
    pub turn: u32,
    // // whose turn is it?
    pub player: Player,

    pub hints_total: u32,
    pub hints_remaining: u32,
    pub lives_total: u32,
    pub lives_remaining: u32,
    // only relevant when deck runs out
    deckless_turns_remaining: u32,
}

// complete game view of a given player
// state will be borrowed GameState
#[derive(Debug)]
pub struct GameStateView<'a> {
    // the player whose view it is
    pub player: Player,
    // what is known about their own hand
    // pub known:
    // the cards of the other players
    pub other_player_states: HashMap<Player, &'a PlayerState>,
    // board state
    pub board: &'a BoardState,
}

// complete game state (known to nobody!)
#[derive(Debug)]
pub struct GameState {
    pub player_states: HashMap<Player, PlayerState>,
    pub board: BoardState,
}

pub type Score = u32;

impl GameState {
    pub fn new(opts: &GameOptions) -> GameState {
        let mut deck = GameState::make_deck();

        let mut player_states : HashMap<Player, PlayerState> = HashMap::new();
        for i in 0..opts.num_players {
            let raw_hand = (0..opts.hand_size).map(|_| {
                    // we can assume the deck is big enough to draw initial hands
                    deck.draw().unwrap()
                }).collect::<Vec<_>>();
            let state = PlayerState {
                hand: Pile(raw_hand),
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
            board: BoardState {
                deck: deck,
                fireworks: fireworks,
                discard: Pile::new(),
                num_players: opts.num_players,
                player: 0,
                turn: 1,
                hints_total: opts.num_hints,
                hints_remaining: opts.num_hints,
                lives_total: opts.num_lives,
                lives_remaining: opts.num_lives,
                // number of turns to play with deck length ran out
                deckless_turns_remaining: opts.num_players + 1,
            }
        }
    }

    fn make_deck() -> Pile {
        let mut deck: Pile = Pile(Vec::new());

        for color in COLORS.iter() {
            for &(value, count) in VALUE_COUNTS.iter() {
                for _ in 0..count {
                    deck.place(Card {color: color, value: value});
                }
            }
        };
        deck.shuffle();
        info!("Created deck: {:?}", deck);
        deck
    }

    pub fn get_players(&self) -> Vec<Player> {
        (0..self.board.num_players).collect::<Vec<_>>()
    }

    pub fn is_over(&self) -> bool {
        // TODO: add condition that fireworks cannot be further completed?
        (self.board.lives_remaining == 0) ||
        (self.board.deckless_turns_remaining == 0)
    }

    pub fn score(&self) -> Score {
        let mut score = 0;
        for (_, firework) in &self.board.fireworks {
            // subtract one to account for the 0 we pushed
            score += firework.size() - 1;
        }
        score as u32
    }

    // get the game state view of a particular player
    pub fn get_view(&self, player: Player) -> GameStateView {
        let mut other_player_states = HashMap::new();
        for (other_player, state) in &self.player_states {
            if player != *other_player {
                other_player_states.insert(player, state);
            }
        }
        GameStateView {
            player: player,
            other_player_states: other_player_states,
            board: &self.board,
        }
    }

    // takes a card from the player's hand, and replaces it if possible
    fn take_from_hand(&mut self, index: usize) -> Card {
        let ref mut hand = self.player_states.get_mut(&self.board.player).unwrap().hand;
        let card = hand.take(index);
        if let Some(new_card) = self.board.deck.draw() {
            hand.place(new_card);
        }
        card
    }

    fn try_add_hint(&mut self) {
        if self.board.hints_remaining < self.board.hints_total {
            self.board.hints_remaining += 1;
        }
    }

    pub fn process_choice(&mut self, choice: &TurnChoice) {
        match *choice {
            TurnChoice::Hint => {
                assert!(self.board.hints_remaining > 0);
                self.board.hints_remaining -= 1;
                // TODO: actually inform player of values..
                // nothing to update, really...
                // TODO: manage common knowledge
            }
            TurnChoice::Discard(index) => {
                let card = self.take_from_hand(index);
                self.board.discard.place(card);

                self.try_add_hint();
            }
            TurnChoice::Play(index) => {
                let card = self.take_from_hand(index);

                debug!(
                    "Here!  Playing card at {}, which is {:?}",
                    index, card
                );

                let mut firework_made = false;

                {
                    let ref mut firework = self.board.fireworks.get_mut(&card.color).unwrap();

                    let playable = {
                        let under_card = firework.top().unwrap();
                        card.value == under_card.value + 1
                    };

                    if playable {
                        firework_made = card.value == FINAL_VALUE;
                        firework.place(card);
                    } else {
                        self.board.discard.place(card);
                        self.board.lives_remaining -= 1;
                        debug!(
                            "Removing a life! Lives remaining: {}",
                            self.board.lives_remaining
                        );
                    }
                }

                if firework_made {
                    self.try_add_hint();
                }
            }
        }

        if self.board.deck.size() == 0 {
            self.board.deckless_turns_remaining -= 1;
        }
        self.board.turn += 1;
        self.board.player = (self.board.player + 1) % self.board.num_players;
        assert_eq!((self.board.turn - 1) % self.board.num_players, self.board.player);

    }
}
