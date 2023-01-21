use std::{cmp::Ordering, fmt::Display};

use crate::{
    game::{Card, CardId, Hinted, Player, PlayerView, TOTAL_CARDS},
    helpers::CardInfo,
};

use super::{Choice, Hint, State};

pub(super) struct PublicKnowledge {
    notes: Vec<Note>,
}

impl PublicKnowledge {
    pub fn first_turn() -> Self {
        Self {
            notes: vec![Note::default(); TOTAL_CARDS as usize],
        }
    }

    pub fn describe_choice(&self, state: &State, choice: &Choice) -> Option<ChoiceDesc> {
        match choice {
            Choice::Play(card_id) => self.describe_play(state, *card_id),
            Choice::Discard(card_id) => self.describe_discard(state, *card_id),
            Choice::Hint(hint) => self.describe_hint(state, hint),
        }
    }

    pub fn interpret_choice(&mut self, state: &State, choice: &Choice) {
        let ChoiceDesc { gave_ptd, category } = self
            .describe_choice(state, choice)
            .expect("action taken should be conventional");
        if let Some(chop) = gave_ptd {
            self.note_mut(chop).ptd = true;
        }
        match category {
            ChoiceCategory::ExpectedPlay(_)
            | ChoiceCategory::ExpectedDiscard
            | ChoiceCategory::Sacrifice(_) => {}

            ChoiceCategory::Hint(HintDesc {
                new_known_plays,
                new_known_trash,
                category,
            }) => {
                for card_id in new_known_plays {
                    self.note_mut(card_id).play = true;
                }

                for card_id in new_known_trash {
                    self.note_mut(card_id).trash = true;
                }

                match category {
                    HintCategory::RefPlay(target) => {
                        self.note_mut(target).play = true;
                    }
                    HintCategory::RefDiscard(_) => {
                        // The target already has been given PTD due to the `gave_ptd` field
                    }
                    HintCategory::Lock(player) => {
                        let newest = *state.hands[player].last().unwrap();
                        self.note_mut(newest).lock = true;
                    }
                    HintCategory::EightClueStall
                    | HintCategory::FillIn
                    | HintCategory::RankAction
                    | HintCategory::LockedHandStall => {}
                    HintCategory::LoadedRankStall => {}
                }
            }
        }

        if let Choice::Hint(hint) = choice {
            for &card_id in &hint.touched {
                self.note_mut(card_id).clued = true;
            }
        }
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

    fn unclued(&self, card_id: CardId) -> bool {
        self.note(card_id).unclued()
    }

    fn describe_play(&self, state: &State, card_id: CardId) -> Option<ChoiceDesc> {
        if !self.note(card_id).is_playable() {
            return None;
        }
        // If the next player is not loaded, give them PTD
        let next_player = state.board.player_to_right(state.board.player);
        // TODO: What if this play was known to give them an action
        Some(ChoiceDesc {
            gave_ptd: self.chop_if_unloaded(state, next_player),
            category: ChoiceCategory::ExpectedPlay(card_id),
        })
    }

    fn describe_discard(&self, state: &State, card_id: CardId) -> Option<ChoiceDesc> {
        let category = if self.is_locked(state, state.board.player) {
            ChoiceCategory::Sacrifice(card_id)
        } else if self.note(card_id).trash || self.note(card_id).ptd {
            ChoiceCategory::ExpectedDiscard
        } else {
            return None;
        };
        // If the next player is not loaded, give them PTD
        let next_player = state.board.player_to_right(state.board.player);
        Some(ChoiceDesc {
            gave_ptd: self.chop_if_unloaded(state, next_player),
            category,
        })
    }

    fn describe_hint(&self, state: &State, hint: &Hint) -> Option<ChoiceDesc> {
        let new_known_plays: Vec<CardId> = state.hands[hint.receiver]
            .iter()
            .copied()
            .filter(|&card_id| !self.note(card_id).play && state.is_empathy_playable(card_id))
            .collect();

        let new_known_trash: Vec<CardId> = state.hands[hint.receiver]
            .iter()
            .copied()
            .filter(|&card_id| !self.note(card_id).trash && state.is_empathy_trash(card_id))
            .collect();

        self.categorize_hint(state, hint, &new_known_plays, &new_known_trash)
            .map(|category| ChoiceDesc {
                gave_ptd: match category {
                    HintCategory::RefDiscard(target) => Some(target),
                    HintCategory::LockedHandStall | HintCategory::EightClueStall => {
                        Some(*state.hands[hint.receiver].last().unwrap())
                    }
                    HintCategory::RefPlay(_)
                    | HintCategory::FillIn
                    | HintCategory::RankAction
                    | HintCategory::Lock(_)
                    | HintCategory::LoadedRankStall => None,
                },
                category: ChoiceCategory::Hint(HintDesc {
                    new_known_plays,
                    new_known_trash,
                    category,
                }),
            })
    }

    fn categorize_hint(
        &self,
        state: &State,
        hint: &Hint,
        new_known_plays: &[CardId],
        new_known_trash: &[CardId],
    ) -> Option<HintCategory> {
        let is_fill_in = new_known_plays
            .iter()
            .chain(new_known_trash)
            .any(|&card_id| self.note(card_id).clued && hint.touched.contains(&card_id));

        if is_fill_in {
            return Some(HintCategory::FillIn);
        }

        if let Hinted::Color(_) = hint.hinted {
            if let Some(target) = self.color_clue_target(state, hint.receiver, &hint.touched) {
                return Some(HintCategory::RefPlay(target));
            }
        } else {
            // A rank clue giving a known play or fixing (with negative info) a bad touched card
            // does not mean anything extra
            let fixed_trash = new_known_trash
                .iter()
                .any(|&card_id| self.note(card_id).clued);
            if !new_known_plays.is_empty() || fixed_trash {
                return Some(HintCategory::RankAction);
            }

            if let Some(chop) = self.chop_if_unloaded(state, hint.receiver) {
                if let Some(target) = self.rank_clue_target(state, hint.receiver, &hint.touched) {
                    // check if target = chop. if so, it's a lock/8 clue stall
                    if target == chop {
                        return if hint.touched.contains(&chop) {
                            Some(HintCategory::Lock(hint.receiver))
                        } else if state.board.hints_remaining == state.board.opts.num_hints {
                            Some(HintCategory::EightClueStall)
                        } else if self.is_locked(state, state.board.player) {
                            Some(HintCategory::LockedHandStall)
                        } else {
                            Some(HintCategory::Lock(hint.receiver))
                        };
                    } else {
                        return Some(HintCategory::RefDiscard(target));
                    }
                }
            } else if state.board.hints_remaining == state.board.opts.num_hints {
                return Some(HintCategory::LoadedRankStall);
            } else if self.is_locked(state, state.board.player) {
                // TODO: implementt color stalls and unlock promise
                return Some(HintCategory::LockedHandStall);
            } else {
                // TODO: LPC
                return None;
            }
        }

        None
    }

    fn color_clue_target(
        &self,
        state: &State,
        receiver: Player,
        touched: &[CardId],
    ) -> Option<CardId> {
        let previously_unclued = self.previously_unclued(state, receiver);

        for precedence in 0..previously_unclued.len() {
            let focus = previously_unclued
                [previously_unclued.len() - ((precedence + 1) % previously_unclued.len()) - 1];
            let target = previously_unclued[previously_unclued.len() - precedence - 1];

            if touched.contains(&focus) {
                return Some(target);
            }
        }

        None
    }

    fn rank_clue_target(
        &self,
        state: &State,
        receiver: Player,
        touched: &[CardId],
    ) -> Option<CardId> {
        let previously_unclued: Vec<CardId> = self.previously_unclued(state, receiver);

        for precedence in 0..previously_unclued.len() {
            let focus = previously_unclued[previously_unclued.len() - precedence - 1];
            let target = previously_unclued
                [previously_unclued.len() - ((precedence + 1) % previously_unclued.len()) - 1];

            // The only time the target can be newly clued is if it's a lock
            let is_lock = precedence == previously_unclued.len() - 1;
            if touched.contains(&focus) && (is_lock || !touched.contains(&target)) {
                return Some(target);
            }
        }

        None
    }

    fn previously_unclued(&self, state: &State, receiver: Player) -> Vec<CardId> {
        state.hands[receiver]
            .iter()
            .copied()
            .filter(|&card_id| self.unclued(card_id))
            .collect()
    }

    fn chop_if_unloaded(&self, state: &State, player: Player) -> Option<CardId> {
        (!self.is_loaded(state, player) && !self.is_locked(state, player))
            .then(|| *state.hands[player].last().unwrap())
    }

    fn is_loaded(&self, state: &State, player: Player) -> bool {
        state.hands[player]
            .iter()
            .any(|&card_id| self.note(card_id).is_action())
    }

    fn is_locked(&self, state: &State, player: Player) -> bool {
        let newest = *state.hands[player].last().unwrap();
        !self.is_loaded(state, player) && self.note(newest).lock
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

#[derive(Debug, Copy, Clone, Default)]
struct Note {
    clued: bool,
    play: bool,
    trash: bool,
    /// Has this card ever been given "permission to discard"?
    ptd: bool,
    /// If this card is on newest, and the player has no known safe action, then they are locked
    lock: bool,
}

impl Note {
    fn is_action(&self) -> bool {
        self.play || self.trash || self.ptd
    }

    fn unclued(&self) -> bool {
        !self.clued && !self.play && !self.trash
    }

    fn is_playable(&self) -> bool {
        // A card can be playable and later become trash
        self.play && !self.trash
    }
}

impl Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut need_pipe = false;
        if self.lock {
            f.write_str("lock ---->")?;
            need_pipe = true;
        }
        if self.ptd {
            if need_pipe {
                f.write_str(" | ")?;
            }
            f.write_str("ptd")?;
            need_pipe = true;
        }
        if self.play {
            if need_pipe {
                f.write_str(" | ")?;
            }
            f.write_str(if self.clued { "play" } else { "f" })?;
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
    pub gave_ptd: Option<CardId>,
    pub category: ChoiceCategory,
}

#[derive(Clone)]
pub enum ChoiceCategory {
    /// A play that was publicy known to be safe (instructed/good touch)
    ExpectedPlay(CardId),
    /// A discard that was publicy known to be safe (trash/ptd)
    ExpectedDiscard,
    Sacrifice(CardId),
    Hint(HintDesc),
}

#[derive(Debug, Clone)]
pub struct HintDesc {
    new_known_plays: Vec<CardId>,
    new_known_trash: Vec<CardId>,
    category: HintCategory,
}

#[derive(Debug, Clone, Copy)]
enum HintCategory {
    RefPlay(CardId),
    RefDiscard(CardId),
    /// A clue touching new cards means nothing extra if it touches an already clued card and fills it in
    FillIn,
    /// A rank clue touching new cards means nothing extra if it gives a previously unknown play
    RankAction,
    LockedHandStall,
    EightClueStall,
    /// In a stalling situation, a rank clue to a loaded player means nothing extra and does not give PTD
    /// (Exception: locked hand stalls in 2p)
    LoadedRankStall,
    Lock(Player),
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
                new_known_plays: _,
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
                new_known_plays: _,
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
        self.new_known_plays.len() as u32 + self.category.new_plays()
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
