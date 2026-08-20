# Harnais differentiel : rapport

Fichier genere par `just goldens`. Toute edition manuelle est ecrasee.

| Oracle | Source | Role |
|---|---|---|
| C# | D2SSharp, submodule `reference/D2SSharp` (MIT) | lecture, ecriture, verdict round-trip |
| Java | d2rsavegameparser 3.2.0, jar telecharge et verifie par sha256 (LGPL 2.1) | lecture seule, execute, jamais porte |

Sorties : `goldens/csharp/<chemin>.json` (modele D2SSharp complet, cles triees,
indente, sans horodatage) et `goldens/java/<chemin>.tsv` (un objet par ligne).

Le jar de l'oracle Java est telecharge par `tools/goldens-java/fetch-jars.sh`,
verifie par sha256 `0b2164f3c8219e390c42ccba3e2377dce2514561e1c32f5d0c1d4c9622960f89`
et jamais commite. Il lit les tables du jeu dans
`reference/d2rsavegameparser/txt/`, en place et en lecture seule.

Le parseur declare `org.slf4j:slf4j-api` en portee `provided` et ses classes ne
se chargent pas sans lui. Le meme script telecharge donc slf4j-api 2.0.17,
sha256 `7b751d952061954d5abfed7181c1f645d336091b679891591d63329c622eb832`.
Aucun fournisseur de journalisation n'est ajoute : le parseur reste silencieux.

## Conventions

1. Axes. Le parseur Java expose `x()` = ligne et `y()` = colonne, soit l'inverse
   de D2SSharp. Les TSV sont ecrits dans la convention D2SSharp : colonne
   `invX` = `y()` Java = `InvX` C#, ligne `invY` = `x()` Java = `InvY` C#. Sur les 57
   fichiers lus par les deux oracles, 2564 lignes ont la meme position apres cet echange.
   La mesure de PLAN section 6 porte sur `v105/windows/ModernSharedStashSoftCoreV2.d2i` :
   135 objets de premier niveau, 151 lignes concordantes objets sertis inclus.
2. Conteneurs. Vocabulaire commun : `INVENTORY`, `CUBE`, `STASH`, `EQUIPPED`,
   `BELT`, `SOCKET`, `TRANSIT`. Les objets d'un coffre partage portent le prefixe
   `TAB<n>:`, ceux d'un cadavre `CORPSE:`, ceux du mercenaire `MERC:`, le golem
   `GOLEM:`. Cote C# le conteneur vient de `ItemPosition.Mode` puis
   `ItemPosition.StorePage` ; cote Java de `location()` puis `container()`.
3. Comptage. `objets` compte les objets de premier niveau (hors objets sertis) ;
   `lignes` compte les objets sertis en plus, et c'est l'unite de comparaison.
4. Equivalences de vocabulaire appliquees a la comparaison, mesurees :
   - objet compact (`ItemFlags.CompactSave`) : C# `NORMAL`, Java `NONE` (1142 lignes) ;
   - C# `ItemMode.OnCursor`, Java `ItemLocation.TRANSIT` (5 lignes).
   Ces deux regles nomment differemment une meme valeur lue ; tout autre ecart
   est un desaccord et figure dans la section dediee. Une ligne peut relever
   des deux regles a la fois, les deux comptes ne s'additionnent donc pas.
5. Format TSV : lignes de metadonnees `#cle<TAB>valeur`, puis l'entete de
   colonnes `container code invX invY stack quality fingerprint`, puis un objet
   par ligne. `fingerprint` est l'empreinte calculee par l'oracle Java ; D2SSharp
   n'a pas d'equivalent, elle n'entre pas dans la comparaison.
6. Concordance globale : 2564/2564 lignes concordantes, dont 1422 identiques
   champ a champ sans aucune regle, et 1142 ramenees par les equivalences
   du point 4. La colonne `concordance` des tableaux donne le meme detail.
7. Garde-fou : `goldens-csharp report` sort en erreur si un fichier `v105/` est
   refuse par un oracle, si son round-trip C# n'est plus identique, ou si un
   desaccord apparait. Une regression de lecture ou d'ecriture arrete donc
   `just goldens` au lieu de passer inapercue dans un diff.

## Corpus v105/linux (55 fichiers)

| fixture | octets | version | objets | lignes | round-trip C# | concordance C#/Java |
|---|---:|---:|---:|---:|---|---|
| Armageddon.d2s | 1962 | 105 | 62 | 68 | identique | 68/68 dont 49 par equivalence |
| Bases.d2s | 1343 | 105 | 21 | 21 | identique | 21/21 dont 4 par equivalence |
| BasesTwo.d2s | 1446 | 105 | 24 | 24 | identique | 24/24 dont 4 par equivalence |
| Belts.d2s | 1123 | 105 | 8 | 8 | identique | 8/8 dont 4 par equivalence |
| Ber.d2s | 3971 | 105 | 101 | 130 | identique | 130/130 dont 45 par equivalence |
| BigSets.d2s | 1227 | 105 | 10 | 10 | identique | 10/10 dont 4 par equivalence |
| Charmed.d2s | 2583 | 105 | 67 | 67 | identique | 67/67 strictes |
| CharmedTwo.d2s | 1384 | 105 | 20 | 20 | identique | 20/20 dont 4 par equivalence |
| Cica.d2s | 3005 | 105 | 65 | 88 | identique | 88/88 dont 37 par equivalence |
| CicaLinux.d2s | 2315 | 105 | 42 | 58 | identique | 58/58 dont 25 par equivalence |
| Circlets.d2s | 1054 | 105 | 6 | 6 | identique | 6/6 dont 4 par equivalence |
| CircletsCoronet.d2s | 1060 | 105 | 6 | 6 | identique | 6/6 dont 4 par equivalence |
| Crafts.d2s | 1111 | 105 | 12 | 12 | identique | 12/12 dont 4 par equivalence |
| Crameuse.d2s | 2502 | 105 | 55 | 75 | identique | 75/75 dont 40 par equivalence |
| Elektra.d2s | 3779 | 105 | 86 | 122 | identique | 122/122 dont 48 par equivalence |
| Facets.d2s | 1057 | 105 | 7 | 7 | identique | 7/7 dont 4 par equivalence |
| Fire.d2s | 962 | 105 | 4 | 4 | identique | 4/4 dont 4 par equivalence |
| Firefox.d2s | 1199 | 105 | 11 | 15 | identique | 15/15 dont 10 par equivalence |
| Gous.d2s | 3174 | 105 | 66 | 101 | identique | 101/101 dont 56 par equivalence |
| GousDeux.d2s | 1409 | 105 | 31 | 31 | identique | 31/31 dont 17 par equivalence |
| Hammer.d2s | 3334 | 105 | 81 | 110 | identique | 110/110 dont 50 par equivalence |
| IAone.d2s | 1055 | 105 | 10 | 10 | identique | 10/10 dont 7 par equivalence |
| Javazon.d2s | 3386 | 105 | 75 | 107 | identique | 107/107 dont 47 par equivalence |
| Jewels.d2s | 1259 | 105 | 16 | 16 | identique | 16/16 dont 4 par equivalence |
| JewelsCraft.d2s | 1271 | 105 | 16 | 16 | identique | 16/16 dont 4 par equivalence |
| JewelsNice.d2s | 1377 | 105 | 19 | 19 | identique | 19/19 dont 4 par equivalence |
| LevelingItems.d2s | 2018 | 105 | 32 | 38 | identique | 38/38 dont 10 par equivalence |
| MFitems.d2s | 1086 | 105 | 7 | 10 | identique | 10/10 dont 7 par equivalence |
| MercGear.d2s | 1256 | 105 | 12 | 14 | identique | 14/14 dont 6 par equivalence |
| Merco.d2s | 962 | 105 | 4 | 4 | identique | 4/4 dont 4 par equivalence |
| ModernSharedStashHardCoreV2.d2i | 556 | 105 | 0 | 0 | identique | 0/0 strictes |
| ModernSharedStashSoftCoreV2.d2i | 8729 | 105 | 227 | 251 | identique | 251/251 dont 86 par equivalence |
| Mosaic.d2s | 1020 | 105 | 8 | 8 | identique | 8/8 dont 6 par equivalence |
| Nova.d2s | 3260 | 105 | 72 | 95 | identique | 95/95 dont 33 par equivalence |
| PalaDuper.d2s | 1171 | 105 | 10 | 14 | identique | 14/14 dont 8 par equivalence |
| RingAmys.d2s | 2157 | 105 | 38 | 38 | identique | 38/38 dont 4 par equivalence |
| Runewords.d2s | 1340 | 105 | 9 | 27 | identique | 27/27 dont 22 par equivalence |
| SCSeptMf.d2s | 962 | 105 | 4 | 4 | identique | 4/4 dont 4 par equivalence |
| SetItems.d2s | 1944 | 105 | 27 | 27 | identique | 27/27 dont 4 par equivalence |
| Shay.d2s | 3419 | 105 | 70 | 105 | identique | 105/105 dont 50 par equivalence |
| Skillers.d2s | 1949 | 105 | 44 | 44 | identique | 44/44 dont 4 par equivalence |
| Spirit.d2s | 962 | 105 | 4 | 4 | identique | 4/4 dont 4 par equivalence |
| StarterItems.d2s | 962 | 105 | 4 | 4 | identique | 4/4 dont 4 par equivalence |
| Sunderer.d2s | 985 | 105 | 5 | 5 | identique | 5/5 dont 4 par equivalence |
| SuperDuper.d2s | 1452 | 105 | 13 | 27 | identique | 27/27 dont 18 par equivalence |
| SuperDupeuse.d2s | 1148 | 105 | 6 | 14 | identique | 14/14 dont 12 par equivalence |
| TCEightySeven.d2s | 1111 | 105 | 8 | 8 | identique | 8/8 dont 4 par equivalence |
| TransferOne.d2s | 1001 | 105 | 6 | 6 | identique | 6/6 dont 4 par equivalence |
| Trapper.d2s | 2773 | 105 | 61 | 97 | identique | 97/97 dont 62 par equivalence |
| UberEats.d2s | 2019 | 105 | 35 | 44 | identique | 44/44 dont 11 par equivalence |
| UniqItemsTwo.d2s | 1757 | 105 | 25 | 25 | identique | 25/25 dont 4 par equivalence |
| UniqueItems.d2s | 2147 | 105 | 35 | 35 | identique | 35/35 dont 4 par equivalence |
| Zealer.d2s | 3044 | 105 | 119 | 132 | identique | 132/132 dont 100 par equivalence |
| Zonya.d2s | 2406 | 105 | 54 | 66 | identique | 66/66 dont 28 par equivalence |
| tHrap.d2s | 2852 | 105 | 88 | 107 | identique | 107/107 dont 64 par equivalence |

## Corpus v105/windows (2 fichiers)

| fixture | octets | version | objets | lignes | round-trip C# | concordance C#/Java |
|---|---:|---:|---:|---:|---|---|
| ModernSharedStashSoftCoreV2.d2i | 4229 | 105 | 135 | 151 | identique | 151/151 dont 79 par equivalence |
| SharedStashSoftCoreV2.d2i | 294 | 105 | 9 | 9 | identique | 9/9 dont 9 par equivalence |

## Corpus legacy (3 fichiers)

| fixture | octets | version | objets | lignes | round-trip C# | concordance C#/Java |
|---|---:|---:|---:|---:|---|---|
| BasesTwo.d2s | 1368 | 99 | 24 | 24 | identique | Java refuse |
| JewelsCraft.d2s | 1194 | - | - | - | lecture refusee | les deux oracles refusent |
| SharedStashHardCoreV2.d2i | 710 | 99 | 34 | 34 | identique | Java refuse |

## Round-trip C# (`write(read(f)) == f`, octet a octet)

- `.d2s` : 54/54 identiques parmi les fichiers lus, sur 55 fichiers du corpus, avec 1 refus en lecture.
- `.d2i` : 5/5 identiques parmi les fichiers lus, sur 5 fichiers du corpus, avec 0 refus en lecture.
- `v105/linux/` : 55/55 identiques sur les fichiers lus.
- `v105/windows/` : 2/2 identiques sur les fichiers lus.
- `legacy/` : 2/2 identiques sur les fichiers lus.

Aucun fichier lu ne differe apres reecriture.

Fichiers refuses par l'oracle C# :

- `legacy/JewelsCraft.d2s` : InvalidOperationException: No txt data available for version 98. Available versions: 96, 97, 99, 105

## Desaccords C#/Java

Aucun. Les 57 fichiers lus par les deux oracles donnent 2564 lignes
concordantes sur conteneur, code, position, pile et qualite. Le detail :

- 1422 lignes identiques champ a champ, sans aucune regle ;
- 1142 lignes ramenees par au moins une equivalence, dont 1142 par la
  regle de qualite des objets compacts et 5 par la regle `OnCursor` / `TRANSIT`
  (une meme ligne peut relever des deux).

Fichiers refuses par l'oracle Java (v105 exclusif) :

- `legacy/BasesTwo.d2s` : io.github.paladijn.d2rsavegameparser.parser.ParseException: Unsupported version: 99
- `legacy/JewelsCraft.d2s` : io.github.paladijn.d2rsavegameparser.parser.ParseException: Unsupported version: 98
- `legacy/SharedStashHardCoreV2.d2i` : io.github.paladijn.d2rsavegameparser.parser.ParseException: Unsupported shared stash version 99

## Reperes anti-regression (PLAN section 6)

Fixture `v105/linux/ModernSharedStashSoftCoreV2.d2i`.

| repere | attendu | mesure |
|---|---|---|
| onglets | 7 (5 normaux, 1 AdvancedStash, 1 Chronicle) | 7 (5 normaux, 1 AdvancedStash, 1 Chronicle) |
| objets | 1533 | 227 objets, 1306 unites empilees dans l'onglet AdvancedStash, somme 1533 |
| or, premier onglet (index 0) | 1075411 | 1075411 |
| onglet Chronicle | 2298 octets | 2298 octets |

Le repere « 1533 objets » ne correspond pas au nombre d'objets du fichier : il
vaut 227 objets plus 1306 unites empilees dans l'onglet AdvancedStash. Les objets
sertis portent le total a 251 lignes.

## Notes mesurees

L'onglet `AdvancedStash` n'est pas une grille : les fichiers ecrits par le jeu
y contiennent des positions qui se recouvrent. Les deux oracles lisent les memes
valeurs ; ce n'est pas un desaccord.

- `v105/linux/ModernSharedStashHardCoreV2.d2i`, onglet d'index 5 : 0 objets, 0 positions deja occupees.
- `v105/linux/ModernSharedStashSoftCoreV2.d2i`, onglet d'index 5 : 81 objets, 43 positions deja occupees.
- `v105/windows/ModernSharedStashSoftCoreV2.d2i`, onglet d'index 5 : 68 objets, 41 positions deja occupees.

Comportement des oracles sur `legacy/` (versions 98 et 99, hors perimetre v105) :

- `legacy/BasesTwo.d2s` : C# lu (version 99), round-trip identique ; Java refuse (io.github.paladijn.d2rsavegameparser.parser.ParseException: Unsupported version: 99).
- `legacy/JewelsCraft.d2s` : C# refuse (InvalidOperationException: No txt data available for version 98. Available versions: 96, 97, 99, 105) ; Java refuse (io.github.paladijn.d2rsavegameparser.parser.ParseException: Unsupported version: 98).
- `legacy/SharedStashHardCoreV2.d2i` : C# lu (version 99), round-trip identique ; Java refuse (io.github.paladijn.d2rsavegameparser.parser.ParseException: Unsupported shared stash version 99).

Ecart avec PLAN section 6, qui attend que D2SSharp lise les fixtures
`legacy/` (v96 a v105) : la bibliotheque n'embarque pas les tables `txt` de
toutes les versions, et refuse donc les fichiers suivants.

- `legacy/JewelsCraft.d2s` : InvalidOperationException: No txt data available for version 98. Available versions: 96, 97, 99, 105

