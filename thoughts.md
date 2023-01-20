Categorize move
(Action, S) -> Category

Update conventional state
(Category, S) -> S

Is conventional
Category -> Bool

Decide move
(S, Context) -> Action

- Potential model:
    - Have a set of preference functions [Action, S, Context] -> Preference
    - `Preference` would have a multiplication operation and a zero, so that a preference function can rule out an option by returning zero
    - Are rationals and multiplcation a good model for preference? How do we express opinions on actions which discard criticals?

You could try to prove that a conventional game never bombs out. False for referential sieve 2p? (A conventional game is where all actions are conventional)

Update contextual state
(Action, S, Context) -> Context

Thought: It's inelegant how we have to deal with slot numbers and stuff. Perhaps have per-cade state in the framework and only describe clues as touching cards with those states? Means that relative order of cards might have to be in their state, if the system cares about that, but that also means the system is being explicit about what it cares about.

Thought: It would be cool to prove theorems about loss conditions in conventional games. We either lose in the endgame, discarding/misplaying a critical, or striking out. We never misplay a critical in conventional games. When do we discard a critical in conventional games? You could talk propositionally about games like "(conventional game includes critical discard) => (all held cards were critical) or (there were zero clues)"
- You could also chop of all of the loss conditions this way


Case study: define just discard newest, then update to include ref play clues

Just discard newest

English description:
- Players are expected to discard their leftmost unclued card

```rs
struct GlobalState {
    hands: Vec<Vec<PerCardState>>,
}
struct PerCardState {
    chop_priority: u8,
}

fn initial_state(opts: GameOptions) -> GlobalState {
    let initial_hand = 
        (1..(opts.hand_size + 1)).map(|n| PerCardState { chop_priority: n }).collect();
    
    GlobalState {
        hands: vec![initial_hand; opts.player_count]
    },
}

enum Category {
    Unconventional,
    DiscardChop
}

fn categorize_move(action: Action, state: GlobalState) -> Category {
    if let Action::Discard { card_state } = action && card_state == hands[action.player].max() {
        Category::DiscardChop,
    } else {
        Category::Unconventional,
    }
}
```
