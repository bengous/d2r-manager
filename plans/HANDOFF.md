# HANDOFF orchestration d2r-manager

Ecrit le 2026-08-21 par la session orchestratrice Fable, avant epuisement de
son quota. Ce fichier est le point de reprise : une session orchestratrice
neuve le lit, puis docs/PLAN.md, et continue sans autre contexte. S'y tenir.

Modeles : orchestrateur = Fable si disponible, sinon Opus. Sous-agents et
reviewers = Opus (`model: "opus"`, `subagent_type: "claude"`). Jamais Haiku ni
Sonnet. Plan d'orchestration complet :
`plans/2026-08-20/d2r-manager-plan-d-orchestration-multi-agents.md`.

## Etat au moment du handoff

- main = `476d83e` : lots 0, 1, 2, 3 integres. Harnais goldens (60 fichiers),
  crate `format` (55/55 concordance, 55/55 round-trip), crate `domain`
  (grilles, regles, typestate de plan, fusion 2026-08-20 rejouee). 73 tests.
- Prochaine etape : lot 4 (`lot4-app-cli`), brief pret plus bas.
- Regle de checkout : ne jamais changer de branche ni commiter pendant qu'un
  agent travaille, c'est un checkout unique.

## Regles de l'orchestrateur

1. Il ne code jamais, ne lit ni code ni diffs ; commandes a sortie courte.
2. Un echec de gate repart au sous-agent avec le constat exact.
3. Chaque implementeur spawne lui-meme son reviewer Opus a contexte frais
   (protocole : `.claude/rules/review.md`) avant de se declarer fini.
4. Escalade utilisateur : uniquement blocage reel ou divergence avec
   docs/PLAN.md. Tout le reste s'arbitre ici et se note dans ce fichier.
5. Agents parfois idle sans rapport : relancer par SendMessage (demander le
   rapport). Agent mort : spawner un agent de continuation qui VERIFIE,
   re-passe une review fraiche et rapporte, sans refaire le travail
   (precedent : lot2-format-2).
6. `grep` du shell est un alias : utiliser `/usr/bin/grep` dans les sondes.

## Etapes restantes, dans l'ordre

1. Rapport de `lot3-domain` recu -> gate lot 3 (commandes ci-dessous),
   arbitrer ses questions, merge ff-only, supprimer la branche, commiter ce
   fichier + toute note PLAN.md decidee.
2. Spawner `lot4-app-cli` (Opus) avec le brief lot 4 verbatim ci-dessous.
3. A son checkpoint sync (SendMessage) : gate leger (ci-dessous), puis lui
   repondre par SendMessage : « checkpoint valide, continue vers les
   transactions ».
4. A son rapport final : gate lot 4, arbitrages, merge ff-only.
5. Spawner `lot5-http-api` (Opus) avec le brief lot 5 verbatim ci-dessous.
6. A son rapport : gate lot 5, merge.
7. Livraison : rejouer sur main les criteres PLAN §8 (`just check`,
   `just test`, `just goldens` puis `git diff --exit-code goldens/`,
   scenario transactionnel sur copie deja couvert par les tests du lot 4),
   verifier corpus et submodules intacts, puis resume complet et honnete a
   l'utilisateur : ce qui passe, ce qui a ete decouvert, ce qui reste.
   C'est la definition de « fini » de PLAN §8.

## Gate d'integration (avant chaque merge)

Sur la branche du lot, sortie courte :

```
echo "branch=$(git branch --show-current) dirty=$(git status --porcelain|wc -l) commits=$(git log --oneline main..HEAD|wc -l)"
git diff main --stat -- goldens/ tools/ fixtures/ | wc -l        # attendu 0 (lot 3+ : ajouter crates/format/)
git submodule status | cut -c1                                    # attendu : espaces seulement
just check >/dev/null 2>&1; echo "check=$?"                       # attendu 0
just test 2>&1 | /usr/bin/grep -E '^test result'                  # attendu : 0 failed partout
/usr/bin/grep -rn '//' crates/ --include='*.rs' | /usr/bin/grep -v 'FORMAT.md §' | /usr/bin/grep -v '://' | wc -l   # attendu 0
```

Puis : rejouer le critere phare du lot (le rapport du lot le nomme), merge :
`git checkout main -q && git merge --ff-only <branche> && git branch -d <branche>`.
Si le lot touche goldens/ : `just goldens` puis `git diff --exit-code goldens/`.

## Checkpoint lot 4 (gate leger, sans merge)

`just check` et `just test` verts + le rapport de checkpoint liste les gardes
de sync reproduites (flock, pgrep double, chemins surs, rsync exclusions,
commit conditionnel). Si vert : repondre « checkpoint valide, continue ».
Sinon : constat exact, l'agent corrige avant de continuer.

## Arbitrages deja rendus (ne pas re-ouvrir)

1. Corpus : goldens 60 fichiers ; criteres 55/55 = `v105/linux/` ; windows
   documentes a part (decision utilisateur).
2. Rythme : bout en bout sans pause utilisateur ; review 100 % Claude
   (decisions utilisateur).
3. Repere « 1533 objets » = 227 entrees + 1306 en piles AdvancedStash.
4. D2SSharp refuse la v98 (`legacy/JewelsCraft.d2s`) faute de txt : normal.
5. Epingles ajoutees a PLAN §4 : slf4j-api 2.0.17 (sha256 dans le tableau).
6. deny.toml : exceptions permissives nommees acceptees (Unicode-3.0).
7. Couverture demon sur 1 fixture : assumee. Mode objet 4 seulement dans
   windows : assume, hors criteres.
8. clippy.toml disallowed-types : un par crate pure (format fait, domain le
   pose dans le lot 3). Jamais a la racine (cli a besoin de fs).
9. Ledger vs interdiction d'ecrire dans ~/Work/d2r-saves : chemins injectes,
   tests sur repertoires temporaires ; l'ecriture reelle n'arrive qu'a
   l'execution par l'utilisateur. Deja dans le brief lot 4.
10. Transfert d'or : aucune operation ne deplace d'or (PLAN §7.1 ne liste que
    move et duplicate). L'apply ne verifie que le multiset d'objets. Question
    ouverte notee en PLAN §11.2 ; hors perimetre tant que l'utilisateur ne
    tranche pas.
11. Blobs du rejeu de fusion sous `crates/domain/tests/data/merge-2026-08-20/`
    (before.d2i, after.d2i, PROVENANCE.md avec sha256) : accepte, parce que
    `fixtures/` est en lecture seule. Pas de nouveau gel scripte.
12. Portee du vault : uniquement les conteneurs adressables par un plan
    (onglets partages, pile AdvancedStash, inventaire, coffre personnel,
    cube). Equipement, ceinture, curseur, cadavres, mercenaire, golem hors
    modele. Objets sertis compris dans l'empreinte de leur hote. Accepte.
13. `carry1` des .txt est un identifiant de groupe, pas un booleen : 9 lignes,
    4 groupes, dont les charmes Sunder (`cs2`) que PLAN §7.2 ne nomme pas. La
    regle porte sur le groupe. Accepte.
14. Restes de la passe 2 de review du lot 3, sans gravite, non bloquants :
    `ApplyError::Conservation` est structurellement injoignable via `apply`
    (assertion interne, pas un chemin d'erreur) ; `into_outcome(self)` et
    `apply(self)` s'excluent. Le second point est traite par une autorisation
    nommee dans le brief du lot 4 ci-dessous.

## Brief lot 4 (passer verbatim au spawn de `lot4-app-cli`)

Tu es l'agent d'implementation du lot 4 du projet d2r-manager : crates `app` (ports et cas d'usage) et `cli` (adaptateurs et binaire clap `d2r-manager`), en deux temps : d'abord sync/ls/find/diff (etape 6 de PLAN §8), checkpoint, puis transactions plan/apply (etape 7). Autonomie de bout en bout dans /home/b3ngous/Work/d2r-manager. Pas de validation intermediaire utilisateur ; remonte uniquement blocages reels et divergences.

AVANT TOUTE ACTION, lis : docs/PLAN.md en entier (§2, §3, §7, §8), CLAUDE.md, .claude/rules/adapters.md, rust.md, review.md. Interdictions absolues de CLAUDE.md ; en plus : crates/format/, crates/domain/, goldens/, tools/ sont les livrables des lots precedents, tu ne les modifies pas ; s'il te manque une API, question a l'orchestrateur, pas de patch chez eux.

AUTORISATION NOMMEE, unique exception a la ligne precedente : la review du lot 3 a laisse un defaut d'ergonomie que tu vas rencontrer. `ValidatedPlan::into_outcome(self)` et `ValidatedPlan::apply(self)` se consomment tous les deux, donc ton flux (valider -> prendre le monde attendu -> ecrire -> relire -> verifier l'invariant) t'oblige a cloner l'outcome. Tu es autorise a corriger l'API de `crates/domain` sur CE point precis, par le plus petit diff possible, sans toucher aux regles ni au typestate (Plan -> ValidatedPlan -> AppliedPlan reste etanche, aucun des trois ne derive Clone). `Vault` n'a pas non plus d'accesseur consommant : meme autorisation si ton flux en a besoin. Les tests de domain doivent rester verts. Tout autre besoin dans domain ou format = question a l'orchestrateur. Sache aussi que `Plan::validate` consomme le vault et le perd en cas de refus : reconstruis-le depuis les fichiers pour reessayer.

L'OR EST HORS PERIMETRE : aucune operation ne le deplace (PLAN §7.1), l'apply ne verifie que le multiset d'objets, et le plafond de 2 500 000 par onglet reste une regle de construction dans domain. Ne l'ajoute pas de toi-meme, c'est une question ouverte de l'utilisateur (PLAN §11.2). JAMAIS d'ecriture dans ~/Games/ ni ~/Work/d2r-saves/ : tous les chemins sont injectes, tes tests et scenarios tournent sur des repertoires temporaires (copie du corpus + depot git jetable simulant d2r-saves). L'ecriture reelle n'arrivera qu'a l'execution par l'utilisateur.

BRANCHE : feature/app-cli depuis main. Commits petits, pas de push. Binaire `d2r-manager`, JAMAIS `d2r` (nom pris).

TEMPS 1 : sync, ls, find, diff (PLAN §8 etape 6).
- `crates/app` : ports (traits) fs/git/processus/horloge + cas d'usage. `crates/domain` et `format` en dessous. `anyhow` dans cli SEULEMENT, `thiserror` dans app. Fakes ecrits main, aucun framework de mock. git par sous-processus `git`, pas de bibliotheque.
- `sync` reproduit les gardes du script bash de reference ~/.local/bin/d2r-backup (LECTURE SEULE, annexe ci-dessous) : verrou exclusif, double detection `D2R.exe` (pgrep exact + pattern), chemins surs (pas de symlink, racines verifiees), rsync --archive --delete --delete-excluded avec exclusions *.ctlo *.keyo, re-verification du jeu apres rsync, commit conditionnel si diff, message `backup: <horodatage ISO>`. Le push reste HORS perimetre (pas de remote chez d2r-manager ; le script bash reste en place, la bascule est une decision utilisateur).
- `ls`, `find`, `diff` : exploration hors jeu des inventaires, coffres personnels, coffres partages, cube, via `format` + `domain`. Sorties lisibles et stables (tri deterministe).
- CHECKPOINT OBLIGATOIRE : quand les criteres du temps 1 passent (`just check` + `just test` verts, gardes de sync couvertes par des tests sur fakes et repertoires temporaires), commit, puis envoie un mini-rapport par SendMessage to: "team-lead" (statut par garde + commandes de preuve) et ATTENDS sa reponse avant le temps 2.

TEMPS 2 : transactions (PLAN §7, §8 etape 7).
- `plan` : produit un JSON d'operations (move, duplicate), objets references par empreinte + conteneur source. N'ecrit rien. La validation vit dans `domain` (typestate Plan -> ValidatedPlan) : consomme-la, ne la reimplemente pas.
- `apply`, sequence verbatim de PLAN §7.3 : jeu ferme (pgrep D2R.exe), timer en pause, commit git de point de retour, ecriture, relecture et verification d'invariant (le multiset d'objets est conserve hors operations explicites), commit final avec le plan dans le message, timer relance. Rollback = git revert. Le « timer » est le declencheur systemd utilisateur du backup : identifie-le en lecture (`systemctl --user list-timers`) ; dans app c'est un port, dans les tests un fake ; l'adaptateur reel utilise `systemctl --user`.
- Registre de provenance (PLAN §7.4) : chaque duplication journalisee (empreinte, source, cible, date) dans `<repo-saves>/ledger/duplications.jsonl`, cree s'il n'existe pas, hors du miroir rsync, commite par apply dans le meme commit que l'operation. `<repo-saves>` est un chemin injecte ; en test c'est le depot jetable.
- Or : plafond 2 500 000 par onglet (regle domain). Uniques par personnage : Annihilus, Hellfire Torch, Gheed's Fortune. Piles max 99. AdvancedStash : jamais de position inventee, on empile ou on ajoute.

CRITERES D'ACCEPTATION : (1) gardes de sync reproduites et testees une a une ; (2) scenario transactionnel complet sur copie du corpus : plan -> validation -> apply -> relecture, invariant multiset verifie ; (3) rollback par git revert verifie par test ; (4) ledger cree et commite par apply dans le meme commit ; (5) `just check` et `just test` verts ; (6) la CLI expose sync, ls, find, diff, plan, apply avec une aide claire.

QUALITE RUST (regle .claude/rules/rust.md) : types d'abord ; le skill `unrepresentable` est disponible via l'outil Skill pour les types publics (etats du timer, resultats d'apply, references d'objet). Zero commentaire. Erreurs : echec rapide, contexte propage, aucun silencieux.

ECHECS : deux echecs meme piste = arret, resume, remontee. Divergence avec PLAN.md = question precise. Contexte qui sature = commit propre + resume + le dire.

REVIEW OBLIGATOIRE (protocole .claude/rules/review.md) : quand les criteres passent, spawne un reviewer Opus a contexte frais (outil Agent, subagent_type: "claude", model: "opus"). Donne-lui ce brief verbatim, PLAN §2, §3, §7, §8, la liste des fichiers de la branche, les criteres. PAS ton auto-evaluation. Mandat : relancer `just check` et `just test` ; verifier `git diff main -- crates/format/ crates/domain/ goldens/ tools/ fixtures/` vide ; verifier par lecture ET par test qu'aucun chemin reel (~/Games, ~/Work/d2r-saves) n'est code en dur dans app ni atteint par les tests ; rejouer le scenario transactionnel et le rollback ; comparer les gardes de sync au script annexe ligne a ligne ; verdict par critere + findings avec preuve. Corrige bloquants et majeurs, re-passe les gates.

RAPPORT FINAL OBLIGATOIRE via SendMessage to: "team-lead" : (a) statut par critere avec preuve courte ; (b) decouvertes ; (c) ecarts vs PLAN et questions ; (d) findings du reviewer et resolutions ; (e) restes assumes ; (f) fichiers cles.

ANNEXE : ~/.local/bin/d2r-backup, la reference des gardes (89 lignes, lecture seule). Consulte-le directement sur disque ; ses gardes, dans l'ordre : fail() sur arguments, HOME et XDG_RUNTIME_DIR requis ; commandes requises date flock git pgrep realpath rsync ; source_dir="$HOME/Games/battlenet/drive_c/users/steamuser/Saved Games/Diablo II Resurrected" (existe, pas un symlink) ; repo_dir="$HOME/Work/d2r-saves" (.git present, pas un symlink, rev-parse --show-toplevel == repo_dir via realpath) ; branche courante == main ; remote origin present ; verrou flock -n sur $XDG_RUNTIME_DIR/d2r-backup/lock (occupe -> exit 0 avec message) ; D2R detecte (pgrep -x 'D2R.exe' OU pgrep -f pattern chemin) -> skip exit 0 ; mirror_dir=$repo_dir/saves (ni symlink ni fichier, realpath == $repo_root/saves) ; rsync --archive --delete --delete-excluded --exclude='*.ctlo' --exclude='*.keyo' source/ mirror/ ; re-check D2R apres rsync -> commit differe exit 0 ; git add -A -- saves ; commit seulement si diff cached non vide, message "backup: $(date --iso-8601=seconds)" ; push origin main (HORS perimetre d2r-manager).

## Brief lot 5 (passer verbatim au spawn de `lot5-http-api`)

Tu es l'agent d'implementation du lot 5 du projet d2r-manager : l'API HTTP locale (PLAN §8 etape 8). Autonomie de bout en bout dans /home/b3ngous/Work/d2r-manager. Remonte uniquement blocages reels et divergences.

AVANT TOUTE ACTION, lis : docs/PLAN.md (§1, §3, §8), CLAUDE.md, .claude/rules/adapters.md, rust.md, review.md. Interdictions absolues de CLAUDE.md ; crates format/domain/app/cli, goldens/, tools/ : livrables des lots precedents, question a l'orchestrateur s'il te manque une API. Chemins injectes, tests sur repertoires temporaires uniquement.

BRANCHE : feature/http-api depuis main. Commits petits, pas de push.

PERIMETRE :
1. Extraction de la crate `infra` : les adaptateurs (fs, git, processus, timer) sortent de `cli` vers `crates/infra`, MAINTENANT SEULEMENT (PLAN §3.2 l'autorisait a l'arrivee de l'API HTTP). REMPLACE, n'accumule pas : `cli` consomme `infra`, aucun adaptateur ne survit en double. Le graphe de dependances reste hexagonal : format <- domain <- app <- {cli, serveur} ; infra implemente les ports de app.
2. Serveur HTTP local, localhost uniquement, jamais d'ecoute externe. Memes cas d'usage que la CLI : sync, ls, find, diff, plan, apply. Aucune operation de plus (pas de creation d'objet, rien qui contourne domain).
3. Contrat OpenAPI fige et commite (fichier openapi.yaml ou .json versionne) : c'est LE livrable qui fige le contrat pour une GUI future (PLAN §2). Le serveur et le contrat ne divergent pas : un test les confronte.
4. Choix du framework HTTP : minimal, licences MIT/Apache-2.0 compatibles deny.toml, dependances epinglees par Cargo.lock. Justifie ton choix en une ligne dans le rapport.

CRITERES D'ACCEPTATION : (1) contrat OpenAPI fige, commite, confronte au serveur par un test ; (2) parite de cas d'usage avec la CLI, verifiee par tests d'integration sur repertoires temporaires ; (3) `cli` ne contient plus d'adaptateur (extraction infra complete, v1 supprimee) ; (4) `just check` et `just test` verts ; (5) cargo-deny passe avec les nouvelles dependances.

QUALITE RUST : regle .claude/rules/rust.md ; skill `unrepresentable` disponible via l'outil Skill. Zero commentaire. Erreurs typees aux frontieres HTTP (codes precis, pas de 500 fourre-tout pour les erreurs metier).

ECHECS ET DIVERGENCES : deux echecs meme piste = arret et remontee ; divergence PLAN.md = question.

REVIEW OBLIGATOIRE (.claude/rules/review.md) : reviewer Opus a contexte frais (Agent, subagent_type: "claude", model: "opus") ; entrees : ce brief verbatim, PLAN §1-3 et §8, liste des fichiers de la branche, criteres ; PAS ton auto-evaluation. Mandat : relancer les gates ; verifier `git diff main -- crates/format/ crates/domain/ goldens/ tools/ fixtures/` vide (app et cli changent legitimement : extraction infra) ; verifier qu'aucun adaptateur ne survit en double dans cli ; confronter 6 routes tirees au hasard au contrat OpenAPI ; verifier que le serveur n'ecoute que sur localhost ; verdict par critere + findings avec preuve. Corrige bloquants et majeurs.

RAPPORT FINAL OBLIGATOIRE via SendMessage to: "team-lead" : (a) statut par critere avec preuve ; (b) decouvertes ; (c) ecarts et questions ; (d) findings du reviewer et resolutions ; (e) restes assumes ; (f) fichiers cles, dont le chemin du contrat OpenAPI.
