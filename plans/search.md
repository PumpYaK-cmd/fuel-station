# search: search and filter the board (card ea8a704d)

base: fbcb67179955f74d56b87acc10067be39f31af62

## Context
`src/ui/mod.rs` `view_board` renders all `self.board` cards per column through
`view_column`. `CardSummary` has `key`, `title`, `tier: Option<String>` (free text, usually
`T1`/`T2`), `human`, `holder: Option<Actor>`.

Rulings: one search box and one filter picker in the board header, left of the
Repair/Pick folder button. Search matches key or title, case-insensitive substring.
Filter options: `all`, `human`, `held` (has a holder), then one entry per distinct tier
present on the board (`tier T1`, ...). Both apply together. They persist across project
switches and reloads. `Ctrl+F` focuses the search box; `Esc` in the box clears it when no
detail is open.

## Design
`src/ui/mod.rs`:
- `App` gets `search: String`, `filter: Filter` where
  `#[derive(Debug, Clone, PartialEq, Eq)] enum Filter { All, Human, Held, Tier(String) }`
  with `Display` (`all`, `human`, `held`, `tier <t>`).
- Messages `Search(String)`, `FilterChanged(Filter)`, `FocusSearch`.
- `fn matches(&self, card: &CardSummary) -> bool`: free function
  `fn card_matches(card: &CardSummary, search: &str, filter: &Filter) -> bool` so it can
  be unit tested; `view_column` filters with it. The header count shows the filtered
  count; the gauge keeps using the unfiltered count (it measures column load).
- Header: `text_input("search", &self.search).id(SEARCH_ID).on_input(Message::Search)
  .style(theme::input).width(220)` and `pick_list(filters, Some(&self.filter),
  Message::FilterChanged).style(theme::picker)` where `filters` is built per view from
  the board (All, Human, Held, then sorted distinct tiers). If the selected
  `Tier(t)` is no longer on the board, still list it so the picker shows it.
- `FocusSearch` → `iced::widget::operation::focus(SEARCH_ID)` (iced 0.14 path; check the
  exact name in the iced source in `~/.cargo/registry` and use what exists).
- Subscription: the keyboard listener currently only runs when a detail is open. Make a
  listener always on for `Ctrl+F` → `FocusSearch`; keep the arrow/Esc mapping only when a
  detail is open (same behaviour as now for those keys). When no detail is open, `Esc`
  with a non-empty search → `Search(String::new())`.
- Tests: `#[cfg(test)] mod tests` at the bottom of `src/ui/mod.rs` for `card_matches`:
  key match, title case-insensitive, human filter, held filter, tier filter, search and
  filter combined.

## Verification
Gate. Manual for the user:
1. Type in search: columns shrink to matches, counts follow, gauges stay.
2. Filter human / held / a tier: only those cards; combined with search.
3. Ctrl+F focuses the box; Esc clears it; with a card open, Esc and arrows behave as before.
4. Switch project: search and filter stay applied.

## Out of scope
Searching card bodies or comments (needs a store query), saved filters.

Tier:  1 implementer. Why: view-only change, every edit named, pure matcher tested.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, manual list above. Why: view code has no tests beyond the matcher.
Scope: 1 file (ui/mod.rs: header, view_column, subscription, tests). Asks: none
