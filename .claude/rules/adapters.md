---
paths:
  - "crates/app/**"
  - "crates/cli/**"
---

# app et cli

- `anyhow` dans `cli` seulement ; les libs restent en `thiserror` (PLAN §3.4).
- git par sous-processus `git`, pas de bibliotheque (PLAN §2).
- Les adaptateurs restent dans `cli` jusqu'a l'API HTTP ; pas de crate `infra`
  avant (PLAN §3.2).
- Fakes ecrits main pour les ports ; aucun framework de mock (PLAN §6.5).
- Chemins injectes partout ; tests et scenarios sur repertoires temporaires
  uniquement, jamais sur `~/Games/` ni `~/Work/d2r-saves/`.
