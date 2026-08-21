---
paths:
  - "crates/domain/**"
---

# Domain rules

Facts measured on the corpus, not conventions: changing them breaks real
files.

- The `AdvancedStash` tab is not a grid: the files the game writes carry
  overlapping positions in it. We stack onto an entry of the same fingerprint
  as long as room remains, otherwise we append and keep the position of the
  incoming item. No position is invented: `Destination::Pile` has no cell
  field.
- The `AdvancedStash` stack field is optional per item and enters the
  fingerprint: grid items carry it absent, tab items carry it present. A move
  from a grid to the tab therefore changes the identity of the item and is
  refused at projection (`app::world`). The stack to stack, stack to grid and
  grid to grid directions stay allowed.
- Stack entries with 0 units exist in the real files: `PileCount` runs from 0
  to 99 for what an entry carries, `PileUnits` from 1 to 99 for what an
  operation moves. An entry at 0 is read, not moved.
- `carry1` of the `.txt` is a group identifier, not a boolean: 4 groups,
  including the Sunder charms (`cs2`), absent from the usual list (Annihilus,
  Hellfire Torch, Gheed's Fortune). The "one per character" rule applies to
  the group.
- Gold does not move: `Plan` carries only `move` and `duplicate`, and `Census`
  excludes gold, so the conservation invariant does not see it. The only
  safety net is `infra/tests/transaction.rs`, test
  `no_operation_moves_a_single_piece_of_gold`, which reads the gold of the
  tabs and the `PlayerStats` from the files. A gold transfer operation is an
  open decision, see `docs/DECISIONS.md`.
- A "quantity <= maxstack" rule would refuse real files: `ama` carries 180
  where the `.txt` announce 120. The 99 cap targets the stacks of the
  `AdvancedStash` tab, not the `quantity` field.
- `Plan` -> `ValidatedPlan` -> `AppliedPlan` are three distinct types, none
  derives `Clone`, and `validate` consumes the vault: rebuild the vault from
  the files to retry after a refusal.
- The geometry comes from the `.txt` through a generated and committed table
  (`src/tables/v105.rs`), revalidated by `tests/tables.rs`; `just tables`
  regenerates it. Never a hand-written dimension.
