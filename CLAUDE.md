# d2r-manager

Gestion de saves Diablo II Resurrected (offline, format v105). Source de
verite : `docs/PLAN.md`, a lire avant toute action, avec `fixtures/PROVENANCE.md`.

## Interdictions absolues

- Jamais ecrire dans `~/Games/`, `~/Work/d2r-saves/`, `fixtures/`, `reference/`.
- Jamais mettre a jour les submodules de `reference/`.
- Jamais porter du code depuis `reference/d2rsavegameparser` (LGPL 2.1) ni
  ouvrir son `src/` : jar execute et `txt/` seulement.
- Version de format autre que 105 : erreur explicite, aucun support.
- Binaire `d2r-manager`, jamais `d2r` (nom pris par le lanceur).

## Commandes

`just check`, `just test`, `just goldens` ;
integrite du corpus : `cd fixtures && sha256sum -c sha256sums.txt`.

## Workflow

- Branches `feature/desc`, rebase, commits petits. Pas de push : pas de remote.
- `docs/PLAN.md`, `CLAUDE.md`, `.claude/rules/` appartiennent a
  l'orchestrateur : ne pas les modifier.
- Deux echecs sur la meme piste : arret, resume, remontee a l'orchestrateur.
- Divergence avec `docs/PLAN.md` : question, jamais d'initiative silencieuse.

## Regles

- `.claude/rules/bitstream.md` : spec FORMAT.md et crate `format`.
- `.claude/rules/purity.md` : purete de `format` et `domain`.
- `.claude/rules/oracles.md` : outils goldens et licences.
- `.claude/rules/adapters.md` : `app`, `cli`, adaptateurs.
- `.claude/rules/review.md` : protocole de review des lots.
- `.claude/rules/rust.md` : qualite Rust, types d'abord.
