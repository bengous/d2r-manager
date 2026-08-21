# Decisions en attente

Questions ouvertes qui demandent un arbitrage de l'utilisateur. Un agent ne
les tranche pas seul et ne code pas la reponse par avance. Fermer une entree,
c'est la supprimer d'ici et ecrire la regle a l'endroit qui la garde : le code,
une rule de `.claude/rules/`, ou `docs/openapi.json`.

## 1. Remote git

Aucun remote n'existe. `sync` commite en local seulement, et le push reste
hors perimetre. La creation d'un depot distant prive est une decision de
l'utilisateur.

## 2. Retour des fichiers vers le jeu

`apply` ecrit dans le miroir du depot de sauvegarde, jamais dans `~/Games/`.
La copie retour vers le repertoire du jeu reste a decider, comme la bascule
depuis le script `~/.local/bin/d2r-backup`, qui reste en place.

## 3. Verrou d'`apply`

`app::sync` prend un verrou par le port `Locks` ; `app::transaction` n'en
prend aucun. La sequence d'`apply` met le timer en pause, mais une sauvegarde
deja en vol peut donc ecraser une ecriture. Le port existe et `infra` en porte
l'implementation (`infra::files::FileLock`) : l'etendre a `apply` est une
decision de l'utilisateur.

## 4. Transfert d'or

`Plan` ne porte que `move` et `duplicate`, donc aucun plan ne deplace d'or.
La fusion de coffres du 2026-08-20 en a pourtant deplace 940 000 a la main.
Une troisieme operation reste a decider ; elle est hors perimetre tant qu'elle
ne l'est pas. Le plafond de 2 500 000 pieces par onglet reste une regle de
construction dans `domain`, quelle que soit l'issue.
