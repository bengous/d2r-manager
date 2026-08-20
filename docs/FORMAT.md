# Format de sauvegarde Diablo II Resurrected, version 105

Spec du bitstream que la crate `format` lit et ecrit. Les numeros de section
sont stables : le code du bitstream y refere par des commentaires
`FORMAT.md §x.y`, seule forme de commentaire autorisee (PLAN §5.3).

## 1. Perimetre et conventions

### 1.1 Perimetre

Cette spec couvre la version de sauvegarde 105 seule (PLAN §3.5). Deux types
de fichier :

| Extension | Contenu | Racine |
|---|---|---|
| `.d2s` | un personnage | §3 |
| `.d2i` | un coffre partage, suite d'onglets | §4 |

Toute autre version produit une erreur explicite portant la version lue.

### 1.2 Sources

| Source | Licence | Usage |
|---|---|---|
| `reference/D2SSharp/src/D2SSharp` | MIT | source de la spec, portage permis |
| `reference/halbu/src` | MIT | recoupement des offsets de l'en-tete |
| `~/Work/d2r-saves/docs/research/format-objet-v105.md` | recherche locale | recoupement |

Chaque champ ci-dessous porte sa reference croisee vers le source C# sous la
forme `Fichier.cs:Membre`. Les chemins sont relatifs a
`reference/D2SSharp/src/D2SSharp/`.

Le parseur Java `reference/d2rsavegameparser` est un oracle execute. Son
`src/` n'est ni lu ni porte : LGPL 2.1 (PLAN §2, §9.3).

### 1.3 Ordre des bits et des octets

`IO/BitReader.cs:ReadBits` lit du bit de poids faible vers le bit de poids
fort a l'interieur de chaque octet, puis passe a l'octet suivant. Un champ de
`n` bits a cheval sur deux octets prend d'abord les bits hauts de l'octet
courant, qui deviennent les bits bas de la valeur.

Les entiers alignes sur l'octet (`u16`, `u32`, `u64`) suivent cet ordre, ce qui
revient a du petit-boutiste. `u64` se lit en deux `u32` : bas puis haut
(`IO/BitReader.cs:ReadUInt64`).

Les tailles de champ sont donnees en bits. « aligne » signifie que le champ
commence sur une frontiere d'octet. Les sections §3.1 a §3.6 et §4.1 sont
entierement alignees ; le desalignement commence a la liste de stats du
joueur (§3.7) et regne dans l'enregistrement d'objet (§5).

### 1.4 Axes et vocabulaire

Convention D2SSharp, imposee par PLAN §6 : `InvX` est la **colonne**, `InvY`
est la **ligne**. Le parseur Java expose l'inverse ; les goldens Java sont
normalises sur la convention D2SSharp par le lot 1.

## 2. Primitives

### 2.1 Flux de bits

`crates/format/src/bits.rs`. `BitReader` lit depuis un `&[u8]`, `BitWriter`
ecrit dans un `Vec<u8>` qui grandit. Les deux comptent une position en bits.

`Bits<N>` porte une valeur qui tient sur `N` bits, `1 <= N <= 32`. La borne
est verifiee a la construction : une valeur trop large ne peut pas atteindre
le flux et deborder sur le champ suivant.

Les largeurs connues a la compilation passent par `Bits<N>`. Les largeurs
lues dans les tables du jeu (§6) passent par `read_bits(width)` et
`write_bits(value, width)`, qui refusent une valeur trop large au lieu de la
tronquer.

### 2.2 Chaines

| Forme | Encodage | Fin | Reference |
|---|---|---|---|
| chaine de taille fixe | octets bruts, UTF-8 | premier octet nul, ou la taille | `IO/BitReader.cs:ReadFixedString` |
| chaine d'objet | 8 bits par caractere | octet nul | `IO/BitReader.cs:Read8BitString` |

La chaine d'objet sert au nom d'oreille et au nom de personnalisation (§5.6).
En v105 elle est sur 8 bits ; la forme 7 bits ne concerne que les versions
inferieures ou egales a 97 (`Model/Item.cs:ReadString`).

`ReadFixedString` s'arrete au premier octet nul mais consomme toujours la
taille declaree. Les octets apres le nul sont conserves tels quels pour la
reecriture.

Longueur maximale d'une chaine d'objet : 63 octets utiles. Le tampon de
`Read8BitString` fait 64 caracteres et sort de la boucle sans consommer de
terminateur au 64e ; une chaine de 64 octets ne se relit donc pas. La crate
refuse au-dela de 63 au lieu de produire un fichier illisible.

### 2.3 Huffman des codes objet

§5.4.

### 2.4 Checksum du `.d2s`

`ChecksumCalculator.cs:Calculate`. Le checksum couvre tout le fichier, les
quatre octets du champ checksum (offset `0x0C`) comptes comme nuls :

```
checksum = 0
pour chaque octet b du fichier, dans l'ordre :
    checksum = rotation_gauche_32(checksum, 1) + b
```

L'addition est modulo 2^32. Le `.d2i` n'a pas de checksum.

## 3. Fichier de personnage `.d2s`

Ordre des sections, `Model/D2Save.cs:Read` :

en-tete (§3.1), Character (§3.2), quetes (§3.4), waypoints (§3.5),
introductions (§3.6), stats (§3.7), competences (§3.8), objets (§3.9),
cadavres (§3.10), puis, si le personnage est en extension : objets du
mercenaire (§3.11), golem de fer (§3.12) et, en Reign of the Warlock,
demon lie (§3.13). Le reste du fichier est une queue conservee telle
quelle (§3.14).

« En extension » se lit sur `PreviewData.GameVersion` (§3.3) en v104 et
au-dela, pas sur les drapeaux du personnage
(`Model/D2Save.cs:IsExpansion`).

### 3.1 En-tete de fichier, 16 octets

| Offset | Bits | Champ | Reference |
|---:|---:|---|---|
| 0 | 32 | magic `0xAA55AA55` | `Model/D2Save.cs:Magic` |
| 4 | 32 | version de sauvegarde, 105 attendue | `Model/D2Save.cs:Read` |
| 8 | 32 | taille du fichier en octets | `Model/D2Save.cs:Write` |
| 12 | 32 | checksum (§2.4) | `ChecksumCalculator.cs` |

Taille et checksum sont recalcules a l'ecriture ; les valeurs lues sont
conservees dans le modele pour la comparaison aux goldens.

### 3.2 Section Character, 387 octets

Taille v105 : 387 octets (`Model/Character.cs:SizeV104`), recoupee par
`reference/halbu/src/character/v105.rs:V105_CHARACTER_LENGTH`. Le champ
`Name` sur 16 octets des versions <= 103 a disparu ; le nom vit dans
`PreviewData` (§3.3).

| Offset | Bits | Champ | Reference |
|---:|---:|---|---|
| 0 | 32 | jeu d'armes actif | `Model/Character.cs:WeaponSwitch` |
| 4 | 32 | drapeaux du personnage | `Enums/CharacterFlags.cs` |
| 8 | 8 | classe, 0 a 7 | `Enums/CharacterClass.cs` |
| 9 | 8 | nombre de stats, 16 | `Model/Character.cs:NumStats` |
| 10 | 8 | nombre de competences, 30 | `Model/Character.cs:NumSkills` |
| 11 | 8 | niveau | `Model/Character.cs:Level` |
| 12 | 32 | date de creation | `Model/Character.cs:CreateTime` |
| 16 | 32 | date de derniere sauvegarde | `Model/Character.cs:LastSaveTime` |
| 20 | 32 | temps de jeu, vaut `0xFFFFFFFF` | `Model/Character.cs:PlayTime` |
| 24 | 16 x 32 | 16 raccourcis de competence | `Model/Skill.cs` |
| 88 | 32 | competence bouton gauche | `Model/Character.cs:LeftSkill` |
| 92 | 32 | competence bouton droit | `Model/Character.cs:RightSkill` |
| 96 | 32 | competence gauche, second jeu | `Model/Character.cs:SwitchLeftSkill` |
| 100 | 32 | competence droite, second jeu | `Model/Character.cs:SwitchRightSkill` |
| 104 | 16 octets | graphismes d'apparence | `Model/Character.cs:AppearanceGraphics` |
| 120 | 16 octets | teintes d'apparence | `Model/Character.cs:AppearanceTints` |
| 136 | 3 octets | ville courante par difficulte | `Model/Character.cs:TownDifficulty` |
| 139 | 32 | graine de carte | `Model/Character.cs:MapSeed` |
| 143 | 16 octets | mercenaire, voir ci-dessous | `Model/MercData.cs` |
| 159 | 228 octets | PreviewData (§3.3) | `Model/PreviewData.cs` |

Un raccourci de competence, 4 octets (`Model/Skill.cs:Read`) : 15 bits d'id
de competence, 1 bit de cote (0 droite, 1 gauche), puis 16 bits d'index
d'objet pour les competences accordees par un objet.

Mercenaire, 16 octets a l'offset 143 (`Model/MercData.cs:Read`) : drapeaux
`u32` (bit 16 = mort), graine `u32`, index de nom `u16`, type d'engage `u16`,
experience `u32`. Un mercenaire existe si graine, index de nom ou experience
est non nul (`Model/MercData.cs:HasMerc`) ; c'est cette condition qui decide
de la presence de la sous-section d'objets en §3.11.

### 3.3 PreviewData, 228 octets

Donnees d'affichage de l'ecran de selection, lues sans parcourir le fichier
entier. Disposition v104 et au-dela (`Model/PreviewData.cs:Read`).

| Offset relatif | Taille | Champ |
|---:|---:|---|
| 0 | 6 x 64 bits | dates de sauvegarde, FILETIME Windows |
| 48 | 6 x 32 bits | experiences totales |
| 72 | 8 bits | couleur d'embleme de guilde |
| 73 | 8 bits | version de jeu : 0 aucune, 1 classique, 2 extension, 3 Reign of the Warlock |
| 74 | 16 bits | remplissage |
| 76 | 12 octets | objet d'apercu, main gauche |
| 88 | 12 octets | objet d'apercu, main droite |
| 100 | 12 octets | objet d'apercu, torse |
| 112 | 12 octets | objet d'apercu, tete |
| 124 | 96 octets | nom du personnage, chaine de taille fixe (§2.2) |
| 220 | 64 bits | reserve, nul |

L'offset 73 tombe a l'offset 232 **dans la section Character**, soit 248
depuis le debut du fichier. C'est exactement ce que
`reference/halbu/src/character/v105.rs:OFFSET_MODE_MARKER` designe comme
marqueur de mode, avec les memes trois valeurs ; halbu indexe la section,
pas le fichier. Les deux lectures concordent.

Un objet d'apercu, 12 octets (`Model/PreviewItem.cs:Read`) : code `u32`
brut (non Huffman, 4 caracteres ASCII), transformation de couleur `u8`,
qualite `u8`, index de fichier `u16`, drapeaux `u32`.

Les emplacements gauche et droite sont inverses par rapport a
`BodyLocation` : l'arme principale (`RightArm`) alimente `LeftHand`
(`Model/D2Save.cs:Convert114ToD2R`).

### 3.4 Section de quetes, 298 octets

`Model/QuestSection.cs:Read`. Magic `"Woo!"` = `0x216F6F57` sur 32 bits,
version `u32`, taille declaree `u16` valant 298, puis trois blocs de
difficulte de 96 octets dans l'ordre normal, cauchemar, enfer.

Un bloc de difficulte, 96 octets (`Model/ActQuests.cs:QuestsDifficulty`) :
48 champs `u16` de drapeaux de quete, repartis en 8 pour l'acte I, 8 pour
l'acte II, 8 pour l'acte III, 8 pour l'acte IV et 16 pour l'acte V. Les noms
de chaque emplacement sont dans `Model/ActQuests.cs` ; les emplacements
`ReservedN` de l'acte IV et de l'acte V n'ont pas de quete associee. Les bits
d'un drapeau sont decrits par `Enums/QuestFlags.cs`.

### 3.5 Section de waypoints, 80 octets

`Model/WaypointSection.cs:Read`. Magic `"WS"` = `0x5357` sur 16 bits,
version `u32`, taille declaree `u16` valant 80, puis trois blocs de 24 octets
dans l'ordre normal, cauchemar, enfer.

Un bloc de difficulte, 24 octets (`Model/WaypointSection.cs:WaypointDifficultyData`) :

| Bits | Champ |
|---:|---|
| 16 | en-tete, vaut `0x0102` |
| 9 | waypoints de l'acte I |
| 9 | waypoints de l'acte II |
| 9 | waypoints de l'acte III |
| 3 | waypoints de l'acte IV |
| 9 | waypoints de l'acte V |
| 9 | remplissage, complete les 48 bits |
| 128 | reserve, 16 octets |

Les 39 bits de waypoints sont un champ de bits par acte, bit 0 pour le
premier waypoint de l'acte (`Enums/Waypoints.cs`).

### 3.6 Section d'introductions, 52 octets

`Model/PlayerIntroSection.cs:Read`. En-tete `u16` valant `0x7701`, taille
declaree `u16` valant 52, puis six blocs de 8 octets : d'abord les drapeaux
d'introduction de quete pour les trois difficultes, ensuite les drapeaux
d'introduction de PNJ pour les trois difficultes.

Chaque bloc est un champ de 64 bits indexe par numero de PNJ, bit `n` dans
l'octet `n / 8` a la position `n % 8` (`Model/NpcDialogData.cs:GetBit`).
L'index 0 est inutilise ; les index 1 a 34 nomment les PNJ, de Gheed a Cain
en acte V (`Model/NpcDialogData.cs`).

### 3.7 Section de stats du joueur, taille variable

`Model/PlayerStats.cs:Read`. Magic `"gf"` = `0x6667` sur 16 bits, puis une
suite d'enregistrements de stat, puis le terminateur, puis alignement sur
l'octet.

Un enregistrement :

| Bits | Champ |
|---:|---|
| 9 | id de stat ; `0x1FF` termine la liste |
| `CSvParam` | couche, lue **signee** |
| `CSvBits` | valeur, signee si `CSvBits < 32` et `CSvSigned`, sinon non signee |

`CSvParam`, `CSvBits` et `CSvSigned` viennent de `ItemStatCost.txt` (§6.1).
Un `CSvParam` nul ne consomme aucun bit. Cette section n'applique ni
`Save Add` ni `ValShift` : ces deux colonnes ne concernent que les stats
d'objet (§5.8).

### 3.8 Section de competences, taille variable

`Model/SkillsSection.cs:Read`. Magic `"if"` = `0x6669` sur 16 bits, puis un
octet de niveau par competence. Le nombre de competences vient du champ
`NumSkills` de §3.2, soit 30. L'id reel d'une competence vaut l'index plus
le decalage de classe : 6, 36, 66, 96, 126, 221, 251, 373 pour les classes 0
a 7 (`Model/SkillsSection.cs:ClassSkillOffsets`).

### 3.9 Section d'objets

`Model/ItemsSection.cs:Read`. Magic `"JM"` = `0x4D4A` sur 16 bits, compteur
`u16`, puis autant d'enregistrements d'objet (§5). Le compteur ne compte que
les objets de premier niveau : les objets sertis sont lus par leur porteur
et n'y figurent pas.

### 3.10 Section de cadavres

`Model/CorpseSection.cs:Read`. Magic `"JM"` = `0x4D4A` sur 16 bits, compteur
`u16` de cadavres, puis, par cadavre, trois `u32` (inconnu, x, y) suivis
d'une section d'objets complete au format §3.9.

### 3.11 Objets du mercenaire

`Model/MercItemsSection.cs:Read`. Magic `"jf"` = `0x666A` sur 16 bits. La
section d'objets §3.9 ne suit **que** si un mercenaire existe au sens de
§3.2 ; sinon la section suivante enchaine immediatement.

### 3.12 Golem de fer

`Model/IronGolemSection.cs:Read`. Magic `"kf"` = `0x666B` sur 16 bits, puis
un octet de presence. S'il est non nul, un enregistrement d'objet (§5) suit.

### 3.13 Demon lie

`Model/DemonSection.cs:TryRead`. Present a partir de la version 103, et
seulement quand la version de jeu de §3.3 vaut Reign of the Warlock. La
section s'annonce par une version `u16` valant 1 puis le magic `"lf"` =
`0x666C` sur 16 bits ; si l'un des deux ne correspond pas, la section est
absente et la position revient en arriere.

Suivent un octet de presence et un octet de remplissage. Si le demon est
present, un descripteur de 92 octets suit :

| Offset relatif | Bits | Champ |
|---:|---:|---|
| 0 | 16 | taille en octets du bloc de stats qui suit le descripteur |
| 2 | 16 | type de demon : 1 monstre standard, 2 super unique |
| 4 | 16 | index de classe, stocke a 1 pres (valeur du fichier moins 1) |
| 6 | 16 | graine de nom |
| 8 | 16 | drapeaux de monstre |
| 10 | 16 | nombre de stats, forme heritee |
| 12 | 8 | inconnu |
| 13 | 8 | inconnu |
| 14 | 16 | inconnu |
| 16 | 28 octets | inconnu |
| 44 | 5 x 32 | inconnus |
| 64 | 16 octets | composants |
| 80 | 10 octets | modificateurs uniques de monstre |
| 90 | 2 octets | inconnu |

Le bloc de stats qui suit occupe exactement la taille annoncee a l'offset 0.
Il porte le magic `"gf"` puis des enregistrements au format de §3.7 a une
difference pres : **la valeur fait toujours 32 bits**, quel que soit
`CSvBits` (`Model/DemonSection.cs:ReadDemonStats`). Le terminateur `0x1FF`
et l'alignement final sont les memes.

### 3.14 Queue

Tout ce qui reste apres la derniere section connue est conserve octet pour
octet (`Model/D2Save.cs:TrailingData`). Sur le corpus v105 cette queue est
vide.

## 4. Fichier de coffre partage `.d2i`

Une suite d'onglets sans en-tete de fichier. La lecture s'arrete quand il
reste au plus 64 octets, la taille d'un en-tete d'onglet
(`Model/D2StashSave.cs:Read`). Sur le corpus, les onglets couvrent le fichier
sans reste.

### 4.1 En-tete d'onglet, 64 octets

`Model/D2StashTab.cs:Read`.

| Offset relatif | Bits | Champ |
|---:|---:|---|
| 0 | 32 | magic `0xAA55AA55` |
| 4 | 32 | format de coffre ; le type d'onglet n'existe qu'a partir de 2 |
| 8 | 32 | format d'objet, c'est la version de sauvegarde des objets de l'onglet |
| 12 | 32 | or de l'onglet |
| 16 | 16 | taille totale de l'onglet, en-tete comprise |
| 18 | 16 | saison |
| 20 | 8 | type d'onglet : 0 normal, 1 AdvancedStash, 2 Chronicle |
| 21 | 43 octets | reserve |

Quand le format de coffre vaut moins de 2, le jeu ignore l'octet de type et
traite l'onglet comme normal (`Model/D2StashTab.cs:Read`). La crate suit
cette regle : le type effectif choisit le corps, et l'octet lu est reecrit
tel quel.

Quatre des cinq `.d2i` du corpus portent un format de coffre de 2. Le
cinquieme, `legacy/SharedStashHardCoreV2.d2i`, porte un format de coffre de 1
et un format d'objet de 99 ; la crate le refuse sur le format d'objet avant
d'atteindre la regle. Cette regle n'a donc aucune couverture par fixture :
elle repose sur le seul source C#.

Le champ de taille sert a atteindre l'onglet suivant : la position repart a
`debut + taille`. Les octets entre la fin du corps et cette borne sont
conserves tels quels (§4.3).

Le format d'objet est la version que §5 doit interpreter. La crate refuse
tout onglet dont ce champ ne vaut pas 105.

### 4.2 Corps d'onglet

- type 0 et type 1 : une section d'objets au format §3.9. La difference
  entre les deux tient a l'usage, pas au format ; l'onglet AdvancedStash
  porte des objets empiles (§5.10).
- type 2 : une section Chronicle (§4.3).

L'onglet AdvancedStash n'est pas une grille. Les fichiers ecrits par le jeu
y contiennent des positions qui se recouvrent : 43 collisions sur 81 objets
mesurees sur `v105/linux/ModernSharedStashSoftCoreV2.d2i` (PLAN §6,
`goldens/REPORT.md`). Ce n'est pas une anomalie et rien ici ne doit s'y
opposer.

### 4.3 Section Chronicle

`Model/ChronicleSection.cs:Read`. Suivi des objets uniques, de set et des
runewords trouves.

| Offset relatif | Taille | Champ |
|---:|---:|---|
| 0 | 32 bits | magic `0xC0EAEDC0` |
| 4 | 16 bits | version, vaut 1 |
| 6 | 16 bits | nombre d'entrees de set |
| 8 | 16 bits | nombre d'entrees d'unique |
| 10 | 16 bits | nombre d'entrees de runeword |
| 12 | 8 octets | reserve |
| 20 | n x 10 octets | entrees, sets puis uniques puis runewords |

Une entree, 10 octets : id d'objet `u32` (index de ligne de `items.txt` pour
un set ou un unique, index de chaine de runeword sinon), source `u16` (nulle
pour un runeword), horodatage `u32`.

Le corps ne remplit pas la taille declaree de l'onglet. L'ecrivain du jeu
ajoute 64 octets de tampon perime a la taille (`add ax, 40h` en
`0x140311e98`, note de `reference/D2SSharp/CLAUDE.md`) ; le lecteur les
ignore. La crate les conserve dans la queue de l'onglet, ce qui rend la
reecriture identique a l'octet pres.

## 5. Enregistrement d'objet

### 5.1 Forme generale

`Model/Item.cs:ReadFrom`. En v105 il n'y a **pas** de magic `"JM"` par
objet : ce prefixe n'existe qu'en version 96 et anterieures.

| Bits | Champ | Section |
|---:|---|---|
| 32 | drapeaux | §5.2 |
| 3 | version d'objet | §5.3 |
| 3 | mode | §5.5 |
| 15 ou 32 | position, selon le mode | §5.5 |
| var. | corps compact ou complet, selon le drapeau `CompactSave` | §5.6, §5.7 |

L'enregistrement se termine par un alignement sur l'octet : le jeu serialise
chaque objet dans un tampon neuf et n'en garde que les octets pleins
(`Model/Item.cs:ReadFrom`, appel a `AlignToByte`). Les objets sertis suivent
immediatement, chacun etant un enregistrement complet au meme format.

Le flux n'est donc jamais desaligne d'un objet au suivant ; il l'est a
l'interieur d'un objet.

### 5.2 Drapeaux, 32 bits

`Enums/ItemFlags.cs`. Bits utilises par la lecture, dans l'ordre des valeurs :

| Valeur | Nom | Effet sur le format |
|---:|---|---|
| `0x00000001` | NewItem | aucun |
| `0x00000002` | Target | aucun |
| `0x00000004` | Targeting | aucun |
| `0x00000008` | Deleted | aucun |
| `0x00000010` | Identified | aucun en sauvegarde : les donnees de qualite sont toujours ecrites |
| `0x00000020` | HasQuantity | aucun |
| `0x00000040` | SwitchIn | aucun |
| `0x00000080` | SwitchOut | aucun |
| `0x00000100` | Broken | aucun |
| `0x00000200` | Repaired | aucun |
| `0x00000800` | Socketed | ajoute le nombre de sockets (§5.6) |
| `0x00001000` | NoSell | aucun |
| `0x00002000` | InStore | aucun |
| `0x00004000` | NoEquip | aucun |
| `0x00008000` | Named | aucun |
| `0x00010000` | IsEar | remplace le code objet par des donnees d'oreille |
| `0x00020000` | StarterItem | aucun |
| `0x00080000` | Init | aucun ; D2SSharp l'efface a la lecture |
| `0x00200000` | CompactSave | choisit le corps compact au lieu du corps complet |
| `0x00400000` | Ethereal | aucun |
| `0x00800000` | JustSaved | aucun ; D2SSharp le pose a l'ecriture |
| `0x01000000` | Personalized | ajoute un nom de personnalisation |
| `0x02000000` | LowQuality | tronque l'enregistrement, voir §7 |
| `0x04000000` | Runeword | ajoute l'id puis la liste de stats de runeword |
| `0x08000000` | Duplicated | aucun |
| `0x10000000` | Chronicle | ajoute le bloc Chronicle d'objet (§5.9) |
| `0x20000000` | ChronicleCompact | choisit la forme compacte du bloc Chronicle |

Mesure sur les 2404 objets de `v105/linux` : aucun objet ne porte `Init` ni
`LowQuality`, tous portent `JustSaved`, et `CompactSave` concorde toujours
avec la colonne `compactsave` des tables (§6.2).

### 5.3 Version d'objet, 3 bits

`Model/Item.cs:ReadItemFormatVersion`. Un bit de forme haute, puis 2 bits de
valeur. La version vaut `valeur + 99` si le bit est pose, `valeur` sinon.
Les versions representables sont donc 0 a 3 et 99 a 102.

Tous les objets du corpus v105 portent la version 101, soit bit pose et
valeur 2.

### 5.4 Code objet, Huffman

`HuffmanEncoding.cs`. Quatre symboles, sans terminateur : un code objet fait
toujours 4 caracteres, les codes plus courts etant completes par des
espaces. L'alphabet est l'espace, les chiffres et les lettres minuscules.

Le code de chaque symbole est donne bit de poids fort en premier dans la
table ci-dessous ; a l'ecriture les bits sont inverses, puisque le flux est
lu du bit de poids faible en premier (§1.3). A la lecture, on accumule bit a
bit jusqu'a reconnaitre un code, au plus 9 bits.

| Symbole | Code | Bits | Symbole | Code | Bits |
|---|---|---:|---|---|---:|
| espace | `10` | 2 | `m` | `01101` | 5 |
| `s` | `0010` | 4 | `p` | `11001` | 5 |
| `b` | `0101` | 4 | `r` | `11100` | 5 |
| `7` | `01111` | 5 | `t` | `01100` | 5 |
| `9` | `01110` | 5 | `u` | `00001` | 5 |
| `a` | `11110` | 5 | `w` | `00000` | 5 |
| `c` | `01000` | 5 | `x` | `00111` | 5 |
| `g` | `11010` | 5 | `2` | `001100` | 6 |
| `h` | `00011` | 5 | `8` | `000100` | 6 |
| `l` | `11101` | 5 | `d` | `110001` | 6 |
| `e` | `110000` | 6 | `f` | `010011` | 6 |
| `k` | `010010` | 6 | `n` | `001101` | 6 |
| `1` | `1111100` | 7 | `3` | `1101101` | 7 |
| `6` | `1101111` | 7 | `i` | `1111110` | 7 |
| `o` | `1111111` | 7 | `v` | `1101110` | 7 |
| `y` | `0001010` | 7 | `0` | `11111011` | 8 |
| `4` | `11111010` | 8 | `5` | `00010110` | 8 |
| `q` | `11011001` | 8 | `z` | `11011000` | 8 |
| `j` | `000101110` | 9 | | | |

Un code objet dont un octet sort de cet alphabet ne peut pas s'ecrire. La
crate refuse de construire un tel code plutot que de produire un
enregistrement illisible.

Un objet oreille n'a pas de code dans le flux ; le lecteur lui attribue
`ear ` (`Model/Item.cs:ReadCompactItem`), ce qui sert ensuite a la recherche
dans les tables.

### 5.5 Mode et position

`Model/ItemPosition.cs:Read`. Le mode occupe 3 bits et choisit la forme de la
position.

| Mode | Valeur | Position |
|---|---:|---|
| Stored | 0 | emplacement |
| Equipped | 1 | emplacement |
| InBelt | 2 | emplacement |
| OnGround | 3 | coordonnees de sol |
| OnCursor | 4 | emplacement |
| Dropping | 5 | coordonnees de sol |
| Socketed | 6 | emplacement |

Coordonnees de sol : `x` sur 16 bits puis `y` sur 16 bits.

Emplacement, 15 bits :

| Bits | Champ |
|---:|---|
| 4 | emplacement de corps : 0 aucun, 1 tete, 2 cou, 3 torse, 4 bras droit, 5 bras gauche, 6 anneau droit, 7 anneau gauche, 8 ceinture, 9 pieds, 10 gants, 11 main droite, 12 main gauche |
| 4 | `InvX`, la **colonne** |
| 4 | `InvY`, la **ligne** |
| 3 | page de rangement, **stockee a 1 pres** |

La page brute vaut la page interne plus un : 0 signifie aucune page (objets
sertis notamment), 1 inventaire, 2 equipement, 3 echange, 4 cube, 5 coffre,
6 ceinture. La huitieme valeur du champ, la page brute 7, n'a pas de nom :
D2SSharp la lit comme une page interne 6 hors enumeration et la reecrit a 7.
La crate la porte sous un nom propre et la reecrit de meme. C'est le seul
champ de position a reecrire pour deplacer un objet entre conteneurs.

Un objet non equipe doit porter un emplacement de corps nul, sinon le jeu
refuse le fichier (`Model/D2StashTab.cs:Convert114ToD2R`).

### 5.6 Corps

Le drapeau `CompactSave` choisit la forme.

**Corps compact** (`Model/Item.cs:ReadCompactItem`), dans l'ordre :

| Bits | Champ | Condition |
|---:|---|---|
| 3 | index de fichier d'oreille | drapeau `IsEar` |
| 7 | niveau du proprietaire de l'oreille | drapeau `IsEar` |
| var. | nom du proprietaire, chaine d'objet (§2.2) | drapeau `IsEar` |
| var. | code objet Huffman (§5.4) | sinon |
| 1 (+12 ou +32) | montant d'or | code de type or |
| 1 | or du joueur | code de type or |
| `Save Bits` | difficulte de l'objet de quete | code de quete avec controle de difficulte |
| 1 (+4x32) | donnees de royaume | toujours |
| 1 (+8) | pile AdvancedStash (§5.10) | toujours |

Le bit qui precede le montant d'or choisit sa largeur : 32 bits s'il est
pose, 12 bits sinon. D2SSharp le derive du montant a l'ecriture ; la crate
reecrit la forme lue, ce qui garantit la reecriture a l'identique.

La difficulte de l'objet de quete est lue sur `Save Bits` bits de la stat
`item_quest_difficulty` (id 356 : 2 bits, `Save Add` nul, `ValShift` nul) et
decodee comme une stat d'objet (§5.8). Le controle porte sur les colonnes
`quest` et `questdiffcheck` de la table d'objets (§6.2).

Un objet compact a, par convention du lecteur, un niveau de 1 et une qualite
normale ; ces deux valeurs ne sont pas dans le fichier.

**Corps complet** (`Model/Item.cs:ReadCompleteItem`), dans l'ordre :

| Bits | Champ | Condition |
|---:|---|---|
| var. | code objet Huffman (§5.4) | toujours |
| 3 | nombre de sertis presents | toujours |
| 32 | graine de l'objet | toujours |
| 7 | niveau de l'objet | toujours |
| 4 | qualite | toujours |
| 1 (+3) | index de graphisme variable | toujours |
| 1 (+11) | id d'affixe automatique | toujours |
| var. | donnees de qualite (§5.7) | selon la qualite |
| 16 | id de runeword | drapeau `Runeword` |
| 3 + 7 + var. | donnees d'oreille | drapeau `IsEar` |
| var. | nom de personnalisation | drapeau `Personalized`, sinon |
| 1 (+4x32) | donnees de royaume | toujours |
| 11 + 8 (+9) | defense puis durabilite | code de type armure |
| 8 (+9) | durabilite | code de type arme |
| 1 (+12 ou +32) + 1 | or | code de type or |
| 1 (+9) | quantite | toujours en v105 |
| 4 | nombre de sockets | drapeau `Socketed` |
| 5 | masque de set | qualite set |
| var. | liste de stats principale (§5.8) | toujours |
| var. | listes de bonus de set, une par bit pose du masque | qualite set |
| var. | liste de stats de runeword | drapeau `Runeword` |
| var. | bloc Chronicle (§5.9) | drapeau `Chronicle` |
| 1 (+8) | pile AdvancedStash (§5.10) | toujours |

Oreille et personnalisation sont exclusives : le drapeau `IsEar` l'emporte.

La quantite sur 1 bit de presence plus 9 bits de valeur, **pour tous les
objets et non plus pour les seuls objets empilables**, est le changement de
la version 105 (`Model/Item.cs:ReadCompleteItem`, branche `saveVersion > 104`).
La colonne `stackable` des tables n'a donc plus de role ici.

Les largeurs de defense, de durabilite et de nombre de sockets ne sont pas
codees en dur : elles viennent de `ItemStatCost.txt` (§6.1). Mesurees en
v105 : defense sur 11 bits avec `Save Add` de 10, durabilite maximale sur
8 bits, durabilite courante sur 9 bits, nombre de sockets sur 4 bits, toutes
avec un `Save Add` nul sauf la defense.

La durabilite courante n'est presente que si la durabilite maximale lue est
strictement positive ; une durabilite maximale nulle signifie indestructible
et rien ne suit.

Les donnees de royaume sont un bit de presence puis quatre `u32`.

### 5.7 Donnees de qualite

`Model/ItemQualityData.cs`. Le champ de qualite sur 4 bits choisit la charge
utile. Aucune n'est conditionnee par le drapeau `Identified` : en sauvegarde
elles sont toujours ecrites.

| Qualite | Valeur | Charge utile |
|---|---:|---|
| Inferior | 1 | 3 bits d'index de fichier |
| Normal | 2 | rien, sauf les cas ci-dessous |
| Superior | 3 | 3 bits d'index de fichier |
| Magic | 4 | 11 bits de prefixe puis 11 bits de suffixe |
| Set | 5 | 12 bits d'index de fichier |
| Rare | 6 | 8 bits de prefixe de nom, 8 bits de suffixe de nom, puis 3 fois : 1 bit (+11) de prefixe et 1 bit (+11) de suffixe |
| Unique | 7 | 12 bits d'index de fichier |
| Craft | 8 | meme forme que Rare |
| Tempered | 9 | 8 bits de prefixe de nom puis 8 bits de suffixe de nom |

Cas speciaux de la qualite normale (`Model/Item.cs:ReadNormalQualityData`),
dans cet ordre de priorite :

- code de type charme : 1 bit qui dit prefixe, puis 11 bits d'affixe ;
- code de partie de corps de monstre, sans etre une partie de corps de
  joueur : 10 bits d'index de fichier ;
- code de parchemin ou de livre : 5 bits de type de sort.

Les identifiants de prefixe magique et d'affixe automatique subissent un
decalage dependant de la version (`Data/IExternalData.cs:GetMagicPrefixOffset`
et `GetAutoMagicOffset`). Ces decalages ne concernent que les versions
`0x48` a `0x58`, soit 72 a 88 ; **en version 105 ils valent zero** et la
valeur du fichier est la valeur semantique.

`Tempered = 9` n'existe pas dans le format historique. Aucun fichier du
corpus ne la porte ; la crate la lit et l'ecrit sans l'avoir observee.

### 5.8 Listes de stats d'objet

`Model/Item.cs:ReadStatList`. Une suite d'enregistrements terminee par l'id
`0x1FF`.

| Bits | Champ |
|---:|---|
| 9 | id de stat ; `0x1FF` termine la liste |
| `Save Param Bits` | parametre, lu **non signe** |
| `Save Bits` | valeur brute |

La valeur semantique vaut `(brut - Save Add) << ValShift`. Les trois
colonnes viennent de `ItemStatCost.txt` (§6.1). C'est la difference avec la
liste de stats du joueur (§3.7), qui utilise `CSvBits` et `CSvParam` et
n'applique ni decalage ni offset.

Six stats en entrainent d'autres, lues juste apres sans id propre
(`Enums/StatId.cs:GetPairedStats`) :

| Stat declencheuse | Id | Stats appariees | Ids |
|---|---:|---|---:|
| `MaxDamagePercent` | 17 | `MinDamagePercent` | 18 |
| `FireMinDamage` | 48 | `FireMaxDamage` | 49 |
| `LightningMinDamage` | 50 | `LightningMaxDamage` | 51 |
| `MagicMinDamage` | 52 | `MagicMaxDamage` | 53 |
| `ColdMinDamage` | 54 | `ColdMaxDamage`, `ColdLength` | 55, 56 |
| `PoisonMinDamage` | 57 | `PoisonMaxDamage`, `PoisonLength` | 58, 59 |

Chaque stat appariee est lue avec ses propres `Save Bits`, `Save Add` et
`ValShift`, sans parametre.

Apres `PoisonMinDamage`, le lecteur du jeu pose en memoire
`STAT_POISON_COUNT` (id 326) a 1. Cette stat n'occupe aucun bit : sa colonne
`Save Bits` vaut zero et elle n'est jamais ecrite. La crate ne la stocke
pas ; elle la deduit de la presence de `PoisonMinDamage` quand il faut
reproduire le modele de l'oracle.

### 5.9 Bloc Chronicle d'objet

`Model/Item.cs:ReadChronicle`. Present a partir de la version 100 quand le
drapeau `Chronicle` est pose.

| Bits | Champ | Condition |
|---:|---|---|
| 16 | id de Chronicle | toujours |
| 32 | horodatage, en minutes depuis une epoque propre au jeu | drapeau `ChronicleCompact` absent |
| 4 | nombre de destinataires | drapeau `ChronicleCompact` absent |
| n x 64 | destinataires | n vaut 1 en forme compacte |

Un destinataire fait deux `u32` : identifiant de compte de jeu puis
identifiant de personnage (`Model/CharacterIdentity.cs`). Aucun objet du
corpus ne porte ce bloc.

### 5.10 Pile AdvancedStash

`Model/Item.cs:ReadAdvancedStashStackSize`. A partir de la version 102 :
1 bit de presence puis 8 bits de taille de pile. Le champ clot
l'enregistrement, corps compact comme corps complet.

825 objets du corpus sur 2404 portent ce champ. La somme des piles de
l'onglet AdvancedStash de `v105/linux/ModernSharedStashSoftCoreV2.d2i` vaut
1306, ce qui reconstitue le repere « 1533 objets » de PLAN §6 : 227 entrees
d'objet plus 1306 unites empilees.

### 5.11 Objets sertis

Le nombre de sertis presents est lu sur 3 bits en tete du corps complet ; la
capacite totale est lue plus loin sur `Save Bits` bits de la stat
`item_numsockets`, et seulement si le drapeau `Socketed` est pose. Les
sertis presents occupent les premieres places ; la difference entre capacite
et sertis presents est le nombre d'emplacements vides.

Les enregistrements des sertis suivent celui du porteur, apres son
alignement, chacun au format complet de §5.1.

## 6. Tables du jeu

La longueur d'un enregistrement d'objet depend des tables du jeu : sans
elles, il est impossible de savoir combien de bits lire. La crate embarque
les seules colonnes dont elle a besoin, derivees des `.txt` v105 du
submodule D2SSharp (`Data/Txt/105/`, MIT).

Fichier genere : `crates/format/src/tables/v105.rs`. Generateur et garde
anti-derive : `crates/format/tests/tables.rs`, recette `just tables`. Le
test echoue si le fichier commite ne correspond plus aux `.txt` de
reference, ce qui interdit une derive silencieuse.

### 6.1 Table des stats

368 lignes, indexees par id de stat, tirees de `itemstatcost.txt`. Colonnes
retenues :

| Colonne | Usage |
|---|---|
| `CSvBits` | largeur de valeur des stats de joueur (§3.7) |
| `CSvParam` | largeur de parametre des stats de joueur et de demon |
| `CSvSigned` | signe des valeurs de stats de joueur |
| `Save Bits` | largeur de valeur des stats d'objet (§5.8) et des champs de §5.6 |
| `Save Param Bits` | largeur de parametre des stats d'objet |
| `Save Add` | offset soustrait a la lecture, ajoute a l'ecriture |
| `ValShift` | decalage applique apres l'offset |

L'id de stat est l'index de ligne du fichier, en-tete exclue. Une ligne vide
compte comme une ligne : c'est ce qui garde les index alignes.

### 6.2 Table des objets

692 codes objet, tires de `weapons.txt`, `armor.txt` et `misc.txt` dans cet
ordre, premiere occurrence gagnante en cas de doublon. Chaque code porte
onze drapeaux :

| Drapeau | Origine |
|---|---|
| `compact_save` | colonne `compactsave` de la table d'objets |
| `has_variable_gfx` | colonne `VarInvGfx` du type d'objet, non nulle |
| `is_armor` | le type d'objet descend de `armo` |
| `is_weapon` | le type d'objet descend de `weap` |
| `is_gold` | le type d'objet descend de `gold` |
| `is_charm` | le type d'objet descend de `char` |
| `is_body_part` | le type d'objet descend de `body` |
| `is_player_body_part` | le type d'objet descend de `play` |
| `is_scroll_or_book` | le type d'objet descend de `scro` ou de `book` |
| `is_quest` | colonne `quest` |
| `quest_diff_check` | colonne `questdiffcheck` |

La descendance suit les colonnes `Equiv1` et `Equiv2` de `itemtypes.txt`,
transitivement (`Data/TxtFileExternalData.cs:BuildTypeDescendants`). Un
`Equiv` qui pointe la ligne d'index 0 n'est pas suivi, comme dans le source
de reference.

La colonne `stackable` n'est pas reprise : en v105 la quantite passe par un
drapeau de presence pour tous les objets (§5.6).

Un code objet absent de la table produit une erreur. Le corpus ne contient
aucun code inconnu.

## 7. Ecarts assumes

Ecarts entre cette implementation et D2SSharp, tous mesures sans effet sur
le corpus.

1. **Objets de pari.** Le drapeau `LowQuality` marque un objet de pari :
   D2SSharp memorise le drapeau, l'efface, et sort de la lecture juste apres
   le code objet en forcant la qualite inferieure. Un tel enregistrement est
   tronque et ne peut pas se reecrire : D2SSharp le reserialiserait en objet
   complet, donc en octets differents. La crate refuse la lecture avec une
   erreur explicite. Aucun objet du corpus ne porte ce drapeau.

2. **Normalisation des drapeaux.** D2SSharp efface `Init` a la lecture, pose
   `JustSaved` a l'ecriture et force `CompactSave` depuis les tables. La
   crate reecrit le mot de drapeaux tel qu'il a ete lu. Mesure sur les 2404
   objets du corpus : aucun `Init`, `JustSaved` partout, `CompactSave`
   toujours conforme aux tables. Les deux comportements coincident donc,
   celui de la crate garantissant en plus la reecriture a l'identique.

3. **Bornes a l'ecriture.** D2SSharp borne a l'ecriture le niveau d'objet,
   les coordonnees, le nombre de sockets, la defense et la durabilite. La
   crate ecrit la valeur lue et signale une erreur si elle ne tient pas dans
   son champ, plutot que de la tronquer en silence.

   D2SSharp borne aussi **a la lecture** : un niveau d'objet nul devient 1
   (`Model/Item.cs:ReadCompleteItem`). La crate garde la valeur lue, ce qui
   preserve la reecriture. Le niveau minimal du corpus vaut 1 ; les deux
   comportements coincident.

4. **Stats a valeur nulle.** D2SSharp saute a l'ecriture toute stat dont la
   valeur decalee est nulle, ou dont `Save Bits` vaut zero. La crate ecrit
   la liste telle qu'elle l'a lue. Un fichier contenant une telle stat
   casserait la reecriture de D2SSharp ; le corpus n'en contient pas.

5. **Donnees de royaume tout a zero.** D2SSharp derive le bit de presence
   des donnees de royaume de leur contenu : un bloc de quatre zeros est
   reecrit sans bit de presence, donc en octets differents. La crate ecrit
   le bit tel qu'il a ete lu. Aucun objet du corpus ne porte de donnees de
   royaume.

6. **Destinataires Chronicle.** D2SSharp borne a 8 le nombre de
   destinataires lus, alors que le champ en compte 4 bits ; au-dela il
   desynchronise le flux. La crate lit exactement le nombre annonce. Aucun
   objet du corpus ne porte de bloc Chronicle.

7. **Queue d'onglet de coffre.** D2SSharp ne conserve les octets entre la fin
   du corps et la taille declaree que pour les onglets Chronicle. La crate
   les conserve pour tous les types d'onglet, et conserve aussi les octets
   qui suivent le dernier onglet. Sur le corpus, ces deux queues sont vides
   hors onglet Chronicle.

8. **Valeurs hors enumeration.** Mode d'objet 7, emplacement de corps 13 a
   15, qualite 0 ou 10 a 15, classe de personnage au-dela de 7, version de
   jeu au-dela de 3 et type d'onglet au-dela de 2 produisent une erreur au
   lieu d'une valeur sans nom. Le corpus n'en contient aucune. Mesure sur les
   2404 objets de `v105/linux` : les modes observes sont 0, 1, 2 et 6, les
   emplacements 0 a 12, les qualites 2 a 8, les pages brutes 0, 1, 4 et 5. Le
   mode 4 n'apparait que dans les deux fichiers `v105/windows`, sur 5 objets.

9. **Lints de documentation.** `clippy::missing_errors_doc` et
   `clippy::missing_panics_doc` sont desactives dans le workspace : ils
   exigent des commentaires de documentation, ce que PLAN §5.3 interdit.
