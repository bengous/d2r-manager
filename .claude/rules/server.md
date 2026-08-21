---
paths:
  - "crates/server/**"
  - "docs/openapi.json"
---

# Local HTTP API

- `docs/openapi.json` is the frozen contract: it freezes the interface for a
  future GUI. Every route, property, query parameter or status added to the
  server is declared in the contract, and the reverse.
- `tests/contract.rs` checks the contract against the server: names, types,
  requiredness, nested schemas, query parameters, inline keywords and
  statuses. When it fails, move the guard toward the contract, never the
  contract toward the guard.
- The parameter names live in the `ROUTES` table: a name added on the server
  side without the contract makes the suite fail. That is intended.
- Loopback listen only. A foreign `Host` returns 403: this is a DNS rebinding
  defense, not authentication.
- No operation that the CLI does not expose; the parity is locked in both
  directions by `x-cli` and `cli/tests/commands.rs`. No item creation, nothing
  that bypasses `domain`: this project is not a character editor.
- Errors typed by business family; never a 500 for a rule refusal.
- The server handles one request at a time: two simultaneous `apply` are
  serialized. No lock is taken, see `docs/DECISIONS.md`.
