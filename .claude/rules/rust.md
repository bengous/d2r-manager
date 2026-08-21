---
paths:
  - "crates/**"
---

# Rust quality

- Invalid states unrepresentable everywhere a datum has more than one
  legitimate shape: enums in place of correlated bools, newtypes for the
  primitives that carry rules, typestate for the ordered operations.
- Precise ownership and borrows: the APIs take what they consume; no
  convenience clone.
- `thiserror` with precise variants in the libraries; `anyhow` only in the
  `cli` and `server` binaries.
- `format`, `domain` and `app` stay pure: zero I/O, clock, process. Every pure
  crate carries its own `clippy.toml` `disallowed-types`; a new pure crate
  without that file is guarded by nothing.
- `dead_code` is set to deny: a public API with no caller is deleted.
