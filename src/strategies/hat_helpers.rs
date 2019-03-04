use game::*;
use helpers::*;

#[derive(Debug,Clone)]
pub struct ModulusInformation {
    pub modulus: u32,
    pub value: u32,
}
impl ModulusInformation {
    pub fn new(modulus: u32, value: u32) -> Self {
        assert!(value < modulus);
        ModulusInformation {
            modulus: modulus,
            value: value,
        }
    }

    pub fn none() -> Self {
        Self::new(1, 0)
    }

    pub fn combine(&mut self, other: Self) {
        self.value = self.value + self.modulus * other.value;
        self.modulus = self.modulus * other.modulus;
    }

    pub fn split(&mut self, modulus: u32) -> Self {
        assert!(self.modulus >= modulus);
        assert!(self.modulus % modulus == 0);
        let original_modulus = self.modulus;
        let original_value = self.value;
        self.modulus = self.modulus / modulus;
        let value = self.value % modulus;
        self.value = self.value / modulus;
        assert!(original_modulus == modulus * self.modulus);
        assert!(original_value == value + modulus * self.value);
        Self::new(modulus, value)
    }

    pub fn cast_up(&mut self, modulus: u32) {
        assert!(self.modulus <= modulus);
        self.modulus = modulus;
    }

    pub fn cast_down(&mut self, modulus: u32) {
        assert!(self.modulus >= modulus);
        assert!(self.value < modulus);
        self.modulus = modulus;
    }

    pub fn add(&mut self, other: &Self) {
        assert!(self.modulus == other.modulus);
        self.value = (self.value + other.value) % self.modulus;
    }

    pub fn subtract(&mut self, other: &Self) {
        assert!(self.modulus == other.modulus);
        self.value = (self.modulus + self.value - other.value) % self.modulus;
    }
}

pub trait Question {
    // how much info does this question ask for?
    fn info_amount(&self) -> u32;
    // get the answer to this question, given cards
    fn answer(&self, &Cards, &BoardState) -> u32;
    // process the answer to this question, updating card info
    fn acknowledge_answer(
        &self, value: u32, &mut HandInfo<CardPossibilityTable>, &BoardState
    );

    fn answer_info(&self, hand: &Cards, board: &BoardState) -> ModulusInformation {
        ModulusInformation::new(
            self.info_amount(),
            self.answer(hand, board)
        )
    }

    fn acknowledge_answer_info(
        &self,
        answer: ModulusInformation,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        board: &BoardState
    ) {
        assert!(self.info_amount() == answer.modulus);
        self.acknowledge_answer(answer.value, hand_info, board);
    }
}

pub trait PublicInformation: Clone {
    fn get_player_info(&self, &Player) -> HandInfo<CardPossibilityTable>;
    fn set_player_info(&mut self, &Player, HandInfo<CardPossibilityTable>);

    fn new(&BoardState) -> Self;
    fn set_board(&mut self, &BoardState);

    /// If we store more state than just `HandInfo<CardPossibilityTable>`s, update it after `set_player_info` has been called.
    fn update_other_info(&mut self) {
    }

    fn agrees_with(&self, other: Self) -> bool;

    /// By defining `ask_questions`, we decides which `Question`s a player learns the answers to.
    ///
    /// A player "asks" a question by calling the callback. Questions can depend on the answers to
    /// earlier questions: We are given a `&mut HandInfo<CardPossibilityTable>` that we'll have to pass
    /// to that callback; there, it will be modified to reflect the answer to the question. Note that `self`
    /// is not modified and thus reflects the state before any player "asked" any question.
    ///
    /// The product of the `info_amount()`s of all questions we have may not exceed `total_info`.
    /// For convenience, we pass a `&mut u32` to the callback, and it will be updated to the
    /// "remaining" information amount.
    fn ask_questions<Callback>(&self, &Player, &mut HandInfo<CardPossibilityTable>, Callback, total_info: u32)
        where Callback: FnMut(&mut HandInfo<CardPossibilityTable>, &mut u32, Box<Question>);

    fn set_player_infos(&mut self, infos: Vec<(Player, HandInfo<CardPossibilityTable>)>) {
        for (player, new_hand_info) in infos {
            self.set_player_info(&player, new_hand_info);
        }
        self.update_other_info();
    }

    fn get_hat_info_for_player(
        &self, player: &Player, hand_info: &mut HandInfo<CardPossibilityTable>, total_info: u32, view: &OwnedGameView
    ) -> ModulusInformation {
        assert!(player != &view.player);
        let mut answer_info = ModulusInformation::none();
        {
            let callback = |hand_info: &mut HandInfo<CardPossibilityTable>, info_remaining: &mut u32, question: Box<Question>| {
                *info_remaining = *info_remaining / question.info_amount();
                let new_answer_info = question.answer_info(view.get_hand(player), view.get_board());
                question.acknowledge_answer_info(new_answer_info.clone(), hand_info, view.get_board());
                answer_info.combine(new_answer_info);
            };
            self.ask_questions(player, hand_info, callback, total_info);
        }
        answer_info.cast_up(total_info);
        answer_info
    }

    fn update_from_hat_info_for_player(
        &self,
        player: &Player,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        board: &BoardState,
        mut info: ModulusInformation,
    ) {
        let total_info = info.modulus;
        let callback = |hand_info: &mut HandInfo<CardPossibilityTable>, info_remaining: &mut u32, question: Box<Question>| {
            let q_info_amount = question.info_amount();
            *info_remaining = *info_remaining / q_info_amount;
            let info_modulus = info.modulus;
            // Instead of casting down the ModulusInformation to the product of all the questions before
            // answering any, we now have to cast down the ModulusInformation question-by-question.
            info.cast_down((info_modulus / q_info_amount) * q_info_amount);
            let answer_info = info.split(question.info_amount());
            question.acknowledge_answer_info(answer_info, hand_info, board);
        };
        self.ask_questions(player, hand_info, callback, total_info);
    }

    /// When deciding on a move, if we can choose between `total_info` choices,
    /// `self.get_hat_sum(total_info, view)` tells us which choice to take, and at the same time
    /// mutates `self` to simulate the choice becoming common knowledge.
    fn get_hat_sum(&mut self, total_info: u32, view: &OwnedGameView) -> ModulusInformation {
        let (infos, new_player_hands): (Vec<_>, Vec<_>) = view.get_other_players().iter().map(|player| {
            let mut hand_info = self.get_player_info(player);
            let info = self.get_hat_info_for_player(player, &mut hand_info, total_info, view);
            (info, (player.clone(), hand_info))
        }).unzip();
        self.set_player_infos(new_player_hands);
        infos.into_iter().fold(
            ModulusInformation::new(total_info, 0),
            |mut sum_info, info| {
                sum_info.add(&info);
                sum_info
            }
        )
    }

    /// When updating on a move, if we infer that the player making the move called `get_hat_sum()`
    /// and got the result `info`, we can call `self.update_from_hat_sum(info, view)` to update
    /// from that fact.
    fn update_from_hat_sum(&mut self, mut info: ModulusInformation, view: &OwnedGameView) {
        let info_source = view.board.player;
        let (other_infos, mut new_player_hands): (Vec<_>, Vec<_>) = view.get_other_players().into_iter().filter(|player| {
            *player != info_source
        }).map(|player| {
            let mut hand_info = self.get_player_info(&player);
            let player_info = self.get_hat_info_for_player(&player, &mut hand_info, info.modulus, view);
            (player_info, (player.clone(), hand_info))
        }).unzip();
        for other_info in other_infos {
            info.subtract(&other_info);
        }
        let me = view.player;
        if me == info_source {
            assert!(info.value == 0);
        } else {
            let mut my_hand = self.get_player_info(&me);
            self.update_from_hat_info_for_player(&me, &mut my_hand, &view.board, info);
            new_player_hands.push((me, my_hand));
        }
        self.set_player_infos(new_player_hands);
    }

    fn get_private_info(&self, view: &OwnedGameView) -> HandInfo<CardPossibilityTable> {
        let mut info = self.get_player_info(&view.player);
        for card_table in info.iter_mut() {
            for (_, hand) in &view.other_hands {
                for card in hand {
                    card_table.decrement_weight_if_possible(card);
                }
            }
        }
        info
    }
}
