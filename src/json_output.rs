use crate::game::*;
use serde_json::*;

fn color_value(color: &Color) -> usize {
    COLORS
        .iter()
        .position(|&card_color| &card_color == color)
        .unwrap()
}

fn card_to_json(card: &Card) -> serde_json::Value {
    json!({
        "rank": card.value,
        "suitIndex": color_value(&card.color),
    })
}

pub fn action_clue(hint: &Hint) -> serde_json::Value {
    match hint.hinted {
        Hinted::Color(color) => {
            json!({
                "type": 2,
                "target": hint.player,
                "value": color_value(&color),
            })
        }
        Hinted::Value(value) => {
            json!({
                "type": 3,
                "target": hint.player,
                "value": value,
            })
        }
    }
}

pub fn action_play((i, _card): &AnnotatedCard) -> serde_json::Value {
    json!({
        "type": 0,
        "target": i,
    })
}

pub fn action_discard((i, _card): &AnnotatedCard) -> serde_json::Value {
    json!({
        "type": 1,
        "target": i,
    })
}

pub fn json_format(
    deck: &Cards,
    actions: &Vec<serde_json::Value>,
    players: &Vec<String>,
) -> serde_json::Value {
    json!({
        "options": {
            "variant": "No Variant",
        },
        "players": players,
        "first_player": 0,
        "notes": players.iter().map(|_player| {json!([])}).collect::<Vec<_>>(), // TODO add notes
        // The deck is reversed since in our implementation we draw from the end of the deck.
        "deck": deck.iter().rev().map(card_to_json).collect::<Vec<serde_json::Value>>(),
        "actions": actions,
    })
}
