use std::collections::HashMap;
use std::cmp::Eq;
use std::hash::Hash;

use game::*;

// Represents a bit of information about T
pub trait Info<T> where T: Hash + Eq + Clone {
    // get all a-priori possibilities
    fn get_possibilities() -> Vec<T>;

    // get map from values to whether it's possible
    // true means maybe, false means no
    fn get_possibility_map(&self) -> &HashMap<T, bool>;
    fn get_mut_possibility_map(&mut self) -> &mut HashMap<T, bool>;

    fn initialize() -> HashMap<T, bool> {
        let mut possible_map : HashMap<T, bool> = HashMap::new();
        for value in Self::get_possibilities().iter() {
            possible_map.insert(value.clone(), true);
        }
        possible_map
    }

    fn merge(&mut self, other: &Self) {
        for (value, possible) in self.get_mut_possibility_map().iter_mut() {
            *possible = *possible && *other.get_possibility_map().get(value).unwrap();
        }
    }

    fn mark_true(&mut self, value: &T) {
        // mark everything else as definitively impossible
        for (other_value, possible) in self.get_mut_possibility_map().iter_mut() {
            if other_value != value {
                *possible = false;
            } else {
                assert_eq!(*possible, true);
            }
        }
    }

    fn mark_false(&mut self, value: &T) {
        self.get_mut_possibility_map().insert(value.clone(), false);
    }

    fn mark(&mut self, value: &T, info: bool) {
        if info {
            self.mark_true(value);
        } else {
            self.mark_false(value);
        }
    }
}

#[derive(Debug)]
pub struct ColorInfo(HashMap<Color, bool>);
impl ColorInfo {
    pub fn new() -> ColorInfo {
        ColorInfo(ColorInfo::initialize())
    }
}
impl Info<Color> for ColorInfo {
    fn get_possibilities() -> Vec<Color> {
        let mut possible : Vec<Color> = Vec::new();
        for color in COLORS.iter() {
            possible.push(*color);
        }
        possible
    }
    fn get_possibility_map(&self) -> &HashMap<Color, bool> {
        &self.0
    }
    fn get_mut_possibility_map(&mut self) -> &mut HashMap<Color, bool> {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct ValueInfo(HashMap<Value, bool>);
impl ValueInfo {
    pub fn new() -> ValueInfo {
        ValueInfo(ValueInfo::initialize())
    }
}
impl Info<Value> for ValueInfo {
    fn get_possibilities() -> Vec<Value> {
        let mut possible : Vec<Value> = Vec::new();
        for value in VALUES.iter() {
            possible.push(*value);
        }
        possible
    }
    fn get_possibility_map(&self) -> &HashMap<Value, bool> {
        &self.0
    }
    fn get_mut_possibility_map(&mut self) -> &mut HashMap<Value, bool> {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct CardInfo {
    pub color_info: ColorInfo,
    pub value_info: ValueInfo,
}
impl CardInfo {
    pub fn new() -> CardInfo {
        CardInfo {
            color_info: ColorInfo::new(),
            value_info: ValueInfo::new(),
        }
    }
}
