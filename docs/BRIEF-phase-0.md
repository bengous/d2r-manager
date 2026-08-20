# Brief : phase 0, oracles et golden files

Destinataire : agent d'implementation, session neuve. Lire `docs/PLAN.md` en
entier avant toute action. Ce brief ne couvre que la phase 0.

## Pourquoi cette phase existe

Le projet porte un parseur de bitstream depuis C# vers Rust. Un test de
round-trip (`write(read(f)) == f`) ne prouve pas la comprehension : un champ lu
au mauvais offset et reecrit au meme mauvais offset passe le test, et l'outil
affichera ensuite des valeurs fausses sans qu'aucun test ne rougisse.

Les golden files sont le corrige : la comprehension de l'oracle, figee en JSON.
Les tests Rust des phases 2 et 3 comparent leur lecture a ce corrige.

Precedent concret : le 2026-08-20, le croisement des deux oracles a revele que
les accesseurs `x()` et `y()` du parseur Java sont inverses par rapport a la
grille du jeu. Mesure : largeur portee par `y`, zero recouvrement sur 5 onglets
de deux fichiers reels ; portee par `x`, 108 et 60 recouvrements. C'est
exactement le type d'erreur que ce harnais doit attraper.

## Livrables

1. `tools/goldens-csharp/` : projet .NET console. Entree, un chemin de fichier ;
   sortie, un JSON canonique (cles triees, indente) du modele D2SSharp complet,
   plus le verdict round-trip du fichier. Referencer le submodule
   `reference/D2SSharp/src/D2SSharp/D2SSharp.csproj` par `ProjectReference`.
2. `tools/goldens-java/` : equivalent lecture seule avec
   `d2rsavegameparser` 3.2.0. Le jar se telecharge depuis Maven Central par
   script, avec **controle sha256 obligatoire** (valeur dans `docs/PLAN.md` §4) ;
   ne pas le commiter. Le dossier `txt/` vient du submodule
   `reference/d2rsavegameparser/txt/`. Sortie TSV : conteneur, code, position,
   pile, qualite, empreinte.
3. `goldens/csharp/*.json` et `goldens/java/*.tsv` : un fichier par fixture,
   commites.
4. `goldens/REPORT.md` : par fixture, taille, version detectee, nombre d'objets,
   verdict round-trip de l'oracle C#, et concordance C#/Java sur les champs
   communs. Les desaccords sont listes, pas masques.
5. `justfile` : recette `goldens` qui regenere tout, et `check`.

## Contraintes

1. `fixtures/` est en lecture seule. `reference/` contient des submodules figes :
   ne pas les modifier, ne pas les mettre a jour.
2. Ne jamais ecrire dans `~/Games/` ni dans `~/Work/d2r-saves/`.
3. Aucun code Rust dans cette phase.
4. Ne pas porter de code depuis `reference/d2rsavegameparser` : LGPL 2.1, il sert
   d'oracle execute uniquement. D2SSharp et halbu sont MIT, portage permis.
5. Les axes du parseur Java sont inverses par rapport a D2SSharp. Normaliser sur
   la convention D2SSharp (`InvX` = colonne, `InvY` = ligne) dans les sorties, et
   le documenter.
6. L'onglet specialise du coffre moderne (`AdvancedStash`) n'est pas une grille :
   les fichiers ecrits par le jeu y contiennent des positions qui se recouvrent.
   Ne pas traiter ces recouvrements comme une anomalie.

## Attentes chiffrees, verifiables

- 58 fixtures traitees : 53 `.d2s` v105, 2 `.d2i` v105 Linux, 2 `.d2i` v105
  Windows, 3 `legacy` (v98/v99).
- Round-trip `.d2i` : **doit** etre a 4/4 sur les fichiers v105. Deja verifie le
  2026-08-20 sur les deux coffres modernes.
- Round-trip `.d2s` : **jamais mesure a ce jour**. Le resultat est le livrable,
  pas une reussite exigee. Un echec partiel est une information utile : le
  documenter fichier par fichier dans `REPORT.md`.
- `legacy/` : l'oracle Java doit refuser les trois (v105 exclusif). D2SSharp doit
  les lire (il couvre v96 a v105). Consigner les deux comportements.

## Reperes de contenu, pour detecter une regression grossiere

Coffre partage moderne Linux, fixture `v105/linux/ModernSharedStashSoftCoreV2.d2i` :
7 onglets, dont 5 normaux, 1 `AdvancedStash`, 1 `Chronicle`. Mesure du
2026-08-20 sur ce fichier exact : 1533 objets, 1 075 411 pieces d'or a
l'onglet 1, onglet `Chronicle` de 2298 octets.

## Fin de phase

Criteres remplis, resume, arret. Ne pas enchainer sur la phase 1 sans validation.
