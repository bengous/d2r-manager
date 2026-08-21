---
paths:
  - "crates/app/**"
  - "crates/infra/**"
  - "crates/cli/**"
---

# app, infra and cli

- The adapters live in `infra`; `cli` and `server` consume them and carry
  none. The ports are the traits of `app::ports`.
- git through the `git` subprocess, no library: same behavior as the original
  bash script, zero heavy dependency.
- Hand-written fakes for the ports, in `app/tests/support/`; no mock
  framework.
- Paths injected everywhere; tests and scenarios on temporary directories
  only, never on `~/Games/` or `~/Work/d2r-saves/`.
- `app::transaction::prepare` carries read, resolution, validation AND
  dry-run projection. `cli::plan`, `server::handler::plan` and
  `Transaction::apply` share it: a caller that chains `resolve` then
  `validate` without projecting reopens the plan gate on an inapplicable plan.
- `sync` reproduces the guards of the `~/.local/bin/d2r-backup` script (read
  only), one per test in `app/tests/guards.rs`. The push stays out of scope.
- `apply` sequence, in this order: game closed (`app::sync::GAME_PROCESS`),
  timer paused, restore point commit, write, read back and check of the
  conservation invariant, final commit carrying the plan in the message, timer
  restarted. Rollback = `git revert`. The full scenario and the byte-for-byte
  revert are held by `infra/tests/transaction.rs`.
- `apply` takes no lock, while `sync` takes one through the `Locks` port: open
  decision, see `docs/DECISIONS.md`.
