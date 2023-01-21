Conventions:

- Referential Play Clues
    - Touches at least one "previously unclued" card (excluding known trash, already instructed plays, and already clued cards)
    - Focus is leftmost except that the leftmost previously unclued card has lowest precedence
    - Means to play the target. The target can be on top of
        - Good touch plays in receiver's hand
        - Already instructed plays in receiver's hand
        - Filled-in plays in receiver's hand
        - Known (private knowledge included) plays in giver's hand if there is enough time
- Referential Discard Clues
    - Touches at least one "previously unclued" card (excluding known trash, already instructed plays, and already clued cards)
    - Gives PTD on the card to the right (exceptions with 4s)
    - If there is no card to the right, locks
- Good Touch Rank Clues
    - Reveals that a previously unknown card is playable from good touch. Does nothing extra
- Fix clues
    - Reveals that previously instructed/good-touch card is trash
- Trash Fill-in Clues
    - May touch new cards. Reveals that a previously clued card was unknwon trash
- Play Fill-in Clues
    - May touch new cards. Reveals that a previously clued card was an unknown play
- 8 Clue Stalls
    - Number touches rightmost and not slot 1. Gives PTD
- Locked hand first turn clues
    - Direct play on leftmost. May give PTD on slot 1
- Bomb lock. Bombing PTD or known trash triggers a lock
- Unlock Promise
    - With a locked hand, things get complicated


Per card knowledge
```rust
struct Note {
    play_queued: Option<PlayQueued>,
    permission_to_discard: bool,
    clued: bool,
}

struct PlayQueued {
    stacks: PlayStacks,
    // All already queued plays in own hand. All known plays in giver's hand that there would be time to play
    after: Vec<CardId>
}

```

How do we deal with plays that are stacked on top of plays in giver's hand?
Simple options:
- Never do it
- Superposition includes duplicate of first card in each suit played from giver's hand,
  as well as cards on top of each card played from giver's hand at time of clue before the card had permission to play
Complicated option: Pay attention to which plays are publicly known are for the non-public ones, consider what it would take from the hand to make them known
  - For each card, keep track of its useful-unplayable identities