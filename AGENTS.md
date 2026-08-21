# D2R Manager

Diablo II Resurrected save management (offline, v105 format): backup to git,
out-of-game exploration, item move and duplication by transaction. Source of
truth: the code and its tests, with `docs/FORMAT.md` for the bitstream,
`docs/openapi.json` for the API and `fixtures/PROVENANCE.md` for the corpus.
`docs/PLAN.md` is the original brief, archived: it does not describe the
current state.

## Absolute prohibitions

- Never write into `~/Games/`, `~/Work/d2r-saves/`, `fixtures/`, `reference/`.
- Never update the submodules of `reference/`.
- Never port code from `reference/d2rsavegameparser` (LGPL 2.1) and never open
  its `src/`: executed jar and `txt/` only.
- Format version other than 105: explicit error, no support.
- Binary `d2r-manager`, never `d2r` (the launcher takes that name).
- Never `git stash` and never `git checkout --`: other agents share this
  checkout. To compare with main, `git show main:<file>`.

## Commands

- `just check`: corpus sha256, fmt, clippy pedantic `-D warnings`, machete, deny.
- `just test`: `cargo test --workspace --all-features`.
- A single test: `cargo test -p <crate> --test <file> <name>`, for example
  `cargo test -p server --test contract`.
- `just goldens`: rebuilds the oracles and regenerates the goldens, one file
  per fixture and per oracle. Must be idempotent:
  `git diff --exit-code goldens/` afterwards.
- `just tables`: regenerates the `v105.rs` tables of `format` and `domain`
  (`UPDATE_TABLES=1`), then their guard test revalidates them against the
  `.txt`.

## Architecture

Six crates, one-way dependencies, a violation is a compile error:
`format` <- `domain` <- `app` <- `infra` <- {`cli`, `server`}.

- `format`: bytes to model and back, v105 only, pure. The bitstream is
  specified field by field in `docs/FORMAT.md`.
- `domain`: grids, rules, item fingerprints, typestate
  `Plan` -> `ValidatedPlan` -> `AppliedPlan`. Pure.
- `app`: ports (traits) and use cases: sync, inspection, transactions. Pure.
- `infra`: the adapters (fs, locks, git by subprocess, pgrep, systemctl,
  clock).
- `server`: local API, loopback only, contract `docs/openapi.json`.

The differential harness is the core of the tests: `goldens/csharp/*.json` is
the answer key of the D2SSharp oracle, `goldens/java/*.tsv` the cross-check. A
golden is never adjusted to make a test pass.

## Workflow

- Atomic commits.
- Change of business rule or of scope: ask the user, never a silent
  initiative.
- Open questions waiting for a decision: `docs/DECISIONS.md`. An agent does
  not settle them alone.

## Keeping these docs honest

`AGENTS.md` and `.claude/rules/` are living documents: when your change makes a
rule stale, fix the doc in the same commit; when a new convention crystallizes,
add a rule. They guide upfront — never let them re-explain code that should
document itself.
