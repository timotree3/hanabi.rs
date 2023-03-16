use std::{cmp::Ordering, collections::VecDeque};

use fnv::{FnvHashMap, FnvHashSet};

use crate::{
    game::{
        BoardState, Card, CardId, Color, Firework, GameOptions, Hand, Hint as HintChoice, Hinted,
        Player, PlayerView, TurnChoice, TurnRecord, TurnResult, COLORS, VALUES,
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
        Box::new(HatPlayer {
            state: State::first_turn(view),
            my_queue: VecDeque::new(),
            instructed_plays: PerPlayer::new(view.board.opts.num_players, |_| Vec::new()),
            known_trash: FnvHashSet::default(),
        })
    }
}

type PlayStacks = FnvHashMap<Color, Firework>;

// TODO: Add 'turn lifetime and only borrow hands/board
struct State<'game> {
    hands: PerPlayer<Hand>,
    board: BoardState<'game>,
}

impl<'game> State<'game> {
    fn first_turn(view: &PlayerView<'game>) -> State<'game> {
        State {
            hands: view.hands().clone(),
            board: view.board.clone(),
        }
    }

    fn update_board(&mut self, view: &PlayerView<'game>) {
        self.hands.clone_from(view.hands());
        self.board.clone_from(&view.board);
    }

    fn slot_of(&self, player: u32, card_id: u32) -> u8 {
        let index = self.hands[player]
            .iter()
            .position(|&c| c == card_id)
            .expect("card played is in hand");
        (self.hands[player].len() - index) as u8
    }

    // /// Returns true if and only if this card is known playable now and
    // /// will be known trash immediately if it ever becomes unplayable
    // fn is_empathy_permanently_playable(&self, card_id: u32) -> bool {
    //     self.is_empathy_playable(card_id)
    //         && self.empathy[card_id as usize]
    //             .get_possibilities()
    //             .iter()
    //             .filter(|&&card| self.board.is_dispensable(card))
    //             .count()
    //             <= 1
    // }

    // fn is_empathy_known(&self, card_id: u32) -> bool {
    //     self.empathy[card_id as usize].is_determined()
    // }
}

#[derive(Clone)]
enum Choice {
    Play(CardId),
    Discard(CardId),
    Hint(Hint),
}

#[derive(Clone)]
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

enum ChoiceOutcome {
    Play(CardId, Card),
    Discard(CardId, Card),
    Hint(Hint),
}

struct HatPlayer<'game> {
    /// The convention-agnostic public information
    state: State<'game>,
    my_queue: VecDeque<QueuedClue>,
    instructed_plays: PerPlayer<Vec<CardId>>,
    known_trash: FnvHashSet<CardId>,
}

struct QueuedClue {
    slot_sum: u8,
    num_plays: u8,
    first_response: Option<FirstResponse>,
    play_responses: Vec<PlayResponse>,
    remaining_play_responders: FnvHashSet<Player>,
    stacks_when_clued: PlayStacks,
    my_unknown_plays_when_clued: Vec<CardId>,
    hands_when_clued: PerPlayer<Hand>,
    stacked_when_clued: FnvHashSet<Player>,
    clue_giver: Player,
}
impl QueuedClue {
    fn from_hint(player: &HatPlayer<'_>, hint: &Hint) -> Self {
        let touches_newest = hint
            .touched
            .contains(player.state.hands[hint.receiver].last().unwrap());
        let hint_value = match (hint.hinted, touches_newest) {
            (Hinted::Value(_), false) => 1,
            (Hinted::Value(_), true) => 2,
            (Hinted::Color(_), true) => 3,
            (Hinted::Color(_), false) => 4,
        };
        let num_players_away = (player.state.board.opts.num_players + hint.receiver
            - player.state.board.player)
            % player.state.board.opts.num_players;
        let last_responder = player.state.board.player_to_left(player.state.board.player);
        if player.state.board.opts.num_players == 5 && hint.receiver == last_responder {
            // "Emily clue"
            QueuedClue {
                slot_sum: 0,
                num_plays: hint_value,
                first_response: None,
                play_responses: Vec::new(),
                remaining_play_responders: FnvHashSet::from_iter(
                    player
                        .state
                        .board
                        .get_players()
                        .filter(|&p| p == player.state.board.player),
                ),
                stacks_when_clued: player.state.board.fireworks.clone(),
                my_unknown_plays_when_clued: todo!(),
                hands_when_clued: todo!(),
                stacked_when_clued: todo!(),
                clue_giver: todo!(),
            }
        } else {
            QueuedClue {
                slot_sum: hint_value,
                num_plays: num_players_away as u8,
                last_responder,
            }
        }
    }

    fn is_possibly_play_response(&self, player: u32, card: Card) -> bool {
        self.remaining_play_responders.contains(&player)
            && self
                .play_responses
                .iter()
                .all(|response| response.card.color != card.color) // no finesses yet
    }
}

struct PlayResponse {
    card: Card,
    slot: u8,
}

enum FirstResponse {
    Discard { slot: u8 },
    Play,
}

impl HatPlayer<'_> {
    fn interpret_outcome(&mut self, outcome: &ChoiceOutcome) {
        let player = self.state.board.player;
        match *outcome {
            ChoiceOutcome::Play(card_id, card) => {
                if self.instructed_plays[player].last() == Some(&card_id) {
                    // Expected play
                    self.instructed_plays[player].pop();
                } else {
                    let clue = self
                        .my_queue
                        .iter_mut()
                        .find(|clue| clue.is_possibly_play_response(player, card))
                        .expect("todo");

                    clue.first_response.get_or_insert(FirstResponse::Play);
                    let slot = self.state.slot_of(player, card_id);

                    clue.play_responses.push(PlayResponse { card, slot });
                    clue.remaining_play_responders.remove(&player);
                }
            }
            ChoiceOutcome::Discard(card_id, card) => {
                if self.known_trash.remove(&card_id) {
                    // Expected discard
                } else {
                    let slot = self.state.slot_of(player, card_id);

                    // A discard responds to every clue
                    for clue in &mut self.my_queue {
                        clue.first_response
                            .get_or_insert(FirstResponse::Discard { slot });
                        clue.remaining_play_responders.remove(&player);
                    }
                }
            }
            ChoiceOutcome::Hint(ref hint) => {
                // A hint responds to every clue
                for clue in &mut self.my_queue {
                    // A slot of 0 represents a lock
                    clue.first_response
                        .get_or_insert(FirstResponse::Discard { slot: 0 });
                    clue.remaining_play_responders.remove(&player);
                }
            }
        }
    }

    fn prepare_my_turn(&mut self) {
        todo!()
    }

    /// Chooses a preferred move in the position.
    fn choose(&self, view: &PlayerView<'_>) -> Option<Choice> {
        todo!()
        // let conventional_alternatives = {
        //     let mut interpretable_choices: Vec<(Choice, ChoiceDesc)> = possible_choices(view)
        //         .filter_map(|choice| {
        //             self.describe_choice(&choice, &backup_empathy)
        //                 .map(|desc| (choice, desc))
        //         })
        //         .collect();
        //     let one_conventional_alternative = interpretable_choices
        //         .iter()
        //         .map(|(_, desc)| desc)
        //         .max_by(|a, b| {
        //             self.knowledge
        //                 .compare_conventional_alternatives(&self.state, view, a, b)
        //         })?
        //         .clone();
        //     interpretable_choices.retain(|(_, desc)| {
        //         self.knowledge
        //             .compare_conventional_alternatives(
        //                 &self.state,
        //                 view,
        //                 desc,
        //                 &one_conventional_alternative,
        //             )
        //             .is_ge()
        //     });
        //     interpretable_choices
        // };

        // Some(
        //     conventional_alternatives
        //         .into_iter()
        //         .max_by(|a, b| compare_choice(view, a, b))
        //         .unwrap()
        //         .0,
        // )
    }
}

impl<'game> PlayerStrategy<'game> for HatPlayer<'game> {
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
                self.interpret_choice(&Choice::Hint(hint));
            }
            (TurnChoice::Discard(index), TurnResult::Discard(card)) => {
                let card_id = self.state.hands[self.state.board.player][index];
                self.interpret_choice(&Choice::Discard(card_id));
                self.reveal_copy(*card, card_id);
            }
            (TurnChoice::Play(index), TurnResult::Play(card, _)) => {
                let card_id = self.state.hands[self.state.board.player][index];
                self.interpret_choice(&Choice::Play(card_id));
                self.reveal_copy(*card, card_id);
            }
            _ => panic!("mismatched turn choice and turn result"),
        }

        self.state.update_board(view);

        if view.board.player == view.board.player_to_left(view.me()) {
            self.prepare_my_turn();
        }
    }

    fn notes(&self) -> Vec<String> {
        // TODO: notes
        Vec::new()
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
