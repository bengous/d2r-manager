---
paths:
  - "crates/format/**"
  - "docs/FORMAT.md"
---

# Bitstream et spec FORMAT.md

- Zero commentaire, sauf references de la forme `FORMAT.md §x.y` dans le code
  du bitstream (PLAN §5.3).
- Toute largeur de champ vient de `docs/FORMAT.md`, jamais d'un nombre magique
  local (PLAN §8.2).
- Code qui contredit la spec : corriger la spec dans le meme commit.
- Les goldens sont le corrige : ne jamais les ajuster pour faire passer un test.
- Version != 105 : erreur explicite (PLAN §3.5).
- `proptest` sur le bitreader uniquement (PLAN §6.4).
- Axes : `InvX` = colonne, `InvY` = ligne, convention D2SSharp (PLAN §6).
- `AdvancedStash` n'est pas une grille : jamais de position inventee, on
  empile ou on ajoute (PLAN §6).
