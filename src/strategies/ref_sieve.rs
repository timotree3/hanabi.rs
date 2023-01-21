use std::cmp::Ordering;

use crate::{
    game::{
        BoardState, Card, CardCounts, CardId, GameOptions, Hand, Hint as HintChoice, Hinted,
        Player, PlayerView, TurnChoice, TurnRecord, TurnResult, COLORS, TOTAL_CARDS, VALUES,
    },
    helpers::{CardInfo, CardPossibilityTable, PerPlayer},
    strategy::{GameStrategy, GameStrategyConfig, PlayerStrategy},
};

#[derive(Clone)]
pub struct Config;

impl GameStrategyConfig for Config {
    fn initialize(&self, _: &GameOptions) -> Box<dyn GameStrategy> {
        Box::new(Strategy {})
    }
}

pub struct Strategy {}

impl GameStrategy for Strategy {
    fn initialize<'game>(
        &self,
        view: &PlayerView<'game>,
    ) -> Box<dyn PlayerStrategy<'game> + 'game> {
        Box::new(RsPlayer {
            public: Public::first_turn(view),
        })
    }
}

// How do we deal with plays that are stacked on top of plays in giver's hand?
// Simple options:
// - Never do it
// - Superposition includes duplicate of first card in each suit played from giver's hand,
//   as well as cards on top of each card played from giver's hand at time of clue before the card had permission to play
// Complicated option: Pay attention to which plays are publicly known are for the non-public ones, consider what it would take from the hand to make them known
//   - For each card, keep track of its useful-unplayable identities

struct Public<'game> {
    notes: Vec<Note>,
    empathy: Vec<CardPossibilityTable>,
    card_counts: CardCounts, // what any newly drawn card should be
    hands: PerPlayer<Hand>,
    board: BoardState<'game>,
}

#[derive(Debug, Copy, Clone, Default)]
struct Note {
    clued: bool,
    play: bool,
    trash: bool,
    /// Has this card ever been given "permission to discard"?
    ptd: bool,
}
impl Note {
    fn is_action(&self) -> bool {
        self.play || self.trash || self.ptd
    }

    fn unclued(&self) -> bool {
        !self.clued && !self.play && !self.trash
    }
}

impl<'game> Public<'game> {
    fn first_turn(view: &PlayerView<'game>) -> Public<'game> {
        Public {
            notes: vec![Note::default(); TOTAL_CARDS as usize],
            empathy: vec![
                CardPossibilityTable::new();
                (view.board.opts.num_players * view.board.opts.hand_size) as usize
            ],
            card_counts: CardCounts::new(),
            hands: view.hands().clone(),
            board: view.board.clone(),
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

    fn describe_choice(&self, choice: &Choice) -> Option<ChoiceDesc> {
        match choice {
            Choice::Play(card_id) => self.describe_play(*card_id).map(ChoiceDesc::Action),
            Choice::Discard(card_id) => self.describe_discard(*card_id).map(ChoiceDesc::Action),
            Choice::Hint(hint) => self.describe_hint(hint).map(ChoiceDesc::Hint),
        }
    }

    fn describe_play(&self, card_id: CardId) -> Option<ActionDesc> {
        if !self.note(card_id).play {
            return None;
        }
        // If the next player is not loaded, give them PTD
        let next_player = self.board.player_to_right(self.board.player);
        // TODO: What if this play was known to give them an action
        Some(ActionDesc {
            gave_ptd: self.chop_if_unloaded(next_player),
        })
    }

    fn describe_discard(&self, card_id: CardId) -> Option<ActionDesc> {
        if !self.note(card_id).trash && !self.note(card_id).ptd {
            return None;
        }
        // If the next player is not loaded, give them PTD
        let next_player = self.board.player_to_right(self.board.player);
        Some(ActionDesc {
            gave_ptd: self.chop_if_unloaded(next_player),
        })
    }

    fn describe_hint(&self, hint: &Hint) -> Option<HintDesc> {
        let new_known_plays: Vec<CardId> = self.hands[hint.receiver]
            .iter()
            .copied()
            .filter(|&card_id| !self.note(card_id).play && self.is_empathy_playable(card_id))
            .collect();

        let new_known_trash: Vec<CardId> = self.hands[hint.receiver]
            .iter()
            .copied()
            .filter(|&card_id| !self.note(card_id).trash && self.is_empathy_trash(card_id))
            .collect();

        self.categorize_hint(hint, &new_known_plays, &new_known_trash)
            .map(|category| HintDesc {
                new_known_plays,
                new_known_trash,
                category,
            })
    }

    fn categorize_hint(
        &self,
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
            if let Some(target) = self.color_clue_target(hint.receiver, &hint.touched) {
                return Some(HintCategory::RefPlay(target));
            }
        }

        if self.board.hints_remaining == self.board.opts.num_hints {
            Some(HintCategory::EightClueStall)
        } else {
            None
        }
    }

    fn interpret_hint(&mut self, hint: &Hint) {
        let HintDesc {
            new_known_plays,
            new_known_trash,
            category,
        } = self.describe_hint(hint).expect("unconventional hint given");

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
            HintCategory::EightClueStall | HintCategory::FillIn => {}
        }

        for &card_id in &hint.touched {
            self.note_mut(card_id).clued = true;
        }
    }

    fn color_clue_target(&self, player: Player, touched: &[CardId]) -> Option<CardId> {
        let previously_unclued: Vec<CardId> = self.hands[player]
            .iter()
            .copied()
            .filter(|&card_id| self.unclued(card_id))
            .collect();

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

    fn chop_if_unloaded(&self, player: Player) -> Option<CardId> {
        (!self.is_loaded(player)).then(|| *self.hands[player].last().unwrap())
    }

    fn interpret_play(&mut self, card_id: CardId) {
        let ActionDesc { gave_ptd } = self.describe_play(card_id).expect("unconventional play");

        if let Some(chop) = gave_ptd {
            self.note_mut(chop).ptd = true;
        }
    }

    fn interpret_discard(&mut self, card_id: CardId) {
        let ActionDesc { gave_ptd } = self
            .describe_discard(card_id)
            .expect("unconventional discard");

        if let Some(chop) = gave_ptd {
            self.note_mut(chop).ptd = true;
        }
    }

    fn is_loaded(&self, player: Player) -> bool {
        self.hands[player]
            .iter()
            .any(|&card_id| self.note(card_id).is_action())
    }

    fn reveal_copy(&mut self, card: Card, card_id: CardId) {
        self.empathy
            .iter_mut()
            .enumerate()
            .for_each(|(other_card_id, table)| {
                if other_card_id as CardId != card_id {
                    table.decrement_weight_if_possible(card)
                }
            })
    }

    // Update internal state to reflect that a card has been drawn
    fn draw_card(&mut self) {
        self.empathy
            .push(CardPossibilityTable::from(&self.card_counts));
    }

    fn update_board(&mut self, view: &PlayerView<'game>) {
        if view.board.deck_size != self.board.deck_size {
            assert!(view.board.deck_size == self.board.deck_size - 1);
            self.draw_card();
        }
        self.hands.clone_from(view.hands());
        self.board.clone_from(&view.board);

        // Update empathy knowledge in case a revealed copy or a change in playstacks had an effect
        for (note, table) in self.notes.iter_mut().zip(&self.empathy) {
            if table.probability_is_playable(&self.board) == 1.0 {
                note.play = true
            } else if table.probability_is_dead(&self.board) == 1.0 {
                note.trash = true
            }
        }
    }

    fn update_empathy_for_hint(&mut self, hint: &Hint) {
        // The touched cards are in the same order as the hand.
        // Iterate over the two at the same time to efficiently determine that cards are not touched.
        let mut hand_iter = self.hands[hint.receiver].iter().copied();
        for &touched_card_id in &hint.touched {
            for card_id in &mut hand_iter {
                let is_touched = card_id == touched_card_id;
                self.empathy[card_id as usize].mark_hinted(hint.hinted, is_touched);
                if is_touched {
                    break;
                }
            }
        }
    }

    fn is_empathy_playable(&self, card_id: CardId) -> bool {
        // TODO: Use delayed definition of playable
        self.empathy[card_id as usize].probability_is_playable(&self.board) == 1.0
    }

    fn is_empathy_trash(&self, card_id: CardId) -> bool {
        // TODO: Use definition of trash that includes duplicates
        self.empathy[card_id as usize].probability_is_dead(&self.board) == 1.0
    }
}

enum Choice {
    Play(CardId),
    Discard(CardId),
    Hint(Hint),
}

enum ChoiceDesc {
    /// A play or a discard
    Action(ActionDesc),
    Hint(HintDesc),
}

struct ActionDesc {
    gave_ptd: Option<CardId>,
}

struct Hint {
    receiver: Player,
    hinted: Hinted,
    touched: Vec<CardId>,
}
impl Hint {
    fn choice(&self) -> HintChoice {
        HintChoice {
            player: self.receiver,
            hinted: self.hinted,
        }
    }
}

#[derive(Debug)]
struct HintDesc {
    new_known_plays: Vec<CardId>,
    new_known_trash: Vec<CardId>,
    category: HintCategory,
}

#[derive(Debug, Clone, Copy)]
enum HintCategory {
    RefPlay(CardId),
    FillIn,
    EightClueStall,
}

impl HintCategory {
    fn new_plays(&self) -> usize {
        match self {
            HintCategory::RefPlay(_) => 1,
            HintCategory::FillIn | HintCategory::EightClueStall => 0,
        }
    }
}

impl HintDesc {
    fn new_plays(&self) -> usize {
        self.new_known_plays.len() + self.category.new_plays()
    }
}

struct RsPlayer<'game> {
    /// The public knowledge shared amongst the players
    public: Public<'game>,
}

impl RsPlayer<'_> {
    fn describe_choice(
        &mut self,
        choice: &Choice,
        backup_empathy: &[CardPossibilityTable],
    ) -> Option<ChoiceDesc> {
        if let Choice::Hint(hint) = choice {
            // Hack: Update the empathy as it would be after the hint was given
            self.public.update_empathy_for_hint(hint);
        }
        let desc = self.public.describe_choice(choice);
        if let Choice::Hint(_) = choice {
            // Restore the empathy
            self.public.empathy.clone_from_slice(backup_empathy);
        }
        desc
    }

    /// Chooses a preferred move in the position.
    ///
    /// Mutates self in place for efficiency but should leave it unchanged upon exiting.

    fn choose(&mut self, view: &PlayerView<'_>) -> Choice {
        let backup_empathy = self.public.empathy.clone();
        let (choice, _) = possible_choices(view)
            .filter_map(|choice| {
                self.describe_choice(&choice, &backup_empathy)
                    .map(|desc| (choice, desc))
            })
            .filter(|(_, choice_desc)| is_conventional(view, choice_desc))
            .max_by(|a, b| compare_choice(view, a, b))
            .expect("there should be at least one conventional option");
        choice
    }
}

fn possible_choices<'a>(view: &'a PlayerView<'_>) -> impl Iterator<Item = Choice> + 'a {
    let my_hand = view.hands()[view.me()].iter().copied();
    let plays = my_hand.clone().map(Choice::Play);
    let mut discards = my_hand.map(Choice::Discard);
    let mut hints = possible_hints(view).map(Choice::Hint);
    match view.board.hints_remaining {
        // Hinting is impossible with 0 left
        0 => hints.by_ref().for_each(drop),
        // Discarding is impossible at max hint count
        n if n == view.board.opts.num_hints => discards.by_ref().for_each(drop),
        _ => {}
    }
    plays.chain(discards).chain(hints)
}

fn is_conventional(view: &PlayerView<'_>, desc: &ChoiceDesc) -> bool {
    match desc {
        ChoiceDesc::Action(ActionDesc {
            gave_ptd: Some(chop),
        }) => {
            let card = view.card(*chop);
            // We don't give PTD to criticals or playables
            view.board.is_dispensable(card) && !view.board.is_playable(card)
        }
        ChoiceDesc::Action(ActionDesc { gave_ptd: None }) => true,
        ChoiceDesc::Hint(HintDesc {
            new_known_plays: _,
            new_known_trash: _,
            category,
        }) => is_hint_conventional(view, *category),
    }
}

fn compare_choice(
    _view: &PlayerView<'_>,
    a: &(Choice, ChoiceDesc),
    b: &(Choice, ChoiceDesc),
) -> Ordering {
    match (a, b) {
        (
            (Choice::Hint(_), ChoiceDesc::Hint(hint_a)),
            (Choice::Hint(_), ChoiceDesc::Hint(hint_b)),
        ) => hint_a.new_plays().cmp(&hint_b.new_plays()),
        ((Choice::Hint(_), _), _) => Ordering::Greater,
        (_, (Choice::Hint(_), _)) => Ordering::Less,
        ((Choice::Play(_), _), (Choice::Play(_), _)) => Ordering::Equal,
        ((Choice::Play(_), _), (_, _)) => Ordering::Greater,
        ((_, _), (Choice::Play(_), _)) => Ordering::Less,
        ((Choice::Discard(_), _), (Choice::Discard(_), _)) => Ordering::Equal,
    }
}

fn is_hint_conventional(view: &PlayerView<'_>, hint_category: HintCategory) -> bool {
    match hint_category {
        HintCategory::RefPlay(target) => view.board.is_playable(view.card(target)),
        HintCategory::FillIn => true,
        HintCategory::EightClueStall => true,
    }
}

fn possible_hints<'a>(view: &'a PlayerView<'_>) -> impl Iterator<Item = Hint> + 'a {
    view.other_players()
        .flat_map(|receiver| possible_hints_to(view, receiver))
}
fn possible_hints_to<'a>(
    view: &'a PlayerView<'_>,
    receiver: Player,
) -> impl Iterator<Item = Hint> + 'a {
    let color_hints = COLORS.iter().copied().map(move |color| {
        let touched = view
            .hand(receiver)
            .pairs()
            .filter(|(_, card)| card.color == color)
            .map(|(card_id, _)| card_id)
            .collect::<Vec<_>>();
        Hint {
            receiver,
            hinted: Hinted::Color(color),
            touched,
        }
    });
    let value_hints = VALUES.iter().copied().map(move |value| {
        let touched = view
            .hand(receiver)
            .pairs()
            .filter(|(_, card)| card.value == value)
            .map(|(card_id, _)| card_id)
            .collect::<Vec<_>>();
        Hint {
            receiver,
            hinted: Hinted::Value(value),
            touched,
        }
    });

    color_hints
        .chain(value_hints)
        .filter(|hint| !hint.touched.is_empty())
}

fn touched_ids<'a>(
    player: u32,
    touched: &'a [bool],
    hands: &'a PerPlayer<Hand>,
) -> impl Iterator<Item = CardId> + 'a {
    hands[player]
        .iter()
        .copied()
        .enumerate()
        .filter(|&(index, _)| touched[index])
        .map(|(_, card_id)| card_id)
}

impl<'game> PlayerStrategy<'game> for RsPlayer<'game> {
    fn name(&self) -> String {
        "rs".to_owned()
    }

    fn decide(&mut self, view: &PlayerView<'_>) -> TurnChoice {
        let choice = self.choose(view);
        let card_id_to_index = |card_id| {
            self.public.hands[view.me()]
                .iter()
                .position(|&id| id == card_id)
        };

        match choice {
            Choice::Play(card_id) => TurnChoice::Play(
                card_id_to_index(card_id).expect("chose to play a card which was not held"),
            ),
            Choice::Discard(card_id) => TurnChoice::Discard(
                card_id_to_index(card_id).expect("chose to play a card which was not held"),
            ),
            Choice::Hint(hint) => TurnChoice::Hint(hint.choice()),
        }
    }

    fn update(&mut self, turn_record: &TurnRecord, view: &PlayerView<'game>) {
        match (turn_record.choice, &turn_record.result) {
            (TurnChoice::Hint(HintChoice { player, hinted }), TurnResult::Hint(touched)) => {
                let touched_ids: Vec<CardId> = touched_ids(player, touched, view.hands()).collect();
                let hint = Hint {
                    receiver: player,
                    hinted,
                    touched: touched_ids,
                };
                self.public.update_empathy_for_hint(&hint);
                self.public.interpret_hint(&hint);
            }
            (TurnChoice::Discard(index), TurnResult::Discard(card)) => {
                let card_id = self.public.hands[self.public.board.player][index];
                self.public.interpret_discard(card_id);
                self.public.reveal_copy(*card, card_id);
            }
            (TurnChoice::Play(index), TurnResult::Play(card, _)) => {
                let card_id = self.public.hands[self.public.board.player][index];
                self.public.interpret_play(card_id);
                self.public.reveal_copy(*card, card_id);
            }
            _ => panic!("mismatched turn choice and turn result"),
        }

        self.public.update_board(view);
    }
}
