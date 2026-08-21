---
paths:
  - "tools/**"
  - "goldens/**"
---

# Oracles and goldens

- d2rsavegameparser (LGPL 2.1): executed jar and `txt/` only; never open and
  never port `reference/d2rsavegameparser/src/`. D2SSharp and halbu are MIT,
  porting from them is allowed.
- Jar downloaded by `tools/goldens-java/fetch-jars.sh` with mandatory sha256
  check, never committed.
- Canonical JSON: sorted keys, indented. `just goldens` must be idempotent.
- The Java dumper swaps x and y relative to D2SSharp: normalize every output
  onto the convention of the model, described in
  `.claude/rules/bitstream.md`.
- C#/Java disagreements listed in `goldens/REPORT.md`, never hidden.
- The corpus is frozen: `fixtures/sha256sums.txt` gives its list and its exact
  content. The `v105/windows/` files go into a separate section of the REPORT,
  outside the match criteria of `v105/linux/`.
