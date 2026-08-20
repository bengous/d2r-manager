---
paths:
  - "crates/format/**"
  - "crates/domain/**"
---

# Purete de format et domain

- Zero I/O, horloge, processus (PLAN §3.1) ; `clippy.toml` `disallowed-types`
  fait foi.
- `thiserror` ici, jamais `anyhow` (PLAN §3.4).
- Etats invalides irrepresentables : newtypes pour les codes objet, positions
  de grille bornees a la construction, `Plan` / `ValidatedPlan` /
  `AppliedPlan` types distincts (PLAN §3.3).
- `dead_code` en deny (PLAN §5.2).
