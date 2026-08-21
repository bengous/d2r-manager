---
paths:
  - "crates/format/**"
  - "docs/FORMAT.md"
---

# Bitstream and FORMAT.md spec

- Zero comments, except the references of the form `FORMAT.md §x.y` in the
  bitstream code: the only comment form the repository allows.
- Every field width comes from `docs/FORMAT.md`, never from a local magic
  number. Code that contradicts the spec: fix the spec in the same commit.
- The goldens are the answer key: never adjust them to make a test pass. The
  byte round-trip alone proves nothing, a field read at the wrong offset and
  rewritten at the same offset passes it. The proof is the field-by-field
  match (`tests/project/`).
- Save version other than 105: explicit error, no compatibility.
- `proptest` on the bitreader (`src/bits.rs`) only.
- Axes: `InvX` = column, `InvY` = row, D2SSharp convention. This rule is the
  home of that convention for the whole repository.
