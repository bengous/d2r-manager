---
paths:
  - "crates/format/**"
  - "docs/FORMAT.md"
---

# Bitstream et spec FORMAT.md

- Zero commentaire, sauf les references de la forme `FORMAT.md §x.y` dans le
  code du bitstream : seule forme de commentaire autorisee du depot.
- Toute largeur de champ vient de `docs/FORMAT.md`, jamais d'un nombre magique
  local. Code qui contredit la spec : corriger la spec dans le meme commit.
- Les goldens sont le corrige : ne jamais les ajuster pour faire passer un
  test. Le round-trip octet seul ne prouve rien, un champ lu au mauvais offset
  et reecrit au meme offset le passe. La preuve est la concordance champ a
  champ (`tests/project/`).
- Version de sauvegarde autre que 105 : erreur explicite, aucune
  compatibilite.
- `proptest` sur le bitreader (`src/bits.rs`) uniquement.
- Axes : `InvX` = colonne, `InvY` = ligne, convention D2SSharp. Cette rule est
  le foyer de la convention pour tout le depot.
