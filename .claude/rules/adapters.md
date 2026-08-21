---
paths:
  - "crates/app/**"
  - "crates/cli/**"
  - "crates/infra/**"
  - "crates/server/**"
---

# app, infra, cli et server

- `anyhow` dans `cli` seulement ; les libs restent en `thiserror` (PLAN §3.4).
- git par sous-processus `git`, pas de bibliotheque (PLAN §2).
- Les adaptateurs vivent dans `infra` ; `cli` et `server` les consomment et
  n'en portent aucun (PLAN §3.2).
- Le serveur ecoute sur loopback seulement. Toute route ajoutee est declaree
  dans `docs/openapi.json` : le test de contrat refuse la divergence.
- Fakes ecrits main pour les ports ; aucun framework de mock (PLAN §6.5).
- Chemins injectes partout ; tests et scenarios sur repertoires temporaires
  uniquement, jamais sur `~/Games/` ni `~/Work/d2r-saves/`.
