use std::cmp::Eq;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

use game::*;

// Represents information about possible values of type T
pub trait Info<T> where T: Hash + Eq + Clone {
    // get all a-priori possibilities
    fn get_all_possibilities() -> Vec<T>;

    // get map from values to whether it's possible
    // true means maybe, false means no
    fn get_possibility_map(&self) -> &HashMap<T, bool>;
    fn get_mut_possibility_map(&mut self) -> &mut HashMap<T, bool>;

    // get what is now possible
    fn get_possibilities(&self) -> Vec<&T> {
        let mut v = Vec::new();
        let map = self.get_possibility_map();
        for (value, is_possible) in map {
            if *is_possible {
                v.push(value);
            }
        }
        v
    }

    fn is_possible(&self, value: &T) -> bool {
        // self.get_possibility_map().contains_key(value)
        *self.get_possibility_map().get(value).unwrap()
    }

    fn initialize() -> HashMap<T, bool> {
        let mut possible_map : HashMap<T, bool> = HashMap::new();
        for value in Self::get_all_possibilities().iter() {
            possible_map.insert(value.clone(), true);
        }
        possible_map
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
    pub fn new() -> ColorInfo { ColorInfo(ColorInfo::initialize()) }
}
impl Info<Color> for ColorInfo {
    fn get_all_possibilities() -> Vec<Color> { COLORS.to_vec() }
    fn get_possibility_map(&self) -> &HashMap<Color, bool> { &self.0 }
    fn get_mut_possibility_map(&mut self) -> &mut HashMap<Color, bool> { &mut self.0 }
}

#[derive(Debug)]
pub struct ValueInfo(HashMap<Value, bool>);
impl ValueInfo {
    pub fn new() -> ValueInfo { ValueInfo(ValueInfo::initialize()) }
}
impl Info<Value> for ValueInfo {
    fn get_all_possibilities() -> Vec<Value> { VALUES.to_vec() }
    fn get_possibility_map(&self) -> &HashMap<Value, bool> { &self.0 }
    fn get_mut_possibility_map(&mut self) -> &mut HashMap<Value, bool> { &mut self.0 }
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
impl fmt::Display for CardInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut string = String::new();
        for color in &COLORS {
            if self.color_info.is_possible(color) {
                string.push(display_color(color));
            }
        }
        // while string.len() < COLORS.len() + 1 {
        string.push(' ');
        //}
        for value in &VALUES {
            if self.value_info.is_possible(value) {
                string.push_str(&format!("{}", value));
            }
        }
        f.pad(&string)
    }
}
