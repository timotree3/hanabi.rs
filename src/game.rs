
use rand::{self, Rng};
use std::convert::From;
use std::collections::HashMap;
use std::fmt;

use info::*;

/*
* Type definitions
*/

pub type Color = &'static str;
pub const COLORS: [Color; 5] = ["blue", "red", "yellow", "white", "green"];

pub type Value = u32;
// list of (value, count) pairs
pub const VALUES : [Value; 5] = [1, 2, 3, 4, 5];
pub const VALUE_COUNTS : [(Value, u32); 5] = [(1, 3), (2, 2), (3, 2), (4, 2), (5, 1)];
pub const FINAL_VALUE : Value = 5;

#[derive(Debug)]
pub struct Card {
    pub color: Color,
    pub value: Value,
}
impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let colorchar = self.color.chars().next().unwrap();
        write!(f, "{}{}", colorchar, self.value)
    }
}

#[derive(Debug)]
// basically a stack of cards, or card info
pub struct Pile<T>(Vec<T>);
impl <T> Pile<T> {
    pub fn new() -> Pile<T> {
        Pile(Vec::<T>::new())
    }
    pub fn draw(&mut self) -> Option<T> {
        self.0.pop()
    }
    pub fn place(&mut self, item: T) {
        self.0.push(item);
    }
    pub fn take(&mut self, index: usize) -> T {
        self.0.remove(index)
    }
    pub fn top(&self) -> Option<&T> {
        self.0.last()
    }
    pub fn shuffle(&mut self) {
        rand::thread_rng().shuffle(&mut self.0[..]);
    }
    pub fn size(&self) -> usize {
        self.0.len()
    }
}
impl <T> From<Vec<T>> for Pile<T> {
    fn from(items: Vec<T>) -> Pile<T> {
        Pile(items)
    }
}
impl <T> fmt::Display for Pile<T> where T: fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str("["));
        for item in &self.0 {
            try!(f.write_str(&format!("{}, ", item)));
        }
        try!(f.write_str(""));
        Ok(())
    }
}

pub type Cards = Pile<Card>;

pub type CardsInfo = Pile<CardInfo>;

pub type Player = u32;

#[derive(Debug)]
pub struct Firework {
    pub color: Color,
    pub cards: Cards,
}
impl Firework {
    fn new(color: Color) -> Firework {
        let mut cards = Cards::new();
        // have a 0, so it's easier to implement
        let card = Card { value: 0, color: color };
        cards.place(card);
        Firework {
            color: color,
            cards: cards,
        }
    }

    fn top_value(&self) -> Value {
        self.cards.top().unwrap().value
    }

    fn desired_value(&self) -> Option<Value> {
        if self.complete() {
            None
        } else {
            Some(self.top_value() + 1)
        }
    }

    fn score(&self) -> usize {
        // subtract one to account for the 0 we pushed
        self.cards.size() - 1
    }

    fn complete(&self) -> bool {
        self.top_value() == FINAL_VALUE
    }

    fn place(&mut self, card: Card) {
        assert!(
            card.color == self.color,
            "Attempted to place card on firework of wrong color!"
        );
        assert!(
            Some(card.value) == self.desired_value(),
            "Attempted to place card of wrong value on firework!"
        );

        self.cards.place(card);
    }
}
impl fmt::Display for Firework {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.complete() {
            write!(f, "{} firework complete!", self.color)
        } else {
            write!(f, "{} firework at {}", self.color, self.top_value())
        }
    }
}

#[derive(Debug)]
pub enum Hinted {
    Color(Color),
    Value(Value),
}
impl fmt::Display for Hinted {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Hinted::Color(color) => { write!(f, "{}", color) }
            &Hinted::Value(value) => { write!(f, "{}", value) }
        }
    }
}

#[derive(Debug)]
pub struct Hint {
    pub player: Player,
    pub hinted: Hinted,
}

// represents the choice a player made in a given turn
#[derive(Debug)]
pub enum TurnChoice {
    Hint(Hint),
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
    // TODO:
    // pub allow_empty_hints: bool,
}

// The state of a given player:  all other players may see this
#[derive(Debug)]
pub struct PlayerState {
    // the player's actual hand
    hand: Cards,
    // represents what is common knowledge about the player's hand
    info: CardsInfo,
}
impl fmt::Display for PlayerState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str("hand:    "));

        let mut i = 0;
        for card in &self.hand.0 {
            let info : &CardInfo = &self.info.0[i];
            try!(f.write_str(&format!("{} =? {: <15} ", card, info)));
            i += 1;
        }
        Ok(())
    }
}
impl PlayerState {
    pub fn new(hand: Cards) -> PlayerState {
        let infos = (0..hand.size()).map(|_| {
            CardInfo::new()
        }).collect::<Vec<_>>();
        PlayerState {
            hand: hand,
            info: CardsInfo::from(infos),
        }
    }

    pub fn take(&mut self, index: usize) -> (Card, CardInfo) {
        let card = self.hand.take(index);
        let info = self.info.take(index);
        (card, info)
    }

    pub fn place(&mut self, card: Card) {
        self.hand.place(card);
        self.info.place(CardInfo::new());
    }

    pub fn reveal(&mut self, hinted: &Hinted) {
        match hinted {
            &Hinted::Color(ref color) => {
                let mut i = 0;
                for card in &self.hand.0 {
                    self.info.0[i].color_info.mark(
                        color,
                        card.color == *color
                    );
                    i += 1;
                }
            }
            &Hinted::Value(ref value) => {
                let mut i = 0;
                for card in &self.hand.0 {
                    self.info.0[i].value_info.mark(
                        value,
                        card.value == *value
                    );
                    i += 1;
                }
            }

        }
    }
}

fn new_deck() -> Cards {
    let mut deck: Cards = Cards::from(Vec::new());

    for color in COLORS.iter() {
        for &(value, count) in VALUE_COUNTS.iter() {
            for _ in 0..count {
                deck.place(Card {color: color, value: value});
            }
        }
    };
    deck.shuffle();
    info!("Created deck: {}", deck);
    deck
}

// State of everything except the player's hands
// Is all completely common knowledge
#[derive(Debug)]
pub struct BoardState {
    deck: Cards,
    pub discard: Cards,
    pub fireworks: HashMap<Color, Firework>,

    pub num_players: u32,

    // which turn is it?
    pub turn: u32,
    // // whose turn is it?
    pub player: Player,

    pub hints_total: u32,
    pub hints_remaining: u32,
    pub lives_total: u32,
    pub lives_remaining: u32,
    // TODO:
    // pub turn_history: Vec<TurnChoice>,
    // only relevant when deck runs out
    pub deckless_turns_remaining: u32,
}
impl BoardState {
    pub fn new(opts: &GameOptions) -> BoardState {
        let mut fireworks : HashMap<Color, Firework> = HashMap::new();
        for color in COLORS.iter() {
            fireworks.insert(color, Firework::new(color));
        }

        BoardState {
            deck: new_deck(),
            fireworks: fireworks,
            discard: Cards::new(),
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

    fn try_add_hint(&mut self) {
        if self.hints_remaining < self.hints_total {
            self.hints_remaining += 1;
        }
    }

    // returns whether a card would place on a firework
    pub fn is_playable(&self, card: &Card) -> bool {
        let firework = self.fireworks.get(card.color).unwrap();
        Some(card.value) == firework.desired_value()
    }

    pub fn get_players(&self) -> Vec<Player> {
        (0..self.num_players).collect::<Vec<_>>()
    }

    pub fn score(&self) -> Score {
        let mut score = 0;
        for (_, firework) in &self.fireworks {
            score += firework.score();
        }
        score as u32
    }

    pub fn deck_size(&self) -> usize {
        self.deck.size()
    }

    pub fn player_to_left(&self, player: &Player) -> Player {
        (player + 1) % self.num_players
    }
    pub fn player_to_right(&self, player: &Player) -> Player {
        (player - 1) % self.num_players
    }
}
impl fmt::Display for BoardState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str(&format!(
            "Turn {} (Player {}'s turn):\n", self.turn, self.player
        )));
        let deck_size = self.deck_size();
        try!(f.write_str(&format!(
            "{} cards remaining in deck\n", deck_size
        )));
        if deck_size == 0 {
            try!(f.write_str(&format!(
                "Deck is empty.  {} turns remaining in game\n", self.deckless_turns_remaining
            )));
        }
        try!(f.write_str(&format!(
            "{}/{} hints remaining\n", self.hints_remaining, self.hints_total
        )));
        try!(f.write_str(&format!(
            "{}/{} lives remaining\n", self.lives_remaining, self.lives_total
        )));
        try!(f.write_str("Fireworks:\n"));
        for (_, firework) in &self.fireworks {
            try!(f.write_str(&format!("  {}\n", firework)));
        }
        //  discard: Cards::new(),

        Ok(())
    }
}

// complete game view of a given player
// state will be borrowed GameState
#[derive(Debug)]
pub struct GameStateView<'a> {
    // the player whose view it is
    pub player: Player,
    // what is known about their own hand (and thus common knowledge)
    pub info: &'a CardsInfo,
    // the cards of the other players, as well as the information they have
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
impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str("==========================\n"));
        try!(f.write_str("Hands:\n"));
        try!(f.write_str("==========================\n"));
        for player in 0..self.board.num_players {
            let state = &self.player_states.get(&player).unwrap();
            try!(f.write_str(&format!("player {} {}\n", player, state)));
        }
        try!(f.write_str("==========================\n"));
        try!(f.write_str("Board:\n"));
        try!(f.write_str("==========================\n"));
        try!(f.write_str(&format!("{}", self.board)));
        Ok(())
    }
}

pub type Score = u32;

impl GameState {
    pub fn new(opts: &GameOptions) -> GameState {
        let mut board = BoardState::new(opts);

        let mut player_states : HashMap<Player, PlayerState> = HashMap::new();
        for i in 0..opts.num_players {
            let raw_hand = (0..opts.hand_size).map(|_| {
                    // we can assume the deck is big enough to draw initial hands
                    board.deck.draw().unwrap()
                }).collect::<Vec<_>>();
            player_states.insert(
                i,  PlayerState::new(Cards::from(raw_hand)),
            );
        }

        GameState {
            player_states: player_states,
            board: board,
        }
    }

    pub fn get_players(&self) -> Vec<Player> {
        self.board.get_players()
    }

    pub fn is_over(&self) -> bool {
        // TODO: add condition that fireworks cannot be further completed?
        (self.board.lives_remaining == 0) ||
        (self.board.deckless_turns_remaining == 0)
    }

    pub fn score(&self) -> Score {
        self.board.score()
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
            info: &self.player_states.get(&player).unwrap().info,
            other_player_states: other_player_states,
            board: &self.board,
        }
    }

    // takes a card from the player's hand, and replaces it if possible
    fn take_from_hand(&mut self, index: usize) -> Card {
        let ref mut state = self.player_states.get_mut(&self.board.player).unwrap();
        let (card, _) = state.take(index);
        if let Some(new_card) = self.board.deck.draw() {
            info!("Drew new card, {}", new_card);
            state.place(new_card);
        }
        card
    }

    pub fn process_choice(&mut self, choice: &TurnChoice) {
        info!("Player {}'s move", self.board.player);
        match choice {
            &TurnChoice::Hint(ref hint) => {
                assert!(self.board.hints_remaining > 0,
                        "Tried to hint with no hints remaining");
                self.board.hints_remaining -= 1;
                info!("Hint to player {}, about {}", hint.player, hint.hinted);

                assert!(self.board.player != hint.player,
                        format!("Player {} gave a hint to himself", hint.player));

                let ref mut state = self.player_states.get_mut(&hint.player).unwrap();
                state.reveal(&hint.hinted);
            }
            &TurnChoice::Discard(index) => {
                let card = self.take_from_hand(index);
                info!("Discard card in position {}, which is {}", index, card);
                self.board.discard.place(card);

                self.board.try_add_hint();
            }
            &TurnChoice::Play(index) => {
                let card = self.take_from_hand(index);

                info!(
                    "Playing card at position {}, which is {}",
                    index, card
                );

                let mut firework_made = false;

                if self.board.is_playable(&card) {
                    let ref mut firework = self.board.fireworks.get_mut(&card.color).unwrap();
                    firework_made = card.value == FINAL_VALUE;
                    info!("Successfully played {}!", card);
                    if firework_made {
                        info!("Firework complete for {}!", card.color);
                    }
                    firework.place(card);
                } else {
                    self.board.discard.place(card);
                    self.board.lives_remaining -= 1;
                    info!(
                        "Removing a life! Lives remaining: {}",
                        self.board.lives_remaining
                    );
                }

                if firework_made {
                    self.board.try_add_hint();
                }
            }
        }

        if self.board.deck.size() == 0 {
            self.board.deckless_turns_remaining -= 1;
        }
        self.board.turn += 1;
        self.board.player = {
            let cur = self.board.player;
            self.board.player_to_left(&cur)
        };
        assert_eq!((self.board.turn - 1) % self.board.num_players, self.board.player);

    }
}
