use crate::game::*;
use serde_json::*;

fn color_value(color: Color) -> usize {
    COLORS
        .iter()
        .position(|&card_color| card_color == color)
        .unwrap()
}

fn card_to_json(card: Card) -> serde_json::Value {
    json!({
        "rank": card.value,
        "suitIndex": color_value(card.color),
    })
}

pub fn action_clue(hint: &Hint) -> serde_json::Value {
    match hint.hinted {
        Hinted::Color(color) => {
            json!({
                "type": 2,
                "target": hint.player,
                "value": color_value(color),
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

pub fn action_play(card_id: CardId) -> serde_json::Value {
    json!({
        "type": 0,
        "target": card_id,
    })
}

pub fn action_discard(card_id: CardId) -> serde_json::Value {
    json!({
        "type": 1,
        "target": card_id,
    })
}
pub fn action_terminate(player: Player) -> serde_json::Value {
    json!({
        // 4 represent game end
        "type": 4,
        "target": player,
        // 4 represnts manual termination
        "value": 4
    })
}

pub fn json_format(
    deck: &[Card],
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
        "deck": deck.iter().copied().map(card_to_json).collect::<Vec<serde_json::Value>>(),
        "actions": actions,
    })
}
