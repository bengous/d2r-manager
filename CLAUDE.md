# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Gestion de saves Diablo II Resurrected (offline, format v105) : sauvegarde vers
git, exploration hors jeu, deplacement et duplication d'objets par transaction.
Source de verite : `docs/PLAN.md`, avec `docs/FORMAT.md` pour le bitstream et
`fixtures/PROVENANCE.md` pour le corpus.

## Interdictions absolues

- Jamais ecrire dans `~/Games/`, `~/Work/d2r-saves/`, `fixtures/`, `reference/`.
- Jamais mettre a jour les submodules de `reference/`.
- Jamais porter du code depuis `reference/d2rsavegameparser` (LGPL 2.1) ni
  ouvrir son `src/` : jar execute et `txt/` seulement.
- Version de format autre que 105 : erreur explicite, aucun support.
- Binaire `d2r-manager`, jamais `d2r` (nom pris par le lanceur).
- Jamais de `git stash` ni de `git checkout --` : d'autres agents partagent ce
  checkout. Pour comparer avec main, `git show main:<fichier>`.

## Commandes

- `just check` : corpus sha256, fmt, clippy pedantic `-D warnings`, machete, deny.
- `just test` : `cargo test --workspace --all-features` (206 tests).
- Un seul test : `cargo test -p <crate> --test <fichier> <nom>`, par exemple
  `cargo test -p server --test contract`.
- `just goldens` : reconstruit les oracles et regenere les 120 golden files.
  Doit etre idempotent : `git diff --exit-code goldens/` apres coup.
- `just tables` : regenere les tables `v105.rs` de `format` et `domain`
  (`UPDATE_TABLES=1`), puis leur test-garde les revalide contre les `.txt`.

## Architecture

Six crates, dependances a sens unique, violation = erreur de compilation :
`format` <- `domain` <- `app` <- `infra` <- {`cli`, `server`}.

- `format` : octets vers modele et retour, v105 seul, pur. Chaque champ du
  bitstream a sa largeur en bits dans `docs/FORMAT.md`, citee dans le code par
  `FORMAT.md §x.y`, seule forme de commentaire autorisee du depot.
- `domain` : grilles, regles, empreintes d'objet, typestate
  `Plan` -> `ValidatedPlan` -> `AppliedPlan`. Pur.
- `app` : ports (traits) et cas d'usage. `app::transaction::prepare` porte
  lecture, resolution, validation ET projection a blanc ; `cli::plan`,
  `server::handler::plan` et `Transaction::apply` la partagent, sinon la porte
  du plan accepte ce que l'apply refuse.
- `infra` : les sept adaptateurs (fs, verrous, git par sous-processus, pgrep,
  systemctl, horloge). `cli` et `server` n'en portent aucun.
- `server` : API locale, loopback seul. Toute route est declaree dans
  `docs/openapi.json` ; `crates/server/tests/contract.rs` oppose le contrat au
  serveur, noms, types, requiredness et statuts compris.

Le harnais differentiel est le coeur des tests : `goldens/csharp/*.json` est le
corrige produit par l'oracle D2SSharp, `goldens/java/*.tsv` la contre-mesure.
Un round-trip octet a octet ne prouve rien seul, un champ lu au mauvais offset
et reecrit au meme offset le passe ; la concordance champ a champ avec les
goldens est ce qui prouve la comprehension. Un golden ne s'ajuste jamais pour
faire passer un test.

## Workflow

- Branches `feature/desc`, rebase, commits petits. Pas de push : pas de remote.
- `docs/PLAN.md`, `CLAUDE.md` et `.claude/rules/` ne se modifient pas sans
  demande explicite de l'utilisateur.
- Deux echecs sur la meme piste : arret, resume, remontee.
- Divergence avec `docs/PLAN.md` : question, jamais d'initiative silencieuse.
- Questions ouvertes en attente de decision : `docs/PLAN.md` §11.

## Regles

- `.claude/rules/bitstream.md` : spec FORMAT.md et crate `format`.
- `.claude/rules/purity.md` : purete de `format` et `domain`.
- `.claude/rules/oracles.md` : outils goldens et licences.
- `.claude/rules/adapters.md` : `app`, `infra`, `cli`, `server`.
- `.claude/rules/review.md` : protocole de review a contexte frais.
- `.claude/rules/rust.md` : qualite Rust, types d'abord.
