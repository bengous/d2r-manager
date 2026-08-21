---
paths:
  - "crates/app/**"
  - "crates/infra/**"
  - "crates/cli/**"
---

# app, infra et cli

- Les adaptateurs vivent dans `infra` ; `cli` et `server` les consomment et
  n'en portent aucun (PLAN §3.2).
- git par sous-processus `git`, pas de bibliotheque (PLAN §2).
- Fakes ecrits main pour les ports ; aucun framework de mock (PLAN §6.5).
- Chemins injectes partout ; tests et scenarios sur repertoires temporaires
  uniquement, jamais sur `~/Games/` ni `~/Work/d2r-saves/`.
- `app::transaction::prepare` porte lecture, resolution, validation ET
  projection a blanc. `cli::plan`, `server::handler::plan` et
  `Transaction::apply` la partagent : un appelant qui enchaine `resolve` puis
  `validate` sans projeter rouvre la porte du plan sur un plan inapplicable.
- `sync` reproduit les gardes de `~/.local/bin/d2r-backup` (lecture seule),
  une par test dans `app/tests/guards.rs`. Le push reste hors perimetre.
- Sequence d'`apply` : PLAN §7.3, verbatim, timer en pause puis relance,
  commit de point de retour, invariant relu, rollback par `git revert`.
