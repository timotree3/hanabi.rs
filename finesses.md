
- 


```rust
struct PlayerKnowledge {
    my_queue: VecDeque<QueuedClue>
    instructed_plays: PerPlayer<Vec<CardId>>,
    known_trash: Set<CardId>,
}

#[derive(Debug, Clone)]
struct QueuedClue {
    slot_sum: u8
    num_plays: u8,
    first_response: Option<FirstResponse>,
    play_responses: Set<PlayResponse>,
    remaining_play_responders: Vec<Player>,
    discard_knowledge: DiscardKnowledge
}

struct PlayResponse {
   card: Card,
   slot: u8 
}

enum FirstResponse {
    Discard { slot: u8 }
    Play,
}

enum DiscardKnowledge {
    /// I don't even know what to consider trash yet because I have unknown plays
    PlayStacksUndetermined {
        stacks_when_clued: PlayStacks,
        my_unknown_plays: Set<CardId>,
        hands_when_clued: Hands,
        stacked_when_clued: Set<Player>
        clue_giver: Player
    },
    PlayStacksDetermined {
        // Some for everyone whose hand I see. None for me and the clue giver
        candidate_discard: PerPlayer<Option<u8>>,
    }
}
```