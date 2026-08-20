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
commence sur une frontiere d'octet.

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

### 2.3 Huffman des codes objet

§5.4 le detaille.

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

## 4. Fichier de coffre partage `.d2i`

## 5. Enregistrement d'objet

## 6. Tables du jeu

## 7. Ecarts assumes
