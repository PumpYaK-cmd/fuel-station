# Import: number cards by creation date (card a2465c51)

base: 9b02f45770e9611c7083883374aba1e53b0679a6

## Context
`Store::import_board` (`src/store/mod.rs`, ~line 147) assigns `number = i + 1` in
`board.cards` order, which is (list pos, card pos): Done first. Card numbers should follow
creation, so FS-1 is the oldest card.

## Design
- In `import_board`, before the insert loop: build `order: Vec<usize>` = indices of
  `board.cards` sorted by `(created_at, index)` (stable), then `numbers[idx] = rank + 1`.
  Insert loop unchanged except `number = numbers[i]`. Positions keep the current per-column
  counter in `board.cards` order.
- No other file changes besides the test.

## Tests
Extend the existing `import_board` store test (or add one in `src/store/tests.rs`): three
cards given in order Done(created 300), Backlog(created 100), Ready(created 200) -> numbers
3, 1, 2; positions 0 in each column.

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

Tier:  1 implementer. Why: one loop change with an exact spec and test.
Gate:  cargo fmt/clippy/test. Why: fully covered.
Scope: src/store/mod.rs, src/store/tests.rs. Asks: none
