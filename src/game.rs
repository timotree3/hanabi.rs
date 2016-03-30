use rand::{self, Rng, SeedableRng};
use std::collections::HashMap;
use std::fmt;
use std::iter;
use std::slice::IterMut;

pub use info::*;
pub use cards::*;

pub type Player = u32;

#[derive(Debug,Clone)]
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

#[derive(Debug,Clone)]
pub struct Hint {
    pub player: Player,
    pub hinted: Hinted,
}

// represents the choice a player made in a given turn
#[derive(Debug,Clone)]
pub enum TurnChoice {
    Hint(Hint),
    Discard(usize), // index of card to discard
    Play(usize),    // index of card to play
}

// represents what happened in a turn
#[derive(Debug,Clone)]
pub enum TurnResult {
    Hint(Vec<bool>),  // vector of whether each was in the hint
    Discard(Card),    // card discarded
    Play(Card, bool), // card played, whether it succeeded
}

// represents a turn taken in the game
#[derive(Debug,Clone)]
pub struct Turn {
    pub player: Player,
    pub choice: TurnChoice,
    pub result: TurnResult,
}

// represents possible settings for the game
pub struct GameOptions {
    pub num_players: u32,
    pub hand_size: u32,
    // when hits 0, you cannot hint
    pub num_hints: u32,
    // when hits 0, you lose
    pub num_lives: u32,
    // whether to allow hints that reveal no cards
    pub allow_empty_hints: bool,
}

// The state of a given player:  all other players may see this
#[derive(Debug,Clone)]
pub struct PlayerState {
    // the player's actual hand
    pub hand: Cards,
    // represents what is common knowledge about the player's hand
    pub info: Vec<SimpleCardInfo>,
}
impl fmt::Display for PlayerState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str("hand:    "));

        let mut i = 0;
        for card in &self.hand {
            let info : &SimpleCardInfo = &self.info[i];
            try!(f.write_str(&format!("{} =? {: <15} ", card, info)));
            i += 1;
        }
        Ok(())
    }
}
impl PlayerState {
    pub fn new(hand: Cards) -> PlayerState {
        let infos = (0..hand.len()).map(|_| {
            SimpleCardInfo::new()
        }).collect::<Vec<_>>();
        PlayerState {
            hand: hand,
            info: infos,
        }
    }

    pub fn take(&mut self, index: usize) -> (Card, SimpleCardInfo) {
        let card = self.hand.remove(index);
        let info = self.info.remove(index);
        (card, info)
    }

    pub fn place(&mut self, card: Card) {
        self.hand.push(card);
        self.info.push(SimpleCardInfo::new());
    }

    fn hand_info_iter_mut<'a>(&'a mut self) ->
        iter::Zip<IterMut<'a, Card>, IterMut<'a, SimpleCardInfo>>
    {
        self.hand.iter_mut().zip(self.info.iter_mut())
    }

    pub fn reveal(&mut self, hinted: &Hinted) -> Vec<bool> {
        match hinted {
            &Hinted::Color(ref color) => {
                self.hand_info_iter_mut().map(|(card, info)| {
                    let matches = card.color == *color;
                    info.mark_color(color, matches);
                    matches
                }).collect::<Vec<_>>()
            }
            &Hinted::Value(ref value) => {
                self.hand_info_iter_mut().map(|(card, info)| {
                    let matches = card.value == *value;
                    info.mark_value(value, matches);
                    matches
                }).collect::<Vec<_>>()
            }
        }
    }
}

fn new_deck(seed: u32) -> Cards {
    let mut deck: Cards = Cards::new();

    for color in COLORS.iter() {
        for value in VALUES.iter() {
            let count = get_count_for_value(value);
            for _ in 0..count {
                deck.push(Card::new(color, value.clone()));
            }
        }
    };

    rand::ChaChaRng::from_seed(&[seed]).shuffle(&mut deck[..]);

    trace!("Created deck: {:?}", deck);
    deck
}

// State of everything except the player's hands
// Is all completely common knowledge
#[derive(Debug,Clone)]
pub struct BoardState {
    deck: Cards,
    pub total_cards: u32,
    pub discard: Discard,
    pub fireworks: HashMap<Color, Firework>,

    pub num_players: u32,

    // which turn is it?
    pub turn: u32,
    // // whose turn is it?
    pub player: Player,
    pub hand_size: u32,

    pub hints_total: u32,
    pub hints_remaining: u32,
    pub allow_empty_hints: bool,
    pub lives_total: u32,
    pub lives_remaining: u32,
    pub turn_history: Vec<Turn>,
    // only relevant when deck runs out
    pub deckless_turns_remaining: u32,
}
impl BoardState {
    pub fn new(opts: &GameOptions, seed: u32) -> BoardState {
        let mut fireworks : HashMap<Color, Firework> = HashMap::new();
        for color in COLORS.iter() {
            fireworks.insert(color, Firework::new(color));
        }
        let deck = new_deck(seed);
        let total_cards = deck.len() as u32;

        BoardState {
            deck: deck,
            total_cards: total_cards,
            fireworks: fireworks,
            discard: Discard::new(),
            num_players: opts.num_players,
            hand_size: opts.hand_size,
            player: 0,
            turn: 1,
            allow_empty_hints: opts.allow_empty_hints,
            hints_total: opts.num_hints,
            hints_remaining: opts.num_hints,
            lives_total: opts.num_lives,
            lives_remaining: opts.num_lives,
            turn_history: Vec::new(),
            // number of turns to play with deck length ran out
            deckless_turns_remaining: opts.num_players + 1,
        }
    }

    fn try_add_hint(&mut self) {
        if self.hints_remaining < self.hints_total {
            self.hints_remaining += 1;
        }
    }

    pub fn get_firework(&self, color: &Color) -> &Firework {
        self.fireworks.get(color).unwrap()
    }

    fn get_firework_mut(&mut self, color: &Color) -> &mut Firework {
        self.fireworks.get_mut(color).unwrap()
    }

    // returns whether a card would place on a firework
    pub fn is_playable(&self, card: &Card) -> bool {
        Some(card.value) == self.get_firework(&card.color).desired_value()
    }

    pub fn was_played(&self, card: &Card) -> bool {
        let firework = self.fireworks.get(card.color).unwrap();
        if firework.complete() {
            true
        } else {
            card.value < firework.desired_value().unwrap()
        }
    }

    // best possible value we can get for firework of that color,
    // based on looking at discard + fireworks
    fn highest_attainable(&self, color: &Color) -> Value {
        let firework = self.fireworks.get(color).unwrap();
        if firework.complete() {
            return FINAL_VALUE;
        }
        let desired = firework.desired_value().unwrap();

        for value in VALUES.iter() {
            if *value < desired {
                // already have these cards
                continue
            }
            let needed_card = Card::new(color, value.clone());
            if self.discard.has_all(&needed_card) {
                // already discarded all of these
                return value - 1;
            }
        }
        return FINAL_VALUE;
    }

    // is never going to play, based on discard + fireworks
    pub fn is_dead(&self, card: &Card) -> bool {
        let firework = self.fireworks.get(card.color).unwrap();
        if firework.complete() {
            true
        } else {
            let desired = firework.desired_value().unwrap();
            if card.value < desired {
                true
            } else {
                card.value > self.highest_attainable(&card.color)
            }
        }
    }

    // can be discarded without necessarily sacrificing score, based on discard + fireworks
    pub fn is_dispensable(&self, card: &Card) -> bool {
        let firework = self.fireworks.get(card.color).unwrap();
        if firework.complete() {
            true
        } else {
            let desired = firework.desired_value().unwrap();
            if card.value < desired {
                true
            } else {
                if card.value > self.highest_attainable(&card.color) {
                    true
                } else {
                    self.discard.remaining(&card) != 1
                }
            }
        }
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

    pub fn deck_size(&self) -> u32 {
        self.deck.len() as u32
    }

    pub fn discard_size(&self) -> u32 {
        self.discard.cards.len() as u32
    }

    pub fn player_to_left(&self, player: &Player) -> Player {
        (player + 1) % self.num_players
    }
    pub fn player_to_right(&self, player: &Player) -> Player {
        (player + self.num_players - 1) % self.num_players
    }

    pub fn is_over(&self) -> bool {
        (self.lives_remaining == 0) || (self.deckless_turns_remaining == 0)
    }
}
impl fmt::Display for BoardState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_over() {
            try!(f.write_str(&format!(
                "Turn {} (GAME ENDED):\n", self.turn
            )));
        } else {
            try!(f.write_str(&format!(
                "Turn {} (Player {}'s turn):\n", self.turn, self.player
            )));
        }

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
        for color in COLORS.iter() {
            try!(f.write_str(&format!("  {}\n", self.get_firework(color))));
        }
        try!(f.write_str("Discard:\n"));
        try!(f.write_str(&format!("{}\n", self.discard)));

        Ok(())
    }
}

// complete game view of a given player
pub trait GameView {
    fn me(&self) -> Player;
    fn my_info(&self) -> &Vec<SimpleCardInfo>;
    fn get_state(&self, player: &Player) -> &PlayerState;
    fn get_board(&self) -> &BoardState;

    fn get_hand(&self, player: &Player) -> &Cards {
        assert!(self.me() != *player, "Cannot query about your own cards!");
        &self.get_state(player).hand
    }

    fn hand_size(&self, player: &Player) -> usize {
        if self.me() == *player {
            self.my_info().len()
        } else {
            self.get_hand(player).len()
        }
    }

    fn has_card(&self, player: &Player, card: &Card) -> bool {
        for other_card in self.get_hand(player) {
            if *card == *other_card {
                return true;
            }
        }
        false
    }

    fn can_see(&self, card: &Card) -> bool {
        for other_player in self.get_board().get_players() {
            if self.me() == other_player {
                continue
            }
            if self.has_card(&other_player, card) {
                return true;
            }
        }
        false
    }
}

// version of game view that is borrowed.  used in simulator for efficiency,
#[derive(Debug)]
pub struct BorrowedGameView<'a> {
    // the player whose view it is
    pub player: Player,
    // what is known about their own hand (and thus common knowledge)
    pub info: &'a Vec<SimpleCardInfo>,
    // the cards of the other players, as well as the information they have
    pub other_player_states: HashMap<Player, &'a PlayerState>,
    // board state
    pub board: &'a BoardState,
}
impl <'a> GameView for BorrowedGameView<'a> {
    fn me(&self) -> Player {
        self.player
    }
    fn my_info(&self) -> &Vec<SimpleCardInfo> {
        self.info
    }
    fn get_state(&self, player: &Player) -> &PlayerState {
        assert!(self.me() != *player, "Cannot query about your own state!");
        self.other_player_states.get(player).unwrap()
    }
    fn get_board(&self) -> &BoardState {
        self.board
    }
}

// version of game view, may be useful to strategies
#[derive(Debug)]
pub struct OwnedGameView {
    // the player whose view it is
    pub player: Player,
    // what is known about their own hand (and thus common knowledge)
    pub info: Vec<SimpleCardInfo>,
    // the cards of the other players, as well as the information they have
    pub other_player_states: HashMap<Player, PlayerState>,
    // board state
    pub board: BoardState,
}
impl OwnedGameView {
    pub fn clone_from(borrowed_view: &BorrowedGameView) -> OwnedGameView {
        let mut info : Vec<SimpleCardInfo> = Vec::new();
        for card_info in borrowed_view.info.iter() {
            info.push((*card_info).clone());
        }
        let mut other_player_states : HashMap<Player, PlayerState> = HashMap::new();
        for (other_player, player_state) in &borrowed_view.other_player_states {
            other_player_states.insert(*other_player, (*player_state).clone());
        }

        OwnedGameView {
            player: borrowed_view.player.clone(),
            info: info,
            other_player_states: other_player_states,
            board: (*borrowed_view.board).clone(),
        }
    }
}
impl GameView for OwnedGameView {
    fn me(&self) -> Player {
        self.player
    }
    fn my_info(&self) -> &Vec<SimpleCardInfo> {
        &self.info
    }
    fn get_state(&self, player: &Player) -> &PlayerState {
        assert!(self.me() != *player, "Cannot query about your own state!");
        self.other_player_states.get(player).unwrap()
    }
    fn get_board(&self) -> &BoardState {
        &self.board
    }
}


// complete game state (known to nobody!)
#[derive(Debug)]
pub struct GameState {
    pub player_states: HashMap<Player, PlayerState>,
    pub board: BoardState,
}
impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str("\n"));
        try!(f.write_str("======\n"));
        try!(f.write_str("Hands:\n"));
        try!(f.write_str("======\n"));
        for player in self.board.get_players() {
            let state = &self.player_states.get(&player).unwrap();
            try!(f.write_str(&format!("player {} {}\n", player, state)));
        }
        try!(f.write_str("======\n"));
        try!(f.write_str("Board:\n"));
        try!(f.write_str("======\n"));
        try!(f.write_str(&format!("{}", self.board)));
        Ok(())
    }
}

impl GameState {
    pub fn new(opts: &GameOptions, seed: u32) -> GameState {
        let mut board = BoardState::new(opts, seed);

        let mut player_states : HashMap<Player, PlayerState> = HashMap::new();
        for i in 0..opts.num_players {
            let hand = (0..opts.hand_size).map(|_| {
                    // we can assume the deck is big enough to draw initial hands
                    board.deck.pop().unwrap()
                }).collect::<Vec<_>>();
            player_states.insert(
                i,  PlayerState::new(hand),
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
        self.board.is_over()
    }

    pub fn score(&self) -> Score {
        self.board.score()
    }

    // get the game state view of a particular player
    pub fn get_view(&self, player: Player) -> BorrowedGameView {
        let mut other_player_states = HashMap::new();
        for (other_player, state) in &self.player_states {
            if player != *other_player {
                other_player_states.insert(*other_player, state);
            }
        }
        BorrowedGameView {
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
        card
    }

    fn replenish_hand(&mut self) {
        let ref mut state = self.player_states.get_mut(&self.board.player).unwrap();
        if (state.hand.len() as u32) < self.board.hand_size {
            if let Some(new_card) = self.board.deck.pop() {
                debug!("Drew new card, {}", new_card);
                state.place(new_card);
            }
        }
    }

    pub fn process_choice(&mut self, choice: TurnChoice) -> Turn {
        let turn_result = {
            match choice {
                TurnChoice::Hint(ref hint) => {
                    assert!(self.board.hints_remaining > 0,
                            "Tried to hint with no hints remaining");
                    self.board.hints_remaining -= 1;
                    debug!("Hint to player {}, about {}", hint.player, hint.hinted);

                    assert!(self.board.player != hint.player,
                            format!("Player {} gave a hint to himself", hint.player));

                    let ref mut state = self.player_states.get_mut(&hint.player).unwrap();
                    let results = state.reveal(&hint.hinted);
                    if (!self.board.allow_empty_hints) && (results.iter().all(|matched| !matched)) {
                        panic!("Tried hinting an empty hint");
                    }
                    TurnResult::Hint(results)
                }
                TurnChoice::Discard(index) => {
                    let card = self.take_from_hand(index);
                    debug!("Discard card in position {}, which is {}", index, card);
                    self.board.discard.place(card.clone());

                    self.board.try_add_hint();
                    TurnResult::Discard(card)
                }
                TurnChoice::Play(index) => {
                    let card = self.take_from_hand(index);

                    debug!(
                        "Playing card at position {}, which is {}",
                        index, card
                    );
                    let playable = self.board.is_playable(&card);
                    if playable {
                        {
                            let firework = self.board.get_firework_mut(&card.color);
                            debug!("Successfully played {}!", card);
                            firework.place(&card);
                        }
                        if card.value == FINAL_VALUE {
                            debug!("Firework complete for {}!", card.color);
                            self.board.try_add_hint();
                        }
                    } else {
                        self.board.discard.place(card.clone());
                        self.board.lives_remaining -= 1;
                        debug!(
                            "Removing a life! Lives remaining: {}",
                            self.board.lives_remaining
                        );
                    }
                    TurnResult::Play(card, playable)
                }
            }
        };
        let turn = Turn {
            player: self.board.player.clone(),
            result: turn_result,
            choice: choice,
        };
        self.board.turn_history.push(turn.clone());

        self.replenish_hand();

        if self.board.deck.len() == 0 {
            self.board.deckless_turns_remaining -= 1;
        }
        self.board.turn += 1;
        self.board.player = {
            let cur = self.board.player;
            self.board.player_to_left(&cur)
        };
        assert_eq!((self.board.turn - 1) % self.board.num_players, self.board.player);

        turn
    }
}
