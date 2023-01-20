use std::ops::{Index, IndexMut};

use crate::game::Player;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerPlayer<T>(Vec<T>);

impl<T> PerPlayer<T> {
    pub fn new<F>(num_players: u32, initialize: F) -> Self
    where
        F: FnMut(Player) -> T,
    {
        PerPlayer((0..num_players).map(initialize).collect())
    }

    pub fn iter(&self) -> impl Iterator<Item = (Player, &'_ T)> + '_ {
        self.0.iter().enumerate().map(|(i, t)| (i as Player, t))
    }
}

impl<T> Index<Player> for PerPlayer<T> {
    type Output = T;

    fn index(&self, player: Player) -> &Self::Output {
        &self.0[player as usize]
    }
}

impl<T> IndexMut<Player> for PerPlayer<T> {
    fn index_mut(&mut self, player: Player) -> &mut Self::Output {
        &mut self.0[player as usize]
    }
}
