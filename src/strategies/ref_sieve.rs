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
        let note = self.note(card_id);
        !note.clued && !note.play
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
        self.hands = view.hands().clone();
        self.board = view.board.clone();

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
    /// Determines the best hint available.
    ///
    /// Mutates self in place for efficiency but should leave it unchanged upon exiting.
    fn best_hint(&mut self, view: &PlayerView<'_>) -> Option<HintChoice> {
        let mut best: Option<(Hint, HintDesc)> = None;

        let actual_empathy = self.public.empathy.clone();
        for hint in possible_hints(view) {
            // Update the empathy as it would be after the hint was given
            self.public.update_empathy_for_hint(&hint);

            if let Some(desc) = self.public.describe_hint(&hint) {
                if is_hint_conventional(view, desc.category) {
                    best = Some(if let Some((best_hint, best_desc)) = best {
                        if desc.new_plays() > best_desc.new_plays() {
                            (hint, desc)
                        } else {
                            (best_hint, best_desc)
                        }
                    } else {
                        (hint, desc)
                    });
                }
            }

            // Restore the empathy
            self.public.empathy.clone_from(&actual_empathy);
        }

        best.map(|(hint, _)| hint.choice())
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
        let my_play = self.public.hands[view.me()]
            .iter()
            .position(|&card_id| self.public.note(card_id).play);
        let my_trash = self.public.hands[view.me()]
            .iter()
            .position(|&card_id| self.public.note(card_id).trash);
        let my_chop = view.hand_size(view.me()) - 1;

        let best_hint = self.best_hint(view);

        match (best_hint, my_play, my_trash, view.board.hints_remaining) {
            (Some(hint), _, _, 1..) => TurnChoice::Hint(hint),
            (_, Some(play), _, _) => TurnChoice::Play(play),
            (_, _, Some(trash), _) => TurnChoice::Discard(trash),
            (None, None, None, _) | (Some(_), None, None, 0) => TurnChoice::Discard(my_chop),
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
                self.public.reveal_copy(*card, card_id);
            }
            (TurnChoice::Play(index), TurnResult::Play(card, _)) => {
                let card_id = self.public.hands[self.public.board.player][index];
                self.public.reveal_copy(*card, card_id);
            }
            _ => panic!("mismatched turn choice and turn result"),
        }

        self.public.update_board(view);
    }
}
