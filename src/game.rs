use fnv::FnvHashMap;
use std::fmt;
use std::ops::Range;
use tracing::debug;

use crate::helpers::PerPlayer;

pub type Player = u32;

pub type Color = char;
pub const NUM_COLORS: usize = 5;
pub const COLORS: [Color; NUM_COLORS] = ['r', 'y', 'g', 'b', 'w'];

pub type Value = u32;
// list of values, assumed to be small to large
pub const NUM_VALUES: usize = 5;
pub const VALUES: [Value; NUM_VALUES] = [1, 2, 3, 4, 5];
pub const FINAL_VALUE: Value = 5;
/// Total number of cards in the deck (including starting hands)
pub const TOTAL_CARDS: u32 = NUM_COLORS as u32 * 10;

pub fn get_count_for_value(value: Value) -> u32 {
    match value {
        1 => 3,
        2 | 3 | 4 => 2,
        5 => 1,
        _ => {
            panic!("Unexpected value: {value}");
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Card {
    pub color: Color,
    pub value: Value,
}
impl Card {
    pub fn new(color: Color, value: Value) -> Card {
        Card { color, value }
    }
}
impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.color, self.value)
    }
}
impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.color, self.value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CardCounts {
    counts: FnvHashMap<Card, u32>,
}
impl CardCounts {
    pub fn new() -> CardCounts {
        let mut counts = FnvHashMap::default();
        for &color in COLORS.iter() {
            for &value in VALUES.iter() {
                counts.insert(Card::new(color, value), 0);
            }
        }
        CardCounts { counts }
    }

    pub fn get_count(&self, card: Card) -> u32 {
        *self.counts.get(&card).unwrap()
    }

    pub fn remaining(&self, card: Card) -> u32 {
        let count = self.get_count(card);
        get_count_for_value(card.value) - count
    }

    pub fn increment(&mut self, card: Card) {
        let count = self.counts.get_mut(&card).unwrap();
        *count += 1;
    }
}
impl fmt::Display for CardCounts {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for &color in COLORS.iter() {
            write!(f, "{color}: ")?;
            for &value in VALUES.iter() {
                let count = self.get_count(Card::new(color, value));
                let total = get_count_for_value(value);
                write!(f, "{count}/{total} {value}s")?;
                if value != FINAL_VALUE {
                    f.write_str(", ")?;
                }
            }
            f.write_str("\n")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Discard {
    size: u32,
    counts: CardCounts,
}
impl Discard {
    pub fn new() -> Discard {
        Discard {
            size: 0,
            counts: CardCounts::new(),
        }
    }

    pub fn has_all(&self, card: Card) -> bool {
        self.counts.remaining(card) == 0
    }

    pub fn remaining(&self, card: Card) -> u32 {
        self.counts.remaining(card)
    }

    pub fn place(&mut self, card: Card) {
        self.size += 1;
        self.counts.increment(card);
    }
}
impl fmt::Display for Discard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.counts)
    }
}

pub type Score = u32;
pub const PERFECT_SCORE: Score = (NUM_COLORS * NUM_VALUES) as u32;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Firework {
    pub color: Color,
    pub top: Value,
}
impl Firework {
    pub fn new(color: Color) -> Firework {
        Firework { color, top: 0 }
    }

    pub fn needed_value(&self) -> Option<Value> {
        if self.complete() {
            None
        } else {
            Some(self.top + 1)
        }
    }

    pub fn score(&self) -> Score {
        self.top
    }

    pub fn complete(&self) -> bool {
        self.top == FINAL_VALUE
    }

    pub fn place(&mut self, card: Card) {
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

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum Hinted {
    Color(Color),
    Value(Value),
}
impl fmt::Display for Hinted {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Hinted::Color(color) => {
                write!(f, "{color}")
            }
            Hinted::Value(value) => {
                write!(f, "{value}")
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Hint {
    pub player: Player,
    pub hinted: Hinted,
}

// represents the choice a player made in a given turn
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum TurnChoice {
    Hint(Hint),
    Discard(usize), // index of card to discard
    Play(usize),    // index of card to play
}

// represents what happened in a turn
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TurnResult {
    Hint(Vec<bool>),  // vector of whether each was in the hint
    Discard(Card),    // card discarded
    Play(Card, bool), // card played, whether it succeeded
}

// represents a turn taken in the game
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TurnRecord {
    pub player: Player,
    pub choice: TurnChoice,
    pub result: TurnResult,
}
pub type TurnHistory = Vec<TurnRecord>;

// represents possible settings for the game
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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

// State of everything except the player's hands
// Is all completely common knowledge
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BoardState<'game> {
    pub deck_size: u32,
    pub discard: Discard,
    pub fireworks: FnvHashMap<Color, Firework>,

    // which turn is it?
    pub turn: u32,
    pub turn_history: TurnHistory,
    // // whose turn is it?
    pub player: Player,

    pub hints_remaining: u32,
    pub lives_remaining: u32,
    // only relevant when deck runs out
    pub deckless_turns_remaining: u32,

    pub opts: &'game GameOptions,
}
impl<'game> BoardState<'game> {
    pub fn new(opts: &'game GameOptions, deck_size: u32) -> Self {
        let fireworks = COLORS
            .iter()
            .map(|&color| (color, Firework::new(color)))
            .collect::<FnvHashMap<_, _>>();

        BoardState {
            deck_size,
            fireworks,
            discard: Discard::new(),
            player: 0,
            turn: 1,
            hints_remaining: opts.num_hints,
            lives_remaining: opts.num_lives,
            turn_history: Vec::new(),
            // number of turns to play with deck length ran out
            deckless_turns_remaining: opts.num_players + 1,
            opts,
        }
    }

    fn try_add_hint(&mut self) {
        if self.hints_remaining < self.opts.num_hints {
            self.hints_remaining += 1;
        }
    }

    pub fn get_firework(&self, color: Color) -> &Firework {
        self.fireworks.get(&color).unwrap()
    }

    fn get_firework_mut(&mut self, color: Color) -> &mut Firework {
        self.fireworks.get_mut(&color).unwrap()
    }

    // returns whether a card would place on a firework
    pub fn is_playable(&self, card: Card) -> bool {
        Some(card.value) == self.get_firework(card.color).needed_value()
    }

    // best possible value we can get for firework of that color,
    // based on looking at discard + fireworks
    fn highest_attainable(&self, color: Color) -> Value {
        let firework = self.fireworks.get(&color).unwrap();
        if firework.complete() {
            return FINAL_VALUE;
        }
        let needed = firework.needed_value().unwrap();

        for &value in VALUES.iter() {
            if value < needed {
                // already have these cards
                continue;
            }
            let needed_card = Card::new(color, value);
            if self.discard.has_all(needed_card) {
                // already discarded all of these
                return value - 1;
            }
        }
        FINAL_VALUE
    }

    // is never going to play, based on discard + fireworks
    pub fn is_dead(&self, card: Card) -> bool {
        let firework = self.fireworks.get(&card.color).unwrap();
        firework.complete()
            || card.value < firework.needed_value().unwrap()
            || card.value > self.highest_attainable(card.color)
    }

    // can be discarded without necessarily sacrificing score, based on discard + fireworks
    pub fn is_dispensable(&self, card: Card) -> bool {
        self.is_dead(card) || self.discard.remaining(card) != 1
    }

    pub fn get_players(&self) -> Range<Player> {
        0..self.opts.num_players
    }

    pub fn score(&self) -> Score {
        self.fireworks.values().map(Firework::score).sum()
    }

    pub fn discard_size(&self) -> u32 {
        self.discard.size
    }

    pub fn player_to_left(&self, player: Player) -> Player {
        (player + 1) % self.opts.num_players
    }
    pub fn player_to_right(&self, player: Player) -> Player {
        (player + self.opts.num_players - 1) % self.opts.num_players
    }

    pub fn is_over(&self) -> bool {
        (self.lives_remaining == 0)
            || (self.deckless_turns_remaining == 0)
            || (self.score() == PERFECT_SCORE)
    }

    pub fn top_deck(&self) -> Option<CardId> {
        if self.deck_size > 0 {
            Some(TOTAL_CARDS - self.deck_size)
        } else {
            None
        }
    }
}
impl fmt::Display for BoardState<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_over() {
            writeln!(f, "Turn {} (GAME ENDED):", self.turn)?;
        } else {
            writeln!(f, "Turn {} (Player {}'s turn):", self.turn, self.player)?;
        }

        writeln!(f, "{} cards remaining in deck", self.deck_size)?;
        if self.deck_size == 0 {
            writeln!(
                f,
                "Deck is empty.  {} turns remaining in game",
                self.deckless_turns_remaining
            )?;
        }
        writeln!(
            f,
            "{}/{} hints remaining",
            self.hints_remaining, self.opts.num_hints
        )?;
        writeln!(
            f,
            "{}/{} lives remaining",
            self.lives_remaining, self.opts.num_lives
        )?;
        f.write_str("Fireworks:\n")?;
        for &color in COLORS.iter() {
            writeln!(f, "  {}", self.get_firework(color))?;
        }
        f.write_str("Discard:\n")?;
        writeln!(f, "{}\n", self.discard)?;

        Ok(())
    }
}

// Player-specific view on the game state
#[derive(Debug, Clone)]
pub struct PlayerView<'game> {
    /// The player whose view it is
    player: Player,
    /// The mapping from CardIds to cards, which is private and accessible only through careful methods
    deck: &'game [Card],
    /// The card IDs held by the players, which is common knowledge.
    hands: PerPlayer<Hand>,
    /// The board state which is common knowledge
    pub board: BoardState<'game>,
}

pub struct HandView<'view> {
    deck: Option<&'view [Card]>,
    hand: &'view [CardId],
}

impl HandView<'_> {
    fn deck(&self) -> &[Card] {
        self.deck
            .unwrap_or_else(|| panic!("Cannot look at your own hand!"))
    }

    pub fn cards(&self) -> impl DoubleEndedIterator<Item = Card> + '_ {
        let deck = self.deck();
        self.hand.iter().map(|&card_id| deck[card_id as usize])
    }

    pub fn pairs(&self) -> impl DoubleEndedIterator<Item = (CardId, Card)> + '_ {
        let deck = self.deck();
        self.hand
            .iter()
            .map(|&card_id| (card_id, deck[card_id as usize]))
    }

    pub fn size(&self) -> usize {
        self.hand.len()
    }

    pub fn newest_card(&self) -> Card {
        self.deck()[self.newest_id() as usize]
    }

    pub fn newest_id(&self) -> CardId {
        *self.hand.last().unwrap()
    }

    pub fn oldest_card(&self) -> Card {
        self.deck()[self.oldest_id() as usize]
    }

    pub fn nth_id(&self, index: usize) -> CardId {
        self.hand[index]
    }

    pub fn nth_card(&self, index: usize) -> Card {
        self.deck()[self.nth_id(index) as usize]
    }

    pub fn oldest_id(&self) -> CardId {
        *self.hand.first().unwrap()
    }

    pub fn contains(&self, card: Card) -> bool {
        self.cards().any(|other_card| card == other_card)
    }
}

impl<'game> PlayerView<'game> {
    pub fn me(&self) -> Player {
        self.player
    }

    pub fn hands(&self) -> &PerPlayer<Hand> {
        &self.hands
    }

    pub fn hand(&self, player: Player) -> HandView<'_> {
        let deck = if player == self.me() {
            None
        } else {
            Some(self.deck)
        };
        HandView {
            deck,
            hand: &self.hands[player],
        }
    }

    pub fn card(&self, card_id: CardId) -> Card {
        if let Some(next_card_id) = self.board.top_deck() {
            assert!(card_id < next_card_id, "Cannot look at cards in the deck!")
        }
        assert!(
            !self.hands[self.player].contains(&card_id),
            "Cannot look at own hand!"
        );
        self.deck[card_id as usize]
    }

    pub fn hand_size(&self, player: Player) -> usize {
        self.hands[player].len()
    }

    pub fn other_players(&self) -> impl Iterator<Item = Player> {
        let me = self.player;
        self.board.get_players().filter(move |&player| player != me)
    }

    pub fn can_see(&self, card: Card) -> bool {
        self.other_players()
            .any(|player| self.hand(player).contains(card))
    }

    pub fn someone_else_can_play(&self) -> bool {
        self.other_players().any(|player| {
            self.hand(player)
                .cards()
                .any(|card| self.board.is_playable(card))
        })
    }
}

// Every card is represented by its index in the deck.
pub type CardId = u32;
pub type Hand = Vec<CardId>;

// complete game state (known to nobody!)
#[derive(Debug)]
pub struct GameState<'game> {
    pub hands: PerPlayer<Hand>,
    pub deck: &'game [Card],
    pub board: BoardState<'game>,
}

impl fmt::Display for GameState<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("\n")?;
        f.write_str("======\n")?;
        f.write_str("Hands:\n")?;
        f.write_str("======\n")?;
        for player in self.board.get_players() {
            let hand = &self.hands[player];
            write!(f, "player {player}:")?;
            for &card_id in hand {
                let card = self.card(card_id);
                write!(f, "    {card}")?;
            }
            f.write_str("\n")?;
        }
        f.write_str("======\n")?;
        f.write_str("Board:\n")?;
        f.write_str("======\n")?;
        write!(f, "{}", self.board)?;
        Ok(())
    }
}

impl<'game> GameState<'game> {
    pub fn new(opts: &'game GameOptions, deck: &'game [Card]) -> Self {
        // Deal the starting hands by populating them with ascending card IDs
        // We deal them in exactly this way to match the way hanab.live does it
        let hands = PerPlayer::new(opts.num_players, |player| {
            (player * opts.hand_size..((player + 1) * opts.hand_size)).collect::<Vec<_>>()
        });

        let cards_in_starting_hands = opts.num_players * opts.hand_size;

        let board = BoardState::new(opts, deck.len() as u32 - cards_in_starting_hands);

        GameState { hands, deck, board }
    }

    pub fn get_players(&self) -> Range<Player> {
        self.board.get_players()
    }

    pub fn is_over(&self) -> bool {
        self.board.is_over()
    }

    pub fn score(&self) -> Score {
        self.board.score()
    }

    pub fn card(&self, card_id: CardId) -> Card {
        self.deck[card_id as usize]
    }

    pub fn hand(&self, player: Player) -> impl Iterator<Item = Card> + '_ {
        self.hands[player].iter().map(|&card_id| self.card(card_id))
    }

    // get the game state view of a particular player
    pub fn get_view(&self, player: Player) -> PlayerView<'game> {
        PlayerView {
            player,
            deck: self.deck,
            hands: self.hands.clone(),
            board: self.board.clone(),
        }
    }

    // takes a card from the player's hand, and replaces it if possible
    fn take_from_hand(&mut self, index: usize) -> Card {
        let card_id = self.hands[self.board.player].remove(index);
        self.card(card_id)
    }

    fn replenish_hand(&mut self) {
        if let Some(card_id) = self.board.top_deck() {
            let card = self.card(card_id);
            let hand = &mut self.hands[self.board.player];
            if (hand.len() as u32) < self.board.opts.hand_size {
                debug!("Drew new card, {card}");
                hand.push(card_id);
                self.board.deck_size -= 1;
            }
        }
    }

    pub fn process_choice(&mut self, choice: TurnChoice) -> TurnRecord {
        let turn_result = {
            match choice {
                TurnChoice::Hint(ref hint) => {
                    assert!(
                        self.board.hints_remaining > 0,
                        "Tried to hint with no hints remaining"
                    );
                    self.board.hints_remaining -= 1;
                    debug!("Hint to player {}, about {}", hint.player, hint.hinted);

                    assert_ne!(
                        self.board.player, hint.player,
                        "Player {} gave a hint to himself",
                        hint.player
                    );

                    let results = match hint.hinted {
                        Hinted::Color(color) => self
                            .hand(hint.player)
                            .map(|card| card.color == color)
                            .collect::<Vec<_>>(),
                        Hinted::Value(value) => self
                            .hand(hint.player)
                            .map(|card| card.value == value)
                            .collect::<Vec<_>>(),
                    };
                    if !self.board.opts.allow_empty_hints {
                        assert!(
                            results.iter().any(|matched| *matched),
                            "Tried hinting an empty hint"
                        );
                    }

                    TurnResult::Hint(results)
                }
                TurnChoice::Discard(index) => {
                    assert!(
                        self.board.hints_remaining < self.board.opts.num_hints,
                        "Tried to discard while at max hint count"
                    );

                    let card = self.take_from_hand(index);
                    debug!("Discard card in position {}, which is {}", index, card);
                    self.board.discard.place(card);

                    self.board.try_add_hint();
                    TurnResult::Discard(card)
                }
                TurnChoice::Play(index) => {
                    let card = self.take_from_hand(index);

                    debug!("Playing card at position {}, which is {}", index, card);
                    let playable = self.board.is_playable(card);
                    if playable {
                        {
                            let firework = self.board.get_firework_mut(card.color);
                            debug!("Successfully played {}!", card);
                            firework.place(card);
                        }
                        if card.value == FINAL_VALUE {
                            debug!("Firework complete for {}!", card.color);
                            self.board.try_add_hint();
                        }
                    } else {
                        self.board.discard.place(card);
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
        let turn_record = TurnRecord {
            player: self.board.player,
            result: turn_result,
            choice,
        };
        self.board.turn_history.push(turn_record.clone());

        self.replenish_hand();

        if self.board.deck_size == 0 {
            self.board.deckless_turns_remaining -= 1;
        }
        self.board.turn += 1;
        self.board.player = {
            let cur = self.board.player;
            self.board.player_to_left(cur)
        };
        assert_eq!(
            (self.board.turn - 1) % self.board.opts.num_players,
            self.board.player
        );

        turn_record
    }
}
