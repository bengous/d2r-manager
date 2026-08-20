# d2r-tool : plan de projet

Ecrit le 2026-08-20. Statut : **brouillon a valider**. Une fois valide, ce document
fait foi : l'agent d'implementation s'y conforme et s'arrete devant toute divergence.

## 1. But

1. Sauvegarde et synchronisation des saves vers git (reprend le role du script
   `~/.local/bin/d2r-backup` ; la bascule effective est une decision utilisateur
   ulterieure, le script bash reste en place d'ici la).
2. Exploration hors jeu des inventaires, coffres personnels et coffres partages,
   via CLI d'abord, API HTTP locale ensuite, GUI plus tard.
3. Deplacement d'objets entre conteneurs (coffre partage, coffre personnel,
   inventaire, cube), dans les deux sens.
4. Duplication d'un objet possede d'un personnage vers un autre, tracee dans un
   registre de provenance.
5. Transactions : un plan d'operations valide puis applique, adosse a git.
6. Plus tard : lecture du Chronicle (Holy Grail) depuis l'onglet 7 du coffre moderne.

## Non-buts

- **Pas un HeroEditor.** Aucune creation d'objet, aucune edition de stats, aucun
  moyen d'obtenir un objet non possede. Le core n'expose pas ces operations.
- Pas de support des versions de format autres que 105 : erreur claire et refus.
- Pas de jeu en ligne, ladder, ou multijoueur.

## 2. Decisions figees

| Decision | Choix | Motif |
|---|---|---|
| Langage | Rust | surete bit-level, binaire unique, WASM possible pour une GUI future |
| Strategie format | portage depuis D2SSharp, halbu en reference pour le header | seule implementation lecture+ecriture v105 prouvee (round-trip octet a octet verifie le 2026-08-20 sur les fichiers reels) |
| d2rsavegameparser (Java) | oracle execute, jamais porte | LGPL 2.1 ; D2SSharp et halbu sont MIT |
| Architecture | hexagonale, imposee par le graphe de crates | violation = erreur de compilation |
| Git | sous-processus `git`, pas de bibliotheque | meme comportement que le script existant, zero dependance lourde |
| Transactions | plan JSON, apply adosse a des commits git | git est deja le mecanisme de sauvegarde et de rollback |
| Corpus de test | copie figee dans `fixtures/`, provenance tracee | reproductible ; rafraichissement par nouveau gel scripte |
| Golden files | generes par les deux oracles, commites | les tests Rust tournent sans .NET ni Java |
| GUI | reportee ; l'API HTTP fige le contrat | pas de decision prematuree |

## 3. Architecture

| Crate | Role | Depend de |
|---|---|---|
| `format` | octets vers modele et retour, v105 seul, pur, zero I/O | rien |
| `domain` | objets, grilles, regles, plans de transaction, pur | `format` |
| `app` | ports (traits) et cas d'usage : sync, plan, apply | `domain` |
| `cli` | adaptateurs (fs, git, processus) et binaire clap | `app` |

Regles :

1. `format` et `domain` ne touchent ni fichier, ni horloge, ni processus.
   Impose par `clippy.toml` : `disallowed-types` sur `std::fs`, `std::process`,
   `std::time::SystemTime` dans ces deux crates.
2. Les adaptateurs restent dans `cli` tant qu'il est le seul consommateur.
   Extraction en crate `infra` quand l'API HTTP arrive, pas avant.
3. Etats invalides irrepresentables : `Plan` puis `ValidatedPlan` puis
   `AppliedPlan` sont des types distincts ; une position de grille ne se
   construit que bornee ; les codes objet sont des newtypes.
4. Erreurs : `thiserror` dans les libs, `anyhow` dans `cli` seulement.
   Echec rapide, contexte propage, aucun silencieux.
5. Version differente de 105 : erreur explicite, pas de compatibilite.

Arborescence cible :

```
d2r-tool/
  Cargo.toml            # workspace
  rust-toolchain.toml   # 1.97.1
  justfile              # check, test, goldens
  lefthook.yml          # pre-commit = just check
  deny.toml             # licences MIT/Apache-2.0 seules, advisories
  crates/
    format/  domain/  app/  cli/
  tools/
    goldens-csharp/     # dumper JSON, reference le submodule D2SSharp
    goldens-java/       # dumper TSV + fetch du jar epingle par sha256
  fixtures/             # corpus fige, LECTURE SEULE  (fait, commit f891ca9)
  goldens/
    csharp/*.json  java/*.tsv  REPORT.md
  reference/            # submodules figes             (fait, commit 697db1c)
  docs/
    PLAN.md  FORMAT.md  adr/
```

## 4. Versions epinglees

Mesurees sur la machine le 2026-08-20, installees via mise.

| Outil | Version | Role |
|---|---|---|
| Rust | 1.97.1 (`rust-toolchain.toml`) | toolchain, edition 2024 |
| just | 1.44.0 | runner de commandes |
| lefthook | 2.1.4 | hook pre-commit |
| cargo-deny | 0.20.2 | licences et advisories |
| cargo-machete | 0.9.2 | dependances inutilisees |
| .NET SDK | 10.0.400 | oracle D2SSharp (tools/ seulement) |
| OpenJDK | 25.0.1 | oracle Java (tools/ seulement) |
| d2rsavegameparser.jar | 3.2.0, sha256 `0b2164f3c8219e390c42ccba3e2377dce2514561e1c32f5d0c1d4c9622960f89` | oracle Java, telecharge par script |

Dependances Rust : `clap`, `serde`, `serde_json`, `thiserror`, `anyhow` (cli),
`proptest` (dev). Versions prises au `cargo add` initial et figees par le
`Cargo.lock` commite.

Submodules figes : D2SSharp `f26f218` (MIT), d2rsavegameparser `de5dad9`
(LGPL 2.1), halbu `89bd5ff` (MIT).

Ces versions sont un point de depart, pas un gel definitif. Installer un nouvel
outil ou monter une version est permis quand c'est justifie : la mise a jour de
l'epingle (`rust-toolchain.toml`, `Cargo.lock`, ce tableau) se fait dans le meme
changement, jamais en silence.

## 5. Qualite et anti-slop

1. `rustfmt` + clippy pedantic en `-D warnings`, en CI et dans `just check`.
2. `dead_code` en deny ; cargo-machete ; cargo-deny (copyleft interdit dans les
   dependances Rust).
3. Zero commentaire, sauf references a la spec de la forme `FORMAT.md §x.y`
   dans le code du bitstream (contrainte externe irremplacable).
4. Regle d'or du depot : remplacer, ne pas accumuler. Pas de v2 sans suppression
   de la v1 dans le meme changement.
5. La qualite ne se mesure jamais aux compteurs (tests, LOC, couverture).

## 6. Politique de tests

Le harnais differentiel est la suite de tests de `format` :

1. **Goldens C# (JSON)** : la verite de portage. Dump complet du modele D2SSharp
   pour chacun des 55 fichiers v105. Les tests Rust comparent champ a champ.
2. **Goldens Java (TSV)** : contre-verification independante sur les champs
   communs : conteneur, code, position, pile, qualite, empreinte.
   Attention : les axes x/y du parseur Java sont inverses par rapport a D2SSharp
   (mesure le 2026-08-20, 135/135 objets concordants apres echange).
3. **Round-trip** : `write(read(f)) == f` octet pour octet sur les 55 fichiers.
4. **proptest** : sur le bitreader uniquement.
5. Regles de `domain` : tests par table. Ports : fakes ecrits main, aucun
   framework de mock. Fixtures `legacy/` : verifient l'erreur propre.

## 7. Transactions

1. `plan` : produit un JSON d'operations (move, duplicate), objets references
   par empreinte + conteneur source. N'ecrit rien.
2. Validation : grille (dimensions par code objet, table extraite des .txt du
   jeu), capacites, plafond d'or 2 500 000 par onglet, objets limites a un par
   personnage (Annihilus, Hellfire Torch, Gheed's Fortune), piles max 99.
   L'onglet specialise du coffre moderne n'est pas une grille : on n'y invente
   jamais de position, on empile ou on ajoute.
3. `apply` : jeu ferme (`pgrep D2R.exe`), timer en pause, commit git de point de
   retour, ecriture, relecture et verification d'invariant (le multiset d'objets
   est conserve hors operations explicites), commit final avec le plan dans le
   message, timer relance. Rollback = `git revert`.
4. Registre de provenance : chaque duplication journalisee (empreinte, source,
   cible, date) dans `d2r-saves:ledger/duplications.jsonl`, hors `saves/`,
   commite par `apply` dans le meme commit que l'operation.

## 8. Phases et criteres d'acceptation

| Phase | Contenu | Fait quand |
|---|---|---|
| 0 | dumpers oracles, goldens, rapport round-trip .d2s de l'oracle | goldens commites pour les 58 fichiers ; REPORT.md donne le statut round-trip par fichier ; fetch du jar scripte avec controle sha256 |
| 1 | spec `FORMAT.md` depuis le source D2SSharp + la recherche existante (`d2r-saves:docs/research/format-objet-v105.md`) | chaque champ du bitstream a sa largeur en bits et sa reference croisee |
| 2 | `format` lecture | 55/55 fichiers v105 concordent avec les goldens C# ; legacy/ produit l'erreur attendue |
| 3 | `format` ecriture | round-trip 55/55 octet pour octet ; checksum `.d2s` reproduit |
| 4 | `domain` : grilles, regles, plans | tests par table verts ; la fusion de coffres du 2026-08-20 rejouee donne un resultat equivalent (reference : commit `2885c71` de d2r-saves) |
| 5 | `cli` : sync, ls, find, diff | sync reproduit les gardes du script bash ; la bascule reste une decision utilisateur |
| 6 | transactions plan/apply | scenario complet sur copie du corpus, rollback verifie |
| 7 | API HTTP locale | contrat OpenAPI fige, memes cas d'usage que la CLI |

Chaque phase se termine par : criteres verifies, resume, arret. Toute extension
de perimetre se confirme avant execution.

## 9. Consignes a l'agent d'implementation

1. Lire ce plan et `fixtures/PROVENANCE.md` avant toute action.
2. Interdictions absolues : ecrire dans `~/Games/`, dans `~/Work/d2r-saves/`,
   dans `fixtures/` (lecture seule), dans `reference/` (submodules figes).
3. Deux echecs sur la meme piste : arret, resume, reevaluation.
4. Branches `feature/desc`, rebase, commits petits. Pas de push : le remote
   n'existe pas encore.
5. Divergence avec le plan = arret et question, pas d'initiative silencieuse.

## 10. Etat du depot au moment du gel

Commits : `9509293` init, `697db1c` submodules de reference, `f891ca9` corpus
(55 fichiers v105 + 3 legacy, provenance d2r-saves `33162cd`).

## 11. Questions ouvertes

1. **Nom du binaire CLI.** `d2r` est pris par le lanceur `~/.local/bin/d2r`.
   L'utilisateur choisit un nom de bapteme, en attente.
2. Remote GitHub prive : a creer quand l'utilisateur le decide.
