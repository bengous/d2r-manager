<!-- session_id: 89f37d10-304f-4d46-ab02-a9f3c47d9776 -->
# d2r-manager : plan d'orchestration multi-agents

## Contexte

Le depot est en phase 0 : `docs/PLAN.md` (brief fige, main = `d6bde58`),
`fixtures/` (60 fichiers de save, lecture seule), `reference/` (3 submodules
figes), zero code Rust. PLAN.md prevoyait un agent unique de bout en bout.
Nouveau mode d'execution : cette session (Fable) orchestre et ne code pas.
Elle pose les regles, redige les briefs, lance des sous-agents Opus autonomes,
gate chaque lot, integre, escalade. Chaque sous-agent spawne lui-meme un
reviewer Opus a contexte frais avant de declarer son lot fini. Peu de lots,
briefs riches : chaque frontiere de lot est un artefact auto-porteur (goldens,
spec, API de crate), jamais une comprehension a transmettre.

## Decisions utilisateur (2026-08-20)

1. Fixtures : goldens generes pour les 60 fichiers. Criteres 55/55 = linux.
   Les 2 fichiers windows sont documentes a part dans `REPORT.md`.
2. Rythme : bout en bout, sans pause utilisateur entre les lots. Rapport par
   lot. Intervention utilisateur sur blocage ou divergence seulement.
3. Review : 100 % Claude. Pas de review croisee Codex.
4. Modeles : sous-agents et reviewers = Opus (`model: "opus"`,
   `subagent_type: "claude"`). Orchestrateur = cette session Fable.

Verifications faites en amont : `~/Work/d2r-saves/docs/research/format-objet-v105.md`
existe ; `~/Work/d2r-saves/ledger/` n'existe pas (`apply` le creera a
l'execution par l'utilisateur) ; commits d2r-saves `2885c71` et `33162cd`
existent ; `~/.local/bin/d2r-backup` = 89 lignes ; D2SSharp ~7800 lignes C#
(`Item.cs` 1194, `D2SaveOverlay.cs` 685, BitReader/BitWriter ~500, Huffman,
checksum) ; halbu ~7400 lignes Rust centre header, `items/` quasi vide ;
corpus reel = 55 saves linux v105 + 2 windows v105 + 3 legacy (+
`Settings.json`, hors corpus) ; outils mise en place : just 1.44.0,
lefthook 2.1.4, cargo-deny, cargo-machete, dotnet, java 25.

## Mecanique d'execution

- Sequentiel strict : lot 0 puis lots 1 a 5. Pas de worktrees : une branche
  `feature/<lot>` par lot dans le checkout principal.
- Les briefs sont les prompts des sous-agents, composes par l'orchestrateur au
  lancement selon les recettes ci-dessous. Ils ne sont pas commites : le
  commit `d6bde58` a supprime le decoupage en briefs, PLAN.md reste le seul
  brief commite. Reprise apres incident : meme recette, re-composee depuis ce
  plan + branche + FORMAT.md + git log.
- Continuation d'un agent (checkpoint du lot 4, relance apres findings) :
  `SendMessage` vers l'agent existant, contexte intact.
- Chaque brief se termine par le bloc protocole de review (§ Review).
- L'orchestrateur ne corrige jamais lui-meme un echec de gate : retour au
  sous-agent avec le constat exact.

## Lot 0 : cadre (orchestrateur, aucun sous-agent)

1. Amendement minimal de `docs/PLAN.md` : corpus 60 fichiers, criteres 55/55
   precises « linux », windows documentes a part (decision utilisateur 1).
   Un commit `docs(plan)`.
2. `CLAUDE.md` racine, <= 25 lignes, index :
   - Contexte 1 ligne + « Source de verite : `docs/PLAN.md`, a lire avant
     toute action, avec `fixtures/PROVENANCE.md` ».
   - Interdictions absolues : jamais ecrire dans `~/Games/`,
     `~/Work/d2r-saves/`, `fixtures/`, `reference/` ; jamais mettre a jour les
     submodules ; jamais porter du code depuis `reference/d2rsavegameparser`
     (LGPL) ni ouvrir son `src/` ; v105 seulement ; binaire `d2r-manager`,
     jamais `d2r`.
   - Commandes : `just check`, `just test`, `just goldens`,
     `cd fixtures && sha256sum -c sha256sums.txt`.
   - Workflow : branches `feature/desc`, rebase, commits petits, pas de push ;
     deux echecs meme piste = stop et remonter ; divergence avec PLAN.md =
     question, pas d'initiative.
   - Index des regles, une ligne par fichier.
3. `.claude/rules/` (courtes, <= 10 puces, renvoient aux § de PLAN.md) :

   | Fichier | Scope (paths) | Contenu |
   |---|---|---|
   | `bitstream.md` | `crates/format/**`, `docs/FORMAT.md` | zero commentaire sauf `FORMAT.md §x.y` ; toute largeur de champ vient de FORMAT.md ; code qui contredit la spec = spec corrigee dans le meme commit ; goldens = corrige, jamais ajustes pour faire passer un test ; version != 105 = erreur explicite ; proptest bitreader uniquement |
   | `purity.md` | `crates/format/**`, `crates/domain/**` | zero I/O, horloge, processus (clippy `disallowed-types` fait foi) ; `thiserror`, jamais `anyhow` ici ; etats invalides irrepresentables : newtypes, positions bornees, `Plan`/`ValidatedPlan`/`AppliedPlan` distincts ; `dead_code` deny |
   | `oracles.md` | `tools/**`, `goldens/**` | d2rsavegameparser : jar execute + `txt/` seulement, jamais le source ; jar telecharge par script, sha256 obligatoire, jamais commite ; JSON canonique cles triees ; axes Java normalises sur convention D2SSharp (`InvX` = colonne) ; desaccords listes dans REPORT.md, jamais masques |
   | `adapters.md` | `crates/app/**`, `crates/cli/**` | `anyhow` dans cli seulement ; git par sous-processus, pas de bibliotheque ; adaptateurs dans `cli` jusqu'au lot 5 (pas de crate `infra` avant) ; fakes ecrits main, aucun framework de mock ; chemins injectes, tests sur repertoires temporaires uniquement |
   | `review.md` | sans scope, reference par les briefs | protocole reviewer frais : entrees, mandat verifier-en-executant, classification bloquant/majeur/mineur avec preuve, review vide = refaite |

4. Commit du lot 0, puis lancement du lot 1.

## Lots 1 a 5 (sous-agents Opus)

### Lot 1 : harnais differentiel (etape 1 de PLAN.md §8)

- Perimetre : `tools/goldens-csharp/` (console .NET, `ProjectReference` vers
  `reference/D2SSharp/src/D2SSharp/D2SSharp.csproj`, JSON canonique cles
  triees + verdict round-trip), `tools/goldens-java/` (fetch jar 3.2.0 avec
  sha256 du tableau PLAN §4, `txt/` copie depuis
  `reference/d2rsavegameparser/txt/`, sortie TSV), `goldens/csharp/*.json`,
  `goldens/java/*.tsv`, `goldens/REPORT.md`, `justfile` initial (`goldens`,
  squelette `check` avec `sha256sum -c`), `lefthook.yml`.
- Fait quand : goldens commites pour les 60 fichiers ; REPORT complet (taille,
  version, nb objets, verdict round-trip C#, concordance C#/Java, desaccords
  listes, windows a part) ; `just goldens` regenere tout a l'identique.
- Brief contient : PLAN §4 verbatim (versions epinglees) ; les 4 verites
  mesurees de PLAN §6 verbatim (axes Java inverses ; AdvancedStash pas une
  grille ; round-trip `.d2s` jamais mesure, le mesurer fichier par fichier ;
  legacy refuse par Java et lu par D2SSharp, consigner les deux) ; reperes
  anti-regression de `v105/linux/ModernSharedStashSoftCoreV2.d2i` (7 onglets,
  1533 objets, 1 075 411 or onglet 1, Chronicle 2298 octets) ; decision
  fixtures (60 goldens, windows a part) ; interdictions PLAN §9 ; ne jamais
  ouvrir `reference/d2rsavegameparser/src/`.

### Lot 2 : FORMAT.md + crate `format` lecture et ecriture (etapes 2+3+4)

- Fusion voulue : une spec ecrite par une tete et implementee par une autre
  est le pire cas de telephone ; les goldens ne valident la spec qu'a travers
  l'implementation ; le writer doit etre le miroir exact du reader ; FORMAT.md
  sert aussi de contexte compresse anti-saturation.
- Perimetre : scaffolding workspace (`Cargo.toml`, `rust-toolchain.toml`
  1.97.1, `clippy.toml` disallowed-types, `deny.toml`, justfile `check`/`test`
  complets) ; `docs/FORMAT.md` depuis le source D2SSharp +
  `~/Work/d2r-saves/docs/research/format-objet-v105.md` (lecture seule) ;
  `crates/format` complet.
- Fait quand : chaque champ du bitstream a sa largeur en bits et sa reference
  croisee ; 55/55 linux concordent champ a champ avec les goldens C# ; legacy
  en erreur propre ; round-trip 55/55 octet pour octet ; checksum `.d2s`
  reproduit ; proptest sur le bitreader uniquement.
- Ordre interne impose par le brief : squelette FORMAT.md ; bitreader +
  proptest ; header et sections fixes (halbu en reference MIT) ; items via
  `Item.cs` + Huffman ; ecriture + checksum. Toute contradiction decouverte =
  spec corrigee dans le meme commit.
- Contrat de reprenabilite dans le brief : a tout moment, branche + FORMAT.md
  + git log suffisent a un agent frais pour continuer. Si le contexte de
  l'agent sature, l'orchestrateur lance un agent de continuation avec la meme
  recette de brief.
- Fichiers de reference cles : `reference/D2SSharp/src/D2SSharp/Model/Item.cs`,
  `Model/D2Save.cs`, `IO/BitReader.cs`, `D2SaveOverlay.cs`,
  `TxtFileExternalData.cs` ; `reference/halbu/src/` pour le header.

### Lot 3 : crate `domain` (etape 5)

- Perimetre : objets, grilles (dimensions par code objet, table extraite des
  `.txt`), regles (plafond or 2 500 000 par onglet, uniques par personnage
  Annihilus / Hellfire Torch / Gheed's Fortune, piles max 99), types
  `Plan` -> `ValidatedPlan` -> `AppliedPlan`, positions bornees, newtypes.
- Fait quand : tests par table verts ; la fusion de coffres du 2026-08-20
  rejouee donne un resultat equivalent (reference : commit `2885c71` de
  d2r-saves, consulte via `git -C ~/Work/d2r-saves show/archive`, lecture
  seule).
- Brief contient : regle AdvancedStash verbatim (« on n'y invente jamais de
  position : on empile ou on ajoute ») ; convention axes D2SSharp ; purete
  clippy ; mode de consultation de `2885c71` sans ecrire dans d2r-saves.

### Lot 4 : `app` + `cli`, sync puis transactions (etapes 6+7, un agent continu)

- Fusion voulue : l'apply reutilise les adaptateurs de l'etape 6 (fs, git
  sous-processus, `pgrep D2R.exe`) et les conventions clap ; couper obligerait
  le second agent a modifier le code du premier. Checkpoint interne
  obligatoire : apres les criteres de l'etape 6, commit + mini-rapport a
  l'orchestrateur, qui gate avant d'autoriser la suite (via `SendMessage`).
- Perimetre : `crates/app` (ports/traits + cas d'usage sync, plan, apply),
  `crates/cli` (adaptateurs + binaire clap `d2r-manager`), fakes ecrits main.
- Fait quand : sync reproduit les gardes de `~/.local/bin/d2r-backup` (flock,
  pgrep double, chemins surs, rsync `--exclude '*.ctlo' --exclude '*.keyo'`,
  commit conditionnel) ; bascule = decision utilisateur, le script bash reste
  en place ; scenario transactionnel complet sur copie du corpus ; rollback
  `git revert` verifie ; invariant multiset conserve ; ledger
  `duplications.jsonl` cree par apply.
- Resolution ecrite dans le brief (conflit PLAN §7.4 vs §9.2) : tous les
  chemins sont injectes ; tests et scenario complet sur repertoires
  temporaires (copie du corpus + repo git jetable simulant d2r-saves) ;
  l'ecriture reelle dans `~/Work/d2r-saves/ledger/` n'arrive qu'a l'execution
  par l'utilisateur, jamais par un agent.
- Brief contient : les 89 lignes de `d2r-backup` en annexe ; sequence apply de
  PLAN §7.3 verbatim (jeu ferme, timer pause, commit point de retour,
  ecriture, relecture + invariant, commit final avec le plan dans le message,
  timer relance).

### Lot 5 : API HTTP locale (etape 8)

- Perimetre : extraction crate `infra` (autorisee maintenant seulement,
  PLAN §3.2, remplace sans accumuler), serveur HTTP localhost, contrat
  OpenAPI fige, memes cas d'usage que la CLI.
- Fait quand : contrat OpenAPI fige ; parite de cas d'usage avec la CLI.

## Protocole de review (bloc final de chaque brief)

1. Avant de se declarer fini : `just check` et `just test` verts, criteres du
   lot verifies un a un, commits propres, rebase sur main.
2. Spawner un reviewer Opus a contexte frais (outil Agent,
   `subagent_type: "claude"`, `model: "opus"`). Entrees : le brief verbatim,
   les § de PLAN.md concernes, `git diff main...feature/<lot>`, la liste des
   criteres. Ne pas transmettre l'auto-evaluation de l'implementeur.
3. Mandat du reviewer : verifier en executant (relancer `just check` /
   `just test` / `just goldens`, recompter, et pour le lot 2 sonder N champs
   FORMAT.md <-> code <-> D2SSharp), pas en lisant des affirmations. Verdict
   par critere + findings bloquant/majeur/mineur, chacun avec preuve
   (commande + sortie, ou fichier:ligne).
4. L'implementeur corrige tout bloquant et majeur, re-passe les gates,
   consigne findings et resolutions dans son rapport. Mineurs : corriges ou
   assumes explicitement. Une review sans finding doit dire ce qui a ete
   verifie empiriquement, sinon elle est refaite.

## Gate d'integration (orchestrateur, avant chaque merge dans main)

1. `git rebase main` propre ; `just check` (fmt, clippy pedantic
   `-D warnings`, machete, deny) ; `just test`.
2. Lots touchant les goldens : `just goldens` puis
   `git diff --exit-code goldens/` (idempotence).
3. Frontieres intactes : `cd fixtures && sha256sum -c sha256sums.txt` ;
   `git submodule status` inchange ; `git status --porcelain` propre.
4. Rejeu par l'orchestrateur du critere phare du lot (ex. lot 2 : la sortie de
   test affiche 55/55 concordance et 55/55 round-trip).
5. Sondage anti-slop : grep des commentaires hors `FORMAT.md §`, pas de v1+v2
   cohabitantes, messages de commit.
6. Merge par rebase + fast-forward. Echec de gate = retour au sous-agent avec
   le constat exact.

## Escalade et rapports

- Deux echecs meme piste : arret, resume ecrit (tentatives, causes d'echec,
  hypotheses restantes), remontee a l'orchestrateur. Pas de troisieme
  approche discrete.
- Divergence avec PLAN.md : question precise (constat, options,
  recommandation) relayee a l'utilisateur par l'orchestrateur.
- Rapport final de lot (message, pas de fichier commite ; les faits durables
  vont dans REPORT.md, FORMAT.md, adr) : statut par critere avec preuve ;
  decouvertes mesurees ; ecarts vs PLAN et questions/reponses ; findings du
  reviewer et resolutions ; restes assumes ; fichiers cles.
- Fin de projet (PLAN §8) : resume complet et honnete a l'utilisateur : ce qui
  passe, ce qui a ete decouvert, ce qui reste.

## Risques et mitigations

1. Saturation du lot 2 (~7k lignes C# a porter + spec) : FORMAT.md ecrit au
   fil de l'eau = contexte compresse ; commits petits ; contrat de
   reprenabilite ; agent de continuation avec la meme recette si besoin.
2. Reviewer complaisant : contexte frais, pas d'auto-evaluation transmise,
   mandat verifier-en-executant, review vide refaite, gate orchestrateur
   rejoue les criteres. Les goldens sont objectifs.
3. Derive FORMAT.md <-> code : spec corrigee dans le meme commit ; sondage
   croise du reviewer ; references `FORMAT.md §x.y` greppables.
4. Fixtures ou reference modifies : `sha256sum -c` dans `just check` des le
   lot 1 ; gate verifie submodules et porcelain ; interdictions dans CLAUDE.md
   toujours charge.
5. Contamination LGPL : interdiction d'ouvrir `reference/d2rsavegameparser/src/`
   dans CLAUDE.md et `oracles.md` ; les reviewers des lots 1 et 2 confirment
   ce point explicitement.

## Verification de bout en bout

- Apres chaque lot : le gate d'integration ci-dessus.
- A la livraison : les 8 criteres de PLAN §8 rejoues par l'orchestrateur sur
  main (`just check`, `just test`, `just goldens` idempotent, scenario
  transactionnel sur copie, rollback, contrat OpenAPI present) ; corpus et
  submodules intacts ; resume final honnete a l'utilisateur.
