mod conventions;

use std::cmp::Ordering;

use crate::{
    game::{
        BoardState, Card, CardCounts, CardId, GameOptions, Hand, Hint as HintChoice, Hinted,
        Player, PlayerView, TurnChoice, TurnRecord, TurnResult, COLORS, VALUES,
    },
    helpers::{CardInfo, CardPossibilityTable, PerPlayer},
    strategy::{GameStrategy, GameStrategyConfig, PlayerStrategy},
};

use self::conventions::{is_conventional, ChoiceCategory, ChoiceDesc, PublicKnowledge};

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
            knowledge: PublicKnowledge::first_turn(),
            state: State::first_turn(view),
        })
    }
}

// TODO: Add 'turn lifetime and only borrow hands/board
struct State<'game> {
    // TODO: have some way for the conventions to restrict empathy
    empathy: Vec<CardPossibilityTable>,
    card_counts: CardCounts, // what any newly drawn card should be
    hands: PerPlayer<Hand>,
    board: BoardState<'game>,
}

impl<'game> State<'game> {
    fn first_turn(view: &PlayerView<'game>) -> State<'game> {
        State {
            empathy: vec![
                CardPossibilityTable::new();
                (view.board.opts.num_players * view.board.opts.hand_size) as usize
            ],
            card_counts: CardCounts::new(),
            hands: view.hands().clone(),
            board: view.board.clone(),
        }
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

    fn update_board(&mut self, view: &PlayerView<'game>, knowledge: &mut PublicKnowledge) {
        if view.board.deck_size != self.board.deck_size {
            assert!(view.board.deck_size == self.board.deck_size - 1);
            self.draw_card();
        }
        self.hands.clone_from(view.hands());
        self.board.clone_from(&view.board);

        knowledge.check_empathy(self);
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

struct RsPlayer<'game> {
    /// The convention-agnostic public information
    state: State<'game>,
    knowledge: PublicKnowledge,
}

impl RsPlayer<'_> {
    fn describe_choice(
        &mut self,
        choice: &Choice,
        backup_empathy: &[CardPossibilityTable],
    ) -> Option<ChoiceDesc> {
        if let Choice::Hint(hint) = choice {
            // Hack: Update the empathy as it would be after the hint was given
            self.state.update_empathy_for_hint(hint);
        }
        let desc = self.knowledge.describe_choice(&self.state, choice);
        if let Choice::Hint(_) = choice {
            // Restore the empathy
            self.state.empathy.clone_from_slice(backup_empathy);
        }
        desc
    }

    /// Chooses a preferred move in the position.
    ///
    /// Mutates self in place for efficiency but should leave it unchanged upon exiting.

    fn choose(&mut self, view: &PlayerView<'_>) -> Option<Choice> {
        let backup_empathy = self.state.empathy.clone();
        if let Some((choice, _)) = possible_choices(view)
            .filter_map(|choice| {
                self.describe_choice(&choice, &backup_empathy)
                    .filter(|desc| is_conventional(&self.state, view, desc))
                    .map(|desc| (choice, desc))
            })
            .max_by(|a, b| compare_choice(view, a, b))
        {
            Some(choice)
        } else {
            None
        }
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

fn compare_choice(
    _view: &PlayerView<'_>,
    a: &(Choice, ChoiceDesc),
    b: &(Choice, ChoiceDesc),
) -> Ordering {
    // TODO:
    // - Use some kind of tree search for comparison?
    //   - Or just better heuristics?
    // - Plan out lines to extract all safe actions from partner's hand using the fewest clues and with the most flexibility
    // - Care about clue count
    // - Care about playing unknown cards first
    // - Care about playing cards which lead into own hand/into partner's hand
    // - Care about demonstrating private knowledge
    //
    // Perhaps we can decide most things with heuristics and the main hard question is how many BDRs to commit to.
    // We can pick the best moves which commit to 0, 1, and 2 BDRs and do search only among those
    // Perhaps we can even cut off the search early if the line catches up in BDRs
    match (a, b) {
        ((Choice::Play(_), _), (Choice::Play(_), _)) => Ordering::Equal,
        ((Choice::Play(_), _), (_, _)) => Ordering::Greater,
        ((_, _), (Choice::Play(_), _)) => Ordering::Less,
        ((Choice::Discard(_), _), (Choice::Discard(_), _)) => Ordering::Equal,
        ((Choice::Discard(_), _), _) => Ordering::Greater,
        (_, (Choice::Discard(_), _)) => Ordering::Less,
        (
            (
                Choice::Hint(_),
                ChoiceDesc {
                    category: ChoiceCategory::Hint(hint_a),
                    ..
                },
            ),
            (
                Choice::Hint(_),
                ChoiceDesc {
                    category: ChoiceCategory::Hint(hint_b),
                    ..
                },
            ),
        ) => hint_a.new_plays().cmp(&hint_b.new_plays()),
        ((Choice::Hint(_), _), (Choice::Hint(_), _)) => {
            panic!("mismatching choice and choicedesc types")
        }
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

    fn decide(&mut self, view: &PlayerView<'_>) -> Option<TurnChoice> {
        let choice = self.choose(view)?;
        let card_id_to_index = |card_id| {
            self.state.hands[view.me()]
                .iter()
                .position(|&id| id == card_id)
        };

        Some(match choice {
            Choice::Play(card_id) => TurnChoice::Play(
                card_id_to_index(card_id).expect("chose to play a card which was not held"),
            ),
            Choice::Discard(card_id) => TurnChoice::Discard(
                card_id_to_index(card_id).expect("chose to play a card which was not held"),
            ),
            Choice::Hint(hint) => TurnChoice::Hint(hint.choice()),
        })
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
                self.state.update_empathy_for_hint(&hint);
                self.knowledge
                    .interpret_choice(&self.state, &Choice::Hint(hint));
            }
            (TurnChoice::Discard(index), TurnResult::Discard(card)) => {
                let card_id = self.state.hands[self.state.board.player][index];
                self.knowledge
                    .interpret_choice(&self.state, &Choice::Discard(card_id));
                self.state.reveal_copy(*card, card_id);
            }
            (TurnChoice::Play(index), TurnResult::Play(card, _)) => {
                let card_id = self.state.hands[self.state.board.player][index];
                self.knowledge
                    .interpret_choice(&self.state, &Choice::Play(card_id));
                self.state.reveal_copy(*card, card_id);
            }
            _ => panic!("mismatched turn choice and turn result"),
        }

        self.state.update_board(view, &mut self.knowledge);
    }
}
