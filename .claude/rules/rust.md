---
paths:
  - "crates/**"
---

# Qualite Rust

- Concevoir les types et structures de donnees avant le code : viser le
  meilleur Rust possible, pas le premier qui compile.
- Etats invalides irrepresentables partout ou une donnee a plus d'une forme
  legitime : enums a la place de bools correles, newtypes pour les primitives
  porteuses de regles, typestate pour les operations ordonnees.
- Le skill `unrepresentable` est disponible via l'outil Skill : l'invoquer a
  la conception d'un nouveau type ou d'une signature publique a formes
  multiples. Usage selon la tache.
- Propriete et emprunts precis : les API prennent ce qu'elles consomment ;
  pas de clone de confort.
- Erreurs `thiserror` a variantes precises, contexte propage (PLAN §3.4).
