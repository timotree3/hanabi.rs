use crate::{
    game::{
        BoardState, CardId, GameOptions, Hand, Hint as HintChoice, Hinted, Player, PlayerView,
        TurnChoice, TurnRecord, TurnResult, COLORS, TOTAL_CARDS, VALUES,
    },
    helpers::PerPlayer,
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
    hands: PerPlayer<Hand>,
    board: BoardState<'game>,
}

impl<'game> Public<'game> {
    fn first_turn(view: &PlayerView<'game>) -> Public<'game> {
        Public {
            notes: vec![Note::default(); TOTAL_CARDS as usize],
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

    fn categorize_hint(&self, hint: &Hint) -> Option<HintCategory> {
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
        match self.categorize_hint(hint) {
            Some(HintCategory::RefPlay(target)) => {
                self.note_mut(target).play = true;
            }
            Some(HintCategory::EightClueStall) => {}
            None => panic!("unconventional hint given"),
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

    fn update_board(&mut self, view: &PlayerView<'game>) {
        self.hands = view.hands().clone();
        self.board = view.board.clone();
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

enum HintCategory {
    RefPlay(CardId),
    EightClueStall,
}

#[derive(Debug, Copy, Clone, Default)]
struct Note {
    clued: bool,
    play: bool,
}

struct RsPlayer<'game> {
    /// The public knowledge shared amongst the players
    public: Public<'game>,
}

impl RsPlayer<'_> {
    fn best_hint(&self, view: &PlayerView<'_>) -> Option<HintChoice> {
        let mut best = None;

        for hint in possible_hints(view) {
            match self.public.categorize_hint(&hint) {
                Some(HintCategory::RefPlay(target)) => {
                    if view.board.is_playable(view.card(target)) {
                        best = Some(hint);
                    }
                }
                Some(HintCategory::EightClueStall) => {
                    if best.is_none() {
                        best = Some(hint);
                    }
                }
                None => {}
            }
        }

        best.map(|hint| hint.choice())
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
        let my_play = view.hands()[view.me()]
            .iter()
            .position(|&card_id| self.public.note(card_id).play);
        if let Some(index) = my_play {
            TurnChoice::Play(index)
        } else if view.board.hints_remaining == 0 {
            TurnChoice::Discard(view.hand_size(view.me()) - 1)
        } else if let Some(hint) = self.best_hint(view) {
            TurnChoice::Hint(hint)
        } else {
            TurnChoice::Discard(view.hand_size(view.me()) - 1)
        }
    }

    fn update(&mut self, turn_record: &TurnRecord, view: &PlayerView<'game>) {
        match (turn_record.choice, &turn_record.result) {
            (TurnChoice::Hint(HintChoice { player, hinted }), TurnResult::Hint(touched)) => {
                let touched_ids: Vec<CardId> = touched_ids(player, touched, view.hands()).collect();
                self.public.interpret_hint(&Hint {
                    receiver: player,
                    hinted,
                    touched: touched_ids,
                });
            }
            (TurnChoice::Discard(_), TurnResult::Discard(_)) => {}
            (TurnChoice::Play(_), TurnResult::Play(_, _)) => {}
            _ => panic!("mismatched turn choice and turn result"),
        }

        self.public.update_board(view);
    }
}
