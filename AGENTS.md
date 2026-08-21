# D2R Manager

Gestion de saves Diablo II Resurrected (offline, format v105) : sauvegarde vers
git, exploration hors jeu, deplacement et duplication d'objets par transaction.
Source de verite : le code et ses tests, avec `docs/FORMAT.md` pour le
bitstream, `docs/openapi.json` pour l'API et `fixtures/PROVENANCE.md` pour le
corpus. `docs/PLAN.md` est le brief d'origine, archive : il ne decrit pas
l'etat courant.

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
- `just test` : `cargo test --workspace --all-features`.
- Un seul test : `cargo test -p <crate> --test <fichier> <nom>`, par exemple
  `cargo test -p server --test contract`.
- `just goldens` : reconstruit les oracles et regenere les goldens, un fichier
  par fixture et par oracle. Doit etre idempotent :
  `git diff --exit-code goldens/` apres coup.
- `just tables` : regenere les tables `v105.rs` de `format` et `domain`
  (`UPDATE_TABLES=1`), puis leur test-garde les revalide contre les `.txt`.

## Architecture

Six crates, dependances a sens unique, violation = erreur de compilation :
`format` <- `domain` <- `app` <- `infra` <- {`cli`, `server`}.

- `format` : octets vers modele et retour, v105 seul, pur. Le bitstream est
  specifie champ par champ dans `docs/FORMAT.md`.
- `domain` : grilles, regles, empreintes d'objet, typestate
  `Plan` -> `ValidatedPlan` -> `AppliedPlan`. Pur.
- `app` : ports (traits) et cas d'usage : sync, inspection, transactions. Pur.
- `infra` : les adaptateurs (fs, verrous, git par sous-processus, pgrep,
  systemctl, horloge).
- `server` : API locale, loopback seul, contrat `docs/openapi.json`.

Le harnais differentiel est le coeur des tests : `goldens/csharp/*.json` est le
corrige de l'oracle D2SSharp, `goldens/java/*.tsv` la contre-mesure. Un golden
ne s'ajuste jamais pour faire passer un test.

## Workflow

- Commits atomiques.
- Changement de regle metier ou de perimetre : question a l'utilisateur, jamais
  d'initiative silencieuse.
- Questions ouvertes en attente de decision : `docs/DECISIONS.md`. Un agent ne
  les tranche pas seul.

## Keeping these docs honest

`AGENTS.md` and `.claude/rules/` are living documents: when your change makes a
rule stale, fix the doc in the same commit; when a new convention crystallizes,
add a rule. They guide upfront — never let them re-explain code that should
document itself.
