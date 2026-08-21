# d2r-manager : plan de projet

Ecrit et fige le 2026-08-20. Ce document est le brief complet du projet : un
agent d'implementation le prend et construit l'outil en entier. Toute divergence
avec ce document se regle par une question a l'utilisateur, pas par une
initiative silencieuse.

Binaire CLI : `d2r-manager`. Ne pas installer sous le nom `d2r`, pris par le
lanceur `~/.local/bin/d2r`.

## 1. But

1. Sauvegarde et synchronisation des saves vers git (reprend le role du script
   `~/.local/bin/d2r-backup` ; la bascule effective est une decision utilisateur,
   le script bash reste en place d'ici la).
2. Exploration hors jeu des inventaires, coffres personnels et coffres partages,
   via CLI d'abord, API HTTP locale ensuite, GUI plus tard.
3. Deplacement d'objets entre conteneurs (coffre partage, coffre personnel,
   inventaire, cube), dans les deux sens.
4. Duplication d'un objet possede d'un personnage vers un autre, tracee dans un
   registre de provenance.
5. Transactions : un plan d'operations valide puis applique, adosse a git.
6. Plus tard : lecture du Chronicle (Holy Grail) depuis l'onglet 7 du coffre
   moderne. Hors perimetre de la premiere livraison.

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
d2r-manager/
  Cargo.toml            # workspace
  rust-toolchain.toml   # 1.97.1
  justfile              # check, test, goldens
  lefthook.yml          # pre-commit = just check
  deny.toml             # licences permissives : MIT/Apache-2.0 + exceptions nommees (Unicode-3.0), advisories
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
| slf4j-api | 2.0.17, sha256 `7b751d952061954d5abfed7181c1f645d336091b679891591d63329c622eb832` | dependance `provided` de l'oracle Java, telechargee par script |

Dependances Rust : `clap`, `serde`, `serde_json`, `thiserror`, `anyhow` (cli),
`jiff` 0.2.32 (cli, horodatage ISO-8601 local du commit de sauvegarde),
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
4. Remplacer, ne pas accumuler : pas de v2 sans suppression de la v1 dans le
   meme changement.
5. La qualite ne se mesure jamais aux compteurs (tests, LOC, couverture).

## 6. Tests : le harnais differentiel

Le projet porte un parseur de bitstream depuis C# vers Rust. Un round-trip
(`write(read(f)) == f`) ne prouve pas la comprehension : un champ lu au mauvais
offset et reecrit au meme mauvais offset passe le test, et l'outil affiche
ensuite des valeurs fausses. Les golden files sont le corrige : la comprehension
des oracles, figee, contre laquelle les tests Rust comparent champ a champ.

Composants du harnais :

1. `tools/goldens-csharp/` : projet .NET console. Entree, un fichier de save ;
   sortie, un JSON canonique (cles triees, indente) du modele D2SSharp complet,
   plus le verdict round-trip. Referencer
   `reference/D2SSharp/src/D2SSharp/D2SSharp.csproj` par `ProjectReference`.
2. `tools/goldens-java/` : lecture seule via le jar 3.2.0, telecharge par script
   avec controle sha256 obligatoire (tableau §4), jamais commite. Le dossier
   `txt/` vient de `reference/d2rsavegameparser/txt/`. Sortie TSV : conteneur,
   code, position, pile, qualite, empreinte.
3. `goldens/csharp/*.json` et `goldens/java/*.tsv` : un fichier par fixture,
   commites, pour les 60 fichiers du corpus (55 `v105/linux/`,
   2 `v105/windows/`, 3 `legacy/` ; `Settings.json` hors corpus).
   `goldens/REPORT.md` : par fixture, taille, version, nombre d'objets,
   verdict round-trip C#, concordance C#/Java ; les fichiers `v105/windows/`
   dans une section a part. Les desaccords sont listes, jamais masques.
4. Tests Rust de `format` : egalite champ a champ avec les goldens C# sur les
   55 fichiers `v105/linux/` ; round-trip octet pour octet 55/55 ; `proptest`
   sur le bitreader uniquement ; `legacy/` produit l'erreur attendue. Les
   2 fichiers `v105/windows/` sont hors criteres 55/55, couverts par les
   goldens et le REPORT.
5. Regles de `domain` : tests par table. Ports : fakes ecrits main, aucun
   framework de mock. Jamais d'objectif de couverture.

Verites mesurees le 2026-08-20, a integrer sans les redecouvrir :

- Les axes du parseur Java sont inverses par rapport a D2SSharp : 135/135
  objets concordants apres echange de x et y. Normaliser toutes les sorties sur
  la convention D2SSharp (`InvX` = colonne, `InvY` = ligne) et le documenter.
- L'onglet `AdvancedStash` du coffre moderne n'est pas une grille : les fichiers
  ecrits par le jeu y contiennent des positions qui se recouvrent. Ce n'est pas
  une anomalie. On n'y invente jamais de position : on empile ou on ajoute.
- Le champ de pile `AdvancedStash` est un bit optionnel par objet et entre dans
  l'empreinte : les objets de grille le portent absent, ceux de l'onglet le
  portent present (mesure 2026-08-21). Un deplacement d'une grille vers un
  onglet `AdvancedStash` changerait donc l'identite de l'objet : il est refuse
  a la projection, avant toute ecriture. Les sens pile vers pile, pile vers
  grille et grille vers grille restent permis.
- Round-trip `.d2i` : prouve 4/4 sur les fichiers v105. Round-trip `.d2s` :
  jamais mesure. Le mesurer via l'oracle C# et le documenter fichier par
  fichier dans `REPORT.md` ; un echec partiel est une information, pas un
  blocage.
- Fixtures `legacy/` : l'oracle Java doit les refuser (v105 exclusif). D2SSharp
  les lit sauf la v98 (`JewelsCraft.d2s`), refusee faute de donnees txt v98
  dans l'oracle (mesure 2026-08-20). Consigner les comportements mesures.
- Reperes anti-regression, fixture `v105/linux/ModernSharedStashSoftCoreV2.d2i` :
  7 onglets (5 normaux, 1 `AdvancedStash`, 1 `Chronicle`), 1533 objets en
  comptant les piles (227 entrees d'objet, somme des piles `AdvancedStash`
  1306), 1 075 411 pieces d'or a l'onglet 1, onglet `Chronicle` de 2298 octets.

## 7. Transactions

1. `plan` : produit un JSON d'operations (move, duplicate), objets references
   par empreinte + conteneur source. N'ecrit rien.
2. Validation : grille (dimensions par code objet, table extraite des .txt du
   jeu), capacites, plafond d'or 2 500 000 par onglet, objets limites a un par
   personnage (Annihilus, Hellfire Torch, Gheed's Fortune), piles max 99.
3. `apply` : jeu ferme (`pgrep D2R.exe`), timer en pause, commit git de point de
   retour, ecriture, relecture et verification d'invariant (le multiset d'objets
   est conserve hors operations explicites), commit final avec le plan dans le
   message, timer relance. Rollback = `git revert`.
4. Registre de provenance : chaque duplication journalisee (empreinte, source,
   cible, date) dans `d2r-saves:ledger/duplications.jsonl`, hors du miroir
   rsync, commite par `apply` dans le meme commit que l'operation.

## 8. Ordre de construction

L'agent avance de bout en bout, sans validation intermediaire de l'utilisateur.
L'ordre existe parce que chaque etape s'appuie sur la precedente. Les criteres
sont la definition de « fait ».

| Etape | Contenu | Fait quand |
|---|---|---|
| 1 | harnais : dumpers oracles, goldens, REPORT | goldens commites pour les 60 fichiers du corpus ; REPORT complet ; `just goldens` regenere tout |
| 2 | spec `FORMAT.md` depuis le source D2SSharp + `d2r-saves:docs/research/format-objet-v105.md` | chaque champ du bitstream a sa largeur en bits et sa reference croisee |
| 3 | `format` lecture | 55/55 `v105/linux/` concordent avec les goldens C# ; legacy en erreur propre |
| 4 | `format` ecriture | round-trip 55/55 octet pour octet ; checksum `.d2s` reproduit |
| 5 | `domain` : grilles, regles, plans | tests par table verts ; la fusion de coffres du 2026-08-20 rejouee donne un resultat equivalent (reference : commit `2885c71` de d2r-saves) |
| 6 | `cli` : sync, ls, find, diff | sync reproduit les gardes du script bash ; la bascule reste une decision utilisateur |
| 7 | transactions plan/apply | scenario complet sur copie du corpus, rollback verifie |
| 8 | API HTTP locale | contrat OpenAPI fige, memes cas d'usage que la CLI |

Le projet est livre quand tous les criteres passent. Fin = resume complet et
honnete : ce qui passe, ce qui a ete decouvert, ce qui reste.

## 9. Consignes a l'agent d'implementation

1. Lire ce plan et `fixtures/PROVENANCE.md` avant toute action.
2. Interdictions absolues : ecrire dans `~/Games/`, dans `~/Work/d2r-saves/`,
   dans `fixtures/` (lecture seule), dans `reference/` (submodules figes, ne pas
   les mettre a jour).
3. Ne jamais porter de code depuis `reference/d2rsavegameparser` (LGPL 2.1) :
   oracle execute uniquement. D2SSharp et halbu sont MIT, portage permis.
4. Deux echecs sur la meme piste : arret, resume, reevaluation.
5. Branches `feature/desc`, rebase, commits petits. Pas de push : le remote
   n'existe pas encore.
6. Blocage reel ou divergence avec le plan : question a l'utilisateur.

## 10. Etat du depot au moment du gel

Commits : `9509293` init, `697db1c` submodules de reference, `f891ca9` corpus
(55 fichiers v105 + 3 legacy, provenance d2r-saves `33162cd`), puis les commits
de documentation dont ce plan.

## 11. Questions ouvertes

1. Remote GitHub prive : a creer quand l'utilisateur le decide.
2. `apply` ecrit dans le miroir du depot de sauvegarde, pas dans `~/Games/`.
   Le retour des fichiers vers le repertoire du jeu reste a decider, comme la
   bascule du script bash.
3. Verrou d'`apply` : la sequence §7.3 met le timer en pause mais ne prend
   aucun verrou, donc une sauvegarde deja en vol peut ecraser une ecriture.
   Le port de verrou existe deja ; l'ajout est une decision utilisateur.
4. Transfert d'or entre onglets : §7.1 ne prevoit que `move` et `duplicate`,
   donc aucun plan ne deplace d'or. La fusion du 2026-08-20 en a pourtant
   deplace 940 000. Une troisieme operation reste a decider par l'utilisateur ;
   hors perimetre tant qu'elle n'est pas decidee. Le plafond de 2 500 000 par
   onglet reste une regle de construction dans `domain`.
