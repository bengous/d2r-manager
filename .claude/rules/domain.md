---
paths:
  - "crates/domain/**"
---

# Regles du domaine

Faits mesures sur le corpus, pas des conventions : les changer casse des
fichiers reels.

- L'onglet `AdvancedStash` n'est pas une grille : les fichiers ecrits par le
  jeu y portent des positions qui se recouvrent. On empile sur une entree de
  meme empreinte tant qu'il reste de la place, sinon on ajoute en gardant la
  position de l'objet entrant. Aucune position ne s'invente : `Destination::Pile`
  n'a pas de champ cellule.
- Le champ de pile `AdvancedStash` est optionnel par objet et entre dans
  l'empreinte : les objets de grille le portent absent, ceux de l'onglet le
  portent present. Un deplacement d'une grille vers l'onglet change donc
  l'identite de l'objet et se refuse a la projection (`app::world`). Les sens
  pile vers pile, pile vers grille et grille vers grille restent permis.
- Des entrees de pile a 0 unite existent dans les vrais fichiers : `PileCount`
  va de 0 a 99 pour ce qu'une entree porte, `PileUnits` de 1 a 99 pour ce
  qu'une operation deplace. Une entree a 0 se lit, ne se deplace pas.
- `carry1` des `.txt` est un identifiant de groupe, pas un booleen : 4 groupes,
  dont les charmes Sunder (`cs2`), absents de la liste habituelle (Annihilus,
  Hellfire Torch, Gheed's Fortune). La regle du « un par personnage » porte
  sur le groupe.
- L'or ne se deplace pas : `Plan` ne porte que `move` et `duplicate`, et
  `Census` exclut l'or, donc l'invariant de conservation ne le voit pas. Le
  seul filet est `infra/tests/transaction.rs`, test
  `no_operation_moves_a_single_piece_of_gold`, qui lit l'or des onglets et les
  `PlayerStats` depuis les fichiers. Une operation de transfert d'or est une
  decision ouverte, voir `docs/DECISIONS.md`.
- Une regle « quantity <= maxstack » refuserait des fichiers reels : `ama`
  porte 180 la ou les `.txt` annoncent 120. La limite de 99 vise les piles de
  l'onglet `AdvancedStash`, pas le champ `quantity`.
- `Plan` -> `ValidatedPlan` -> `AppliedPlan` sont trois types distincts, aucun
  ne derive `Clone`, et `validate` consomme le vault : reconstruire le vault
  depuis les fichiers pour reessayer apres un refus.
- La geometrie vient des `.txt` par table generee et commitee
  (`src/tables/v105.rs`), revalidee par `tests/tables.rs` ; `just tables`
  regenere. Jamais de dimension ecrite a la main.
