use std::{cmp::Ordering, fmt::Display};

use crate::{
    game::{Card, CardId, Hinted, Player, PlayerView, TOTAL_CARDS},
    helpers::CardInfo,
};

use super::{Choice, Hint, State};

pub(super) struct PublicKnowledge {
    queued_clues: Vec<QueuedHatClue>,
    notes: Vec<Note>,
}

#[derive(Debug, Clone)]
struct QueuedHatClue {
    remaining_slot_sum: u8,
    remaining_plays: u8,
    // OPTIMIZATION: use bitset
    remaining_play_responders: Vec<Player>,
    remaining_discard_responders: Vec<Player>,
}

impl PublicKnowledge {
    pub fn first_turn() -> Self {
        Self {
            notes: vec![Note::default(); TOTAL_CARDS as usize],
            queued_clues: Vec::new(),
        }
    }

    pub fn describe_choice(&self, state: &State, choice: &Choice) -> Option<ChoiceDesc> {
        Some(ChoiceDesc {
            category: match choice {
                Choice::Play(card_id) => self.categorize_play(state, *card_id),
                Choice::Discard(card_id) => self.categorize_discard(state, *card_id),
                Choice::Hint(hint) => ChoiceCategory::Hint(self.describe_hint(state, hint)),
            },
        })
    }

    pub fn interpret_choice(
        &mut self,
        state: &State,
        choice: &Choice,
        // Mirrors the queued_hat_clue vector, providing the slot that this move will subtract
        // from the remaining sum of each hat clue if this move does not respond to it.
        // This is not exactly public information, so we pass it as an argument.
        reaction_if_ignored: Vec<Option<u8>>,
    ) {
        let ChoiceDesc {
            responder,
            category,
        } = self
            .describe_choice(state, choice)
            .expect("action taken should be conventional");

        let reaction = match category {
            ChoiceCategory::Hint(HintDesc {
                new_obvious_plays,
                new_known_trash,
                new_known_cards,
                hat_clue,
            }) => {
                for card_id in new_obvious_plays {
                    self.note_mut(card_id).play = true;
                }

                for card_id in new_known_trash {
                    self.note_mut(card_id).trash = true;
                }

                for card_id in new_known_cards {
                    self.note_mut(card_id).known = true;
                }

                // todo: add a queued hat clue
                Reaction::Ignore
            }
            ChoiceCategory::ExpectedPlay
            | ChoiceCategory::ExpectedDiscard
            | ChoiceCategory::KnownMisplay
            | ChoiceCategory::KnownDiscard => {
                // TODO:
                // - KnownDiscard should give elim/promise position
                // - KnownMisplay could mean something
                Reaction::Ignore
            }
            ChoiceCategory::UnexpectedPlay { slot } => Reaction::Play { slot },
            ChoiceCategory::UnexpectedDiscard { slot } => Reaction::Discard { slot },
        };
    }

    /// Called whenever empathy might have changed
    pub fn check_empathy(&mut self, state: &State) {
        // Update empathy knowledge on cards in hand in case a revealed copy or a change in playstacks had an effect
        for (_, hand) in state.hands.iter() {
            for &card_id in hand {
                let note = self.note_mut(card_id);
                let table = &state.empathy[card_id as usize];
                if table.probability_is_playable(&state.board) == 1.0 {
                    note.play = true
                } else if table.probability_is_dead(&state.board) == 1.0 {
                    note.trash = true
                }
            }
        }
    }

    fn note(&self, card_id: CardId) -> Note {
        self.notes[card_id as usize]
    }

    fn note_mut(&mut self, card_id: CardId) -> &mut Note {
        &mut self.notes[card_id as usize]
    }

    fn categorize_play(&self, state: &State, card_id: CardId) -> ChoiceCategory {
        if self.note(card_id).play {
            ChoiceCategory::ExpectedPlay
        } else if self.note(card_id).known {
            ChoiceCategory::KnownMisplay
        } else {
            ChoiceCategory::UnexpectedDiscard
        }
    }

    fn categorize_discard(&self, state: &State, card_id: CardId) -> ChoiceCategory {
        if self.note(card_id).trash {
            ChoiceCategory::ExpectedDiscard
        } else if self.note(card_id).known {
            ChoiceCategory::KnownDiscard
        } else {
            ChoiceCategory::UnexpectedDiscard
        }
    }

    fn describe_hint(&self, state: &State, hint: &Hint) -> HintDesc {
        let new_obvious_plays: Vec<CardId> = state.hands[hint.receiver]
            .iter()
            .copied()
            .filter(|&card_id| {
                !self.note(card_id).play && state.is_empathy_permanently_playable(card_id)
            })
            .collect();

        let new_known_cards: Vec<CardId> = state.hands[hint.receiver]
            .iter()
            .copied()
            .filter(|&card_id| !self.note(card_id).known && state.is_empathy_known(card_id))
            .collect();

        let new_known_trash: Vec<CardId> = state.hands[hint.receiver]
            .iter()
            .copied()
            .filter(|&card_id| !self.note(card_id).trash && state.is_empathy_trash(card_id))
            .collect();

        HintDesc {
            new_obvious_plays,
            new_known_trash,
            new_known_cards,
            hat_clue: hat_clue(state, hint),
        }
    }

    fn is_loaded(&self, state: &State, player: Player) -> bool {
        state.hands[player]
            .iter()
            .any(|&card_id| self.note(card_id).is_action())
    }

    pub fn notes(&self) -> Vec<String> {
        self.notes.iter().map(Note::to_string).collect()
    }

    /// Returns Ordering::Equal unless one choice is conventionally required over the other
    pub fn compare_conventional_alternatives(
        &self,
        state: &State,
        view: &PlayerView<'_>,
        a: &ChoiceDesc,
        b: &ChoiceDesc,
    ) -> Ordering {
        if state.board.lives_remaining == state.board.opts.num_lives - 1 {
            // If one move avoids striking out, it is better
            match (a.instructed_misplay(view), b.instructed_misplay(view)) {
                (None, Some(_)) => return Ordering::Greater,
                (Some(_), None) => return Ordering::Less,
                (None, None) | (Some(_), Some(_)) => {}
            }
        }

        // If one move results in a less severe discard, it is better.
        // Note that None < Some(_)
        match a.discard_severity(view).cmp(&b.discard_severity(view)) {
            Ordering::Less => return Ordering::Greater,
            Ordering::Equal => {}
            Ordering::Greater => return Ordering::Less,
            // TODO: Is accepting a higher severity discard okay sometimes?
            // Possible future configuration:
            // - Giving ptd to a critical is never a logical alternative
            // - Giving ptd to a 2 is a logical alternative when
            //   - The clue count is "low enough" and the alternatives lock or discard a one-away 3
            // - Giving ptd to an immediately playable is a logical alternative when
            //   - The mainline discards a critical? (probably due to zcsp)
            //   - What about at 2 clues if you're scared of drawing all criticals?
            // - Giving ptd to a 3 is a logical alternative when
            //   - There is a safe action but the clue count is "low enough" not to sieve it in
            //     - Apply sodiumdebt+hallmark's criteria of "am I sieving in the card which will be the best discard?"
            //       (or even 2nd best discard)
            //   - The alternative is locking and the clue count is "low enough" (other metric? score?)
            //   - The alternative is discarding another useful card

            // TODO: How do we take into account the danger of sacrificing?
        }

        // If one sacrifice is less likely to be critical, it is better
        match a
            .risk_of_critical_sacrifice(state)
            .partial_cmp(&b.risk_of_critical_sacrifice(state))
            .unwrap()
        {
            Ordering::Less => return Ordering::Greater,
            Ordering::Equal => {}
            Ordering::Greater => return Ordering::Less,
        }

        // If one move avoids a lock, it is better
        match (a.is_lock(), b.is_lock()) {
            (false, true) => return Ordering::Greater,
            (true, false) => return Ordering::Less,
            (true, true) | (false, false) => {}
        }

        // If one move avoids an intentional strike, it is better
        match (a.instructed_misplay(view), b.instructed_misplay(view)) {
            (None, Some(_)) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (None, None) | (Some(_), Some(_)) => {}
        }

        // Avoid discarding to 8 clues when you could give a new play
        if view.board.hints_remaining == view.board.opts.num_hints - 1 {
            match (a.is_discard(), a.new_plays(), b.is_discard(), b.new_plays()) {
                (true, 0, _, 1..) => return Ordering::Less,
                (_, 1.., true, 0) => return Ordering::Greater,
                _ => {}
            }
        }

        if view.board.pace() < view.board.opts.num_players {
            // Give a play to a player who doesn't play about any
            if !self.is_stacked(state, view.board.player_to_right(view.board.player)) {
                match (a.new_plays(), (b.new_plays())) {
                    (1.., 0) => return Ordering::Greater,
                    (0, 1..) => return Ordering::Less,
                    (0, 0) | (1.., 1..) => {}
                }
            }

            if view.board.pace() == 1 {
                // Prefer playing urgent cards at low pace (see definition of urgent below)
                match (
                    a.is_playing_urgent_card(self, state),
                    b.is_playing_urgent_card(self, state),
                ) {
                    (true, false) => return Ordering::Greater,
                    (false, true) => return Ordering::Less,
                    (true, true) | (false, false) => {}
                }

                // Prefer discarding to playing non-urgent cards when partner already has plays
                if a.is_discard() && b.is_play() {
                    return Ordering::Greater;
                }
                if b.is_discard() && a.is_play() {
                    return Ordering::Less;
                }

                // Prefer cluing to discarding if ... TODO
            }
        }

        // At pace 1, discard instead of playing if
        // - My play might not be a 5 AND
        // - I can save my play for the final round (it's a 3 and partner doesn't have the 5 or it's a 4) AND
        // - My partner has at least two plays
        // TODO

        // At pace 1, clue instead of discarding if
        // - I might not have any useful 4s or 5s and my partner has a play and there are enough clues to stall
        // - My partner's hand is empty and there will be enough clues to stall if they draw a good card
        // - TODO: drawing a 3 into the same hand as its 5
        //
        // Enough clues to stall: clue count > (# non-5s to be played in partner's hand)

        Ordering::Equal

        // TODO: should stalling should be unconventional if there are cluable safe actions?

        // TODO:
        // Ref play clues on unplayables
        // - Forcing a bomb of trash is a logical alternative when
        //   - the clue count is high enough and the alternative is discarding a useful card / locking
        // - Forcing a bomb of a useful card is a logical alternative... never?
        //   - Maybe if the alternative is discarding a critical because there is no possible lock clue
        // - Forcing a bomb of a critical card is a logical alternative... never?
        //
        // - Sacrificing a card is a logical alternative when
        //   - idk...
        //
        // Giving a ref discard instead of a ref play
        // - When giving elim for a playable?
        // - When it's a good line? (always logical alternative?)
        // - Not when the clue count is very high (e.g. 7)?
        //
        // Giving a ref play instead of a clue which gets multiple safe actions
        // - When the line is good?
        // - What about first turn "always clue 1s" policies?
        // - When the mainline bad touches?
    }

    /// Returns true if `player` knows about a play
    fn is_stacked(&self, state: &State, player: Player) -> bool {
        state.hands[player]
            .iter()
            .any(|&card_id| self.note(card_id).is_playable())
    }

    fn is_urgent_card(&self, state: &State, card: Card) -> bool {
        if card.value == 5 {
            return true;
        }
        let missing_cards_in_stack = state.board.highest_attainable(card.color) - card.value;
        // (This is 2p-specific)
        match missing_cards_in_stack {
            0 | 1 => false,
            // Playing a 3 is urgent unless we know we have the matching 5
            2 => state.hands[state.board.player].iter().any(|&card_id| {
                state.empathy[card_id as usize].probability_of_predicate(&|own_card| {
                    own_card == Card::new(card.color, card.value + 2)
                }) == 1.0
            }),
            3.. => true,
        }
    }
}

fn hat_clue(state: &State, hint: &Hint) -> QueuedHatClue {
    let touches_newest = hint
        .touched
        .contains(state.hands[hint.receiver].last().unwrap());
    let hint_value = match (hint.hinted, touches_newest) {
        (Hinted::Value(_), true) => 1,
        (Hinted::Value(_), false) => 2,
        (Hinted::Color(_), false) => 3,
        (Hinted::Color(_), true) => 4,
    };
    let num_players_away = (state.board.opts.num_players + hint.receiver - state.board.player)
        % state.board.opts.num_players;
    let last_responder = state.board.player_to_left(state.board.player);
    if state.board.opts.num_players == 5 && hint.receiver == last_responder {
        // "Emily clue"
        QueuedHatClue {
            remaining_slot_sum: 5,
            remaining_plays: hint_value % 3,
            last_responder,
        }
    } else {
        QueuedHatClue {
            remaining_slot_sum: hint_value,
            remaining_plays: num_players_away as u8,
            last_responder,
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct Note {
    play: bool,
    trash: bool,
    known: bool,
}

impl Note {
    fn is_action(&self) -> bool {
        self.play || self.trash
    }

    fn is_playable(&self) -> bool {
        // A card can be playable and later become trash
        self.play && !self.trash
    }
}

impl Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut need_pipe = false;
        if self.play {
            f.write_str("f")?;
            need_pipe = true;
        }
        if self.trash {
            if need_pipe {
                f.write_str(" | ")?;
            }
            f.write_str("kt")?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ChoiceDesc {
    pub category: ChoiceCategory,
}

#[derive(Clone)]
pub enum ChoiceCategory {
    /// A discard that was publicly known to be safe (instructed/obvious)
    ExpectedPlay,
    /// A discard that was publicly known to be safe (instructed/obvious)
    ExpectedDiscard,
    /// A play of a fully-known unplayable card
    KnownMisplay,
    /// A discard a fully-known non-trash (useful/playable) card
    KnownDiscard,
    /// A play of a card that was not publicly instructed prior
    ///
    /// Either a response to a hat clue or a player with final freedom demonstrating some private knowledge
    UnexpectedPlay {
        slot: u8,
    },
    /// A discard of a card that was not publicly instructed prior
    ///
    /// Either a response to a hat clue or a player with final freedom demonstrating some private knowledge
    UnexpectedDiscard {
        slot: u8,
    },
    Hint(HintDesc),
}

enum Reaction {
    Ignore,
    Play { slot: u8 },
    Discard { slot: u8 },
}

#[derive(Debug, Clone)]
pub struct HintDesc {
    new_obvious_plays: Vec<CardId>,
    new_known_trash: Vec<CardId>,
    new_known_cards: Vec<CardId>,
    hat_clue: QueuedHatClue,
}

impl ChoiceDesc {
    fn discard_severity(&self, view: &PlayerView<'_>) -> DiscardSeverity {
        match (self.gave_ptd, self.instructed_misplay(view)) {
            (None, None) => DiscardSeverity::Safe,
            (None, Some(card_id)) | (Some(card_id), None) => discard_severity(view, card_id),
            (Some(_), Some(_)) => {
                panic!("how did move cause misplay and give ptd at the same time?")
            }
        }
    }

    fn instructed_misplay(&self, view: &PlayerView<'_>) -> Option<CardId> {
        match self.category {
            ChoiceCategory::Hint(HintDesc {
                new_obvious_plays: _,
                new_known_trash: _,
                category,
            }) => match category {
                HintCategory::RefPlay(target) => {
                    let card = view.card(target);
                    if !view.board.is_playable(card) {
                        Some(target)
                    } else {
                        None
                    }
                }
                HintCategory::RefDiscard(_)
                | HintCategory::FillIn
                | HintCategory::RankAction
                | HintCategory::LockedHandStall
                | HintCategory::EightClueStall
                | HintCategory::LoadedRankStall
                | HintCategory::Lock(_) => None,
            },
            ChoiceCategory::ExpectedPlay(_)
            | ChoiceCategory::ExpectedDiscard
            | ChoiceCategory::Sacrifice(_) => None,
        }
    }

    fn risk_of_critical_sacrifice(&self, state: &State) -> f32 {
        match self.category {
            ChoiceCategory::Sacrifice(card_id) => {
                // TODO: private empathy (e.g. using deductions from seen criticals in partner's hand)
                1.0 - state.empathy[card_id as usize].probability_is_dispensable(&state.board)
            }
            ChoiceCategory::ExpectedPlay(_)
            | ChoiceCategory::ExpectedDiscard
            | ChoiceCategory::Hint(_) => 0.0,
        }
    }

    fn is_lock(&self) -> bool {
        match self.category {
            ChoiceCategory::Hint(HintDesc {
                new_obvious_plays: _,
                new_known_trash: _,
                category,
            }) => match category {
                HintCategory::Lock(_) => true,
                HintCategory::RefPlay(_)
                | HintCategory::RefDiscard(_)
                | HintCategory::FillIn
                | HintCategory::RankAction
                | HintCategory::LockedHandStall
                | HintCategory::EightClueStall
                | HintCategory::LoadedRankStall => false,
            },
            ChoiceCategory::ExpectedPlay(_)
            | ChoiceCategory::ExpectedDiscard
            | ChoiceCategory::Sacrifice(_) => false,
        }
    }

    fn is_discard(&self) -> bool {
        match self.category {
            ChoiceCategory::ExpectedDiscard | ChoiceCategory::Sacrifice(_) => true,
            ChoiceCategory::ExpectedPlay(_) | ChoiceCategory::Hint(_) => false,
        }
    }

    fn new_plays(&self) -> u32 {
        match &self.category {
            ChoiceCategory::Hint(hint) => hint.new_plays(),
            ChoiceCategory::ExpectedPlay(_)
            | ChoiceCategory::ExpectedDiscard
            | ChoiceCategory::Sacrifice(_) => 0,
        }
    }

    /// Returns true if this move is a play and the card it is playing should not be saved for the final round
    ///
    /// If this returns true, it's either playing a 1, a 2, a 3 whose 5 is visible, or a 5
    fn is_playing_urgent_card(&self, knowledge: &PublicKnowledge, state: &State) -> bool {
        match self.category {
            // TODO: consider local empathy
            ChoiceCategory::ExpectedPlay(card_id) => state.empathy[card_id as usize]
                .get_possibilities()
                .iter()
                .all(|&card| {
                    !state.board.is_playable(card) || knowledge.is_urgent_card(state, card)
                }),
            ChoiceCategory::ExpectedDiscard
            | ChoiceCategory::Sacrifice(_)
            | ChoiceCategory::Hint(_) => false,
        }
    }

    fn is_play(&self) -> bool {
        match self.category {
            ChoiceCategory::ExpectedPlay(_) => true,
            ChoiceCategory::ExpectedDiscard
            | ChoiceCategory::Sacrifice(_)
            | ChoiceCategory::Hint(_) => false,
        }
    }
}

impl HintCategory {
    fn new_plays(&self) -> u32 {
        match self {
            HintCategory::RefPlay(_) => 1,
            HintCategory::RefDiscard(_)
            | HintCategory::FillIn
            | HintCategory::EightClueStall
            | HintCategory::RankAction
            | HintCategory::Lock(_)
            | HintCategory::LockedHandStall
            | HintCategory::LoadedRankStall => 0,
        }
    }
}

impl HintDesc {
    pub fn new_plays(&self) -> u32 {
        self.new_obvious_plays.len() as u32 + self.category.new_plays()
    }
}
// Lowest to highest severity
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum DiscardSeverity {
    Safe,
    Two,
    Playable,
    Critical,
}

fn discard_severity(view: &PlayerView<'_>, card_id: CardId) -> DiscardSeverity {
    let card = view.card(card_id);
    if !view.board.is_dispensable(card) {
        DiscardSeverity::Critical
    } else if view.board.is_dead(card) || is_duplicate(view, card_id) {
        DiscardSeverity::Safe
    } else if view.board.is_playable(card) {
        DiscardSeverity::Playable
    } else if card.value == 2 {
        DiscardSeverity::Two
    } else {
        DiscardSeverity::Safe
    }
}

fn is_duplicate(view: &PlayerView<'_>, card_id: CardId) -> bool {
    let card = view.card(card_id);
    view.other_players().any(|player| {
        view.hand(player)
            .pairs()
            .any(|(visible_card_id, visible_card)| {
                visible_card_id != card_id && visible_card == card
            })
    })
}
