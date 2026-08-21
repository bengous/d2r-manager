---
paths:
  - "crates/app/**"
  - "crates/infra/**"
  - "crates/cli/**"
---

# app, infra et cli

- Les adaptateurs vivent dans `infra` ; `cli` et `server` les consomment et
  n'en portent aucun. Les ports sont les traits de `app::ports`.
- git par sous-processus `git`, pas de bibliotheque : meme comportement que le
  script bash d'origine, zero dependance lourde.
- Fakes ecrits main pour les ports, dans `app/tests/support/` ; aucun
  framework de mock.
- Chemins injectes partout ; tests et scenarios sur repertoires temporaires
  uniquement, jamais sur `~/Games/` ni `~/Work/d2r-saves/`.
- `app::transaction::prepare` porte lecture, resolution, validation ET
  projection a blanc. `cli::plan`, `server::handler::plan` et
  `Transaction::apply` la partagent : un appelant qui enchaine `resolve` puis
  `validate` sans projeter rouvre la porte du plan sur un plan inapplicable.
- `sync` reproduit les gardes du script `~/.local/bin/d2r-backup` (lecture
  seule), une par test dans `app/tests/guards.rs`. Le push reste hors
  perimetre.
- Sequence d'`apply`, dans cet ordre : jeu ferme (`app::sync::GAME_PROCESS`),
  timer en pause, commit de point de retour, ecriture, relecture et
  verification de l'invariant de conservation, commit final portant le plan
  dans le message, timer relance. Rollback = `git revert`. Le scenario complet
  et le revert octet a octet sont tenus par `infra/tests/transaction.rs`.
- `apply` ne prend aucun verrou, alors que `sync` en prend un par le port
  `Locks` : decision ouverte, voir `docs/DECISIONS.md`.
